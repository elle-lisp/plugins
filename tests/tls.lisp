(elle/epoch 10)

## TLS plugin integration tests
## Tests the tls plugin (.so loaded via import-file)
##
## The plugin is a pair of pure state machines — it does no I/O. These tests
## exploit that: a client state machine and a server state machine are wired
## to each other in this process, with each side's outgoing ciphertext handed
## straight to the other. No sockets, no network, no elle-side TLS library.
##
## Elle's own tests/elle/tls.lisp covers the socket path through lib/tls.lisp
## and needs network access. This file covers the primitives themselves and
## must run offline, so the two do not overlap.

## Try release first, then debug — a local `cargo build -p elle-tls` puts the
## .so in debug, while CI downloads the release artifact.
(def [ok? plugin]
  (let [[r-ok? r] (protect (import-file "target/release/libelle_tls.so"))]
    (if r-ok? [r-ok? r] (protect (import-file "target/debug/libelle_tls.so")))))

## Report why the load failed. "not built" is the common case, but a plugin
## built against a different ABI version is refused too, and a bare SKIP
## would hide that in CI.
(when (not ok?)
  (print "SKIP: tls plugin did not load: " plugin "\n")
  (exit 0))

## Plugin primitives are not resolvable by name at compile time. They are
## reached through the struct that import-file returns.
(def client-state-fn        (get plugin :client-state))
(def server-config-fn       (get plugin :server-config))
(def server-state-fn        (get plugin :server-state))
(def process-fn             (get plugin :process))
(def get-outgoing-fn        (get plugin :get-outgoing))
(def get-plaintext-fn       (get plugin :get-plaintext))
(def read-plaintext-fn      (get plugin :read-plaintext))
(def plaintext-indexof-fn   (get plugin :plaintext-indexof))
(def handshake-complete?-fn (get plugin :handshake-complete?))
(def write-plaintext-fn     (get plugin :write-plaintext))
(def alpn-protocol-fn       (get plugin :alpn-protocol))
(def close-notify-fn        (get plugin :close-notify))

## ── The registered primitive table ────────────────────────────────

(each k in [:client-state :server-config :server-state :process
            :get-outgoing :get-plaintext :read-plaintext
            :plaintext-indexof :handshake-complete? :write-plaintext
            :alpn-protocol :close-notify]
  (assert (not (nil? (get plugin k)))
          (string "tls: plugin must export " (string k))))

## ── A client with no peer ─────────────────────────────────────────

(let [st (client-state-fn "example.com")]
  (assert (= (process-fn st (bytes)) :handshaking)
          "tls/process: a fresh client is :handshaking")
  (let [hello (get-outgoing-fn st)]
    (assert (> (length hello) 0)
            "tls/get-outgoing: a fresh client queues a ClientHello")
    ## RFC 8446 §5.1: 22 is the handshake record content type.
    (assert (= (get hello 0) 22)
            "tls/get-outgoing: the ClientHello is a handshake record"))
  (assert (= (length (get-outgoing-fn st)) 0)
          "tls/get-outgoing: draining twice yields nothing the second time")
  (assert (not (handshake-complete?-fn st))
          "tls/handshake-complete?: false with no peer to answer"))

## ── Handshake driver ──────────────────────────────────────────────

## A handshake settles in three flights, so 64 rounds is slack of two orders
## of magnitude. The bound exists to fail the test instead of hanging when a
## state machine stops making progress.
(def max-rounds 64)

(defn handshake [client server]
  "Run a TLS handshake between two in-process state machines.
   Hand each side's outgoing ciphertext to the other until both report
   the handshake complete. Errors if the exchange stops making progress."
  (process-fn client (bytes))  ## pump out the ClientHello
  (let [@rounds 0]
    (forever
      (assign rounds (+ rounds 1))
      (when (> rounds max-rounds)
        (error {:error :test-error
                :message "tls handshake did not settle"}))
      (let [c-out (get-outgoing-fn client)]
        (when (> (length c-out) 0) (process-fn server c-out)))
      (let [s-out (get-outgoing-fn server)]
        (when (> (length s-out) 0) (process-fn client s-out)))
      (when (and (handshake-complete?-fn client)
                 (handshake-complete?-fn server))
        (break nil)))))

(defn deliver [from to payload]
  "Encrypt payload on one state machine and feed the ciphertext to the other.
   Returns the status keyword the receiver reported."
  (let [w (write-plaintext-fn from payload)]
    (assert (= w:status :ok)
            (string "tls/write-plaintext: expected :ok, got " (string w:message)))
    (assert (> (length w:outgoing) 0)
            "tls/write-plaintext: an :ok write must carry ciphertext")
    (process-fn to w:outgoing)))

(defn connected [config client-opts]
  "Handshake a client against a server config in process.
   Returns [client server]."
  (let [client (client-state-fn "localhost" client-opts)
        server (server-state-fn config)]
    (handshake client server)
    [client server]))

(defn openssl [argv]
  "Run openssl with the given argument array. Returns its exit status."
  (let [r (subprocess/system "openssl" argv)]
    r:exit))

(defn self-signed [cert-path key-path common-name]
  "Write a self-signed certificate and its key. Returns the openssl status."
  (openssl ["req" "-x509" "-newkey" "rsa:2048"
            "-keyout" key-path "-out" cert-path
            "-days" "1" "-nodes" "-subj" (string "/CN=" common-name)]))

## ── Loopback: handshake, data transfer, shutdown ──────────────────
##
## Needs a certificate, so it needs openssl. Skip the section if openssl is
## missing rather than failing the whole file.

(with-temp-dir scratch
  (let [cert-path (path/join scratch "cert.pem")
        key-path (path/join scratch "key.pem")
        gen (self-signed cert-path key-path "localhost")]
    (if (not (= gen 0))
      (println "SKIP: openssl not available; loopback tests skipped")
      (begin
        (def config (server-config-fn cert-path key-path))
        (def client (client-state-fn "localhost" {:no-verify true}))
        (def server (server-state-fn config))

        (handshake client server)
        (assert (handshake-complete?-fn client) "tls: client completes the handshake")
        (assert (handshake-complete?-fn server) "tls: server completes the handshake")

        ## ── Application data, client to server ────────────────────

        (assert (= (deliver client server (bytes "hello\n")) :has-data)
                "tls/process: decrypted application data reports :has-data")
        (assert (= (string (get-plaintext-fn server)) "hello\n")
                "tls/get-plaintext: the server reads what the client wrote")
        (assert (= (length (get-plaintext-fn server)) 0)
                "tls/get-plaintext: draining twice yields nothing the second time")

        ## ── Partial drain, server to client ───────────────────────

        (deliver server client (bytes "echo: hello\nrest"))
        (assert (= (plaintext-indexof-fn client 10) 11)
                "tls/plaintext-indexof: finds the newline without draining")
        (assert (= (string (read-plaintext-fn client 12)) "echo: hello\n")
                "tls/read-plaintext: drains exactly the requested bytes")
        (assert (= (string (get-plaintext-fn client)) "rest")
                "tls/read-plaintext: leaves the remainder buffered")
        (assert (nil? (plaintext-indexof-fn client 10))
                "tls/plaintext-indexof: nil when the byte is absent")

        ## ── A payload spanning many TLS records ───────────────────
        ##
        ## rustls splits plaintext across 16 KiB records and each record pays
        ## its own header and AEAD tag, so the ciphertext grows with the
        ## RECORD COUNT, not by a fixed amount. A write buffer sized with flat
        ## slack covers only the first handful of records and every larger
        ## write then fails outright. 1 MB is 62 records — far past any flat
        ## guess, and cheap to run in process.
        ##
        ## Counter-factual: with the buffer sized at plaintext + 256 bytes,
        ## tls/write-plaintext raises "encrypt error: cannot encrypt due to
        ## insufficient size" and this section never reaches its assertion.

        (def big-size 1000000)

        (deliver client server (bytes (string/repeat "x" big-size)))
        (let [got (get-plaintext-fn server)]
          (assert (= (length got) big-size)
                  (string "tls: server must decrypt all " big-size
                          " bytes, got " (length got))))

        ## ── close_notify ──────────────────────────────────────────

        (let [alert (close-notify-fn client)]
          (assert (> (length alert:outgoing) 0)
                  "tls/close-notify: encodes an alert to send")
          (assert (= (process-fn server alert:outgoing) :peer-closed)
                  "tls/process: a received close_notify reports :peer-closed"))

        (println "tls: loopback handshake, transfer and shutdown PASSED")))))

## ── ALPN ──────────────────────────────────────────────────────────
##
## RFC 7301. The client offers a list, the server picks from its own list,
## and both sides then report the same answer.

(with-temp-dir scratch
  (let [cert-path (path/join scratch "cert.pem")
        key-path (path/join scratch "key.pem")
        gen (self-signed cert-path key-path "localhost")]
    (if (not (= gen 0))
      (println "SKIP: openssl not available; ALPN tests skipped")
      (begin
        ## Nothing is agreed until the handshake has run.
        (assert (nil? (alpn-protocol-fn (client-state-fn "localhost")))
                "tls/alpn-protocol: nil before the handshake")

        ## Both sides offer h2 and both sides report it.
        (let [config (server-config-fn cert-path key-path {:alpn ["h2"]})
              [c s] (connected config {:no-verify true :alpn ["h2" "http/1.1"]})]
          (assert (= (alpn-protocol-fn c) "h2")
                  (string "tls/alpn-protocol: client expected h2, got "
                          (string (alpn-protocol-fn c))))
          (assert (= (alpn-protocol-fn s) "h2")
                  (string "tls/alpn-protocol: server expected h2, got "
                          (string (alpn-protocol-fn s)))))

        ## rustls picks by SERVER preference, not client preference: it walks
        ## its own list and takes the first entry the client also offered.
        ## The two lists are reversed here so the orders disagree.
        (let [config (server-config-fn cert-path key-path {:alpn ["h2" "http/1.1"]})
              [c _] (connected config {:no-verify true :alpn ["http/1.1" "h2"]})]
          (assert (= (alpn-protocol-fn c) "h2")
                  (string "tls/alpn-protocol: server preference wins, expected h2, got "
                          (string (alpn-protocol-fn c)))))

        ## A server that configures no protocols agrees to none.
        (let [config (server-config-fn cert-path key-path)
              [c s] (connected config {:no-verify true :alpn ["h2"]})]
          (assert (nil? (alpn-protocol-fn c))
                  "tls/alpn-protocol: nil when the server offers no protocols")
          (assert (nil? (alpn-protocol-fn s))
                  "tls/alpn-protocol: nil on the server too"))

        ## An empty client list sends no ALPN extension, so the server has
        ## nothing to select from and the handshake still succeeds.
        (let [config (server-config-fn cert-path key-path {:alpn ["h2"]})
              [c _] (connected config {:no-verify true :alpn []})]
          (assert (nil? (alpn-protocol-fn c))
                  "tls/alpn-protocol: nil when the client offers no protocols"))

        ## Disjoint lists are fatal — RFC 7301 §3.2 requires the
        ## no_application_protocol alert, which surfaces as a signal.
        (let [config (server-config-fn cert-path key-path {:alpn ["http/1.1"]})
              [ok? err] (protect (connected config {:no-verify true :alpn ["h2"]}))]
          (assert (not ok?) "tls: disjoint ALPN lists must fail the handshake")
          (assert (= (get err :error) :tls-error)
                  (string "tls: disjoint ALPN is :tls-error, got "
                          (string (get err :error)))))

        (println "tls: ALPN negotiation PASSED")))))

## ── Mutual TLS ────────────────────────────────────────────────────
##
## The server names a CA in :client-ca, which makes it demand a client
## certificate that the CA signed.

(with-temp-dir scratch
  (let [ca-cert (path/join scratch "ca.pem")
        ca-key (path/join scratch "ca.key")
        server-cert (path/join scratch "server.pem")
        server-key (path/join scratch "server.key")
        client-cert (path/join scratch "client.pem")
        client-key (path/join scratch "client.key")
        client-csr (path/join scratch "client.csr")
        ext-file (path/join scratch "client.ext")]
    ## webpki requires the clientAuth extended key usage on a client
    ## certificate, so sign the CSR with an extension file that sets it.
    (file/write ext-file "extendedKeyUsage=clientAuth\n")
    (let [status (+ (self-signed ca-cert ca-key "test-ca")
                    (self-signed server-cert server-key "localhost")
                    (openssl ["req" "-newkey" "rsa:2048" "-keyout" client-key
                              "-out" client-csr "-nodes" "-subj" "/CN=test-client"])
                    (openssl ["x509" "-req" "-in" client-csr
                              "-CA" ca-cert "-CAkey" ca-key
                              "-out" client-cert "-days" "1"
                              "-set_serial" "1" "-extfile" ext-file]))]
      (if (not (= status 0))
        (println "SKIP: openssl not available; mutual TLS tests skipped")
        (begin
          (def config (server-config-fn server-cert server-key
                                        {:client-ca ca-cert}))

          ## A client holding the signed certificate is admitted, and the
          ## server can see which certificate it presented.
          (let [[c s] (connected config {:no-verify true
                                         :client-cert client-cert
                                         :client-key client-key})]
            (assert (handshake-complete?-fn c)
                    "mutual TLS: the client completes the handshake")
            (assert (handshake-complete?-fn s)
                    "mutual TLS: the server completes the handshake")
            (assert (= (deliver c s (bytes "authenticated\n")) :has-data)
                    "mutual TLS: application data flows after the handshake")
            (assert (= (string (get-plaintext-fn s)) "authenticated\n")
                    "mutual TLS: the server reads what the client wrote"))

          ## A client with no certificate is refused.
          (let [[ok? err] (protect (connected config {:no-verify true}))]
            (assert (not ok?)
                    "mutual TLS: a client with no certificate must be refused")
            (assert (= (get err :error) :tls-error)
                    (string "mutual TLS: refusal is :tls-error, got "
                            (string (get err :error)))))

          ## A server without :client-ca asks for nothing, so the same
          ## bare client connects.
          (let [open-config (server-config-fn server-cert server-key)
                [c _] (connected open-config {:no-verify true})]
            (assert (handshake-complete?-fn c)
                    "mutual TLS: no :client-ca means no certificate is demanded"))

          (println "tls: mutual TLS PASSED"))))))

## ── Option validation ─────────────────────────────────────────────

(let [[ok? err] (protect (client-state-fn "example.com"
                                          {:client-cert "/nonexistent/c.pem"}))]
  (assert (not ok?) "tls/client-state: :client-cert alone must signal")
  (assert (= (get err :error) :value-error)
          (string "tls/client-state: :client-cert alone is :value-error, got "
                  (string (get err :error)))))

(let [[ok? err] (protect (client-state-fn "example.com"
                                          {:client-key "/nonexistent/k.pem"}))]
  (assert (not ok?) "tls/client-state: :client-key alone must signal")
  (assert (= (get err :error) :value-error)
          (string "tls/client-state: :client-key alone is :value-error, got "
                  (string (get err :error)))))

(let [[ok? err] (protect (client-state-fn "example.com"
                                          {:client-cert "/nonexistent/c.pem"
                                           :client-key "/nonexistent/k.pem"}))]
  (assert (not ok?) "tls/client-state: a missing client cert must signal")
  (assert (= (get err :error) :io-error)
          (string "tls/client-state: a missing client cert is :io-error, got "
                  (string (get err :error)))))

(let [[ok? err] (protect (client-state-fn "example.com" {:alpn "h2"}))]
  (assert (not ok?) "tls/client-state: a string :alpn must signal")
  (assert (= (get err :error) :type-error)
          (string "tls/client-state: a string :alpn is :type-error, got "
                  (string (get err :error)))))

(let [[ok? err] (protect (client-state-fn "example.com" {:alpn [""]}))]
  (assert (not ok?) "tls/client-state: an empty :alpn entry must signal")
  (assert (= (get err :error) :value-error)
          (string "tls/client-state: an empty :alpn entry is :value-error, got "
                  (string (get err :error)))))

(let [[ok? err] (protect (client-state-fn "example.com" {:alpn [1 2]}))]
  (assert (not ok?) "tls/client-state: a non-string :alpn entry must signal")
  (assert (= (get err :error) :type-error)
          (string "tls/client-state: a non-string :alpn entry is :type-error, got "
                  (string (get err :error)))))

(let [[ok? err] (protect (alpn-protocol-fn "not-a-state"))]
  (assert (not ok?) "tls/alpn-protocol: a string argument must signal")
  (assert (= (get err :error) :type-error)
          (string "tls/alpn-protocol: a string argument is :type-error, got "
                  (string (get err :error)))))

(println "tls: option validation PASSED")

## ── Error cases ───────────────────────────────────────────────────

(let [[ok? err] (protect (client-state-fn ""))]
  (assert (not ok?) "tls/client-state: empty hostname must signal")
  (assert (= (get err :error) :tls-error)
          (string "tls/client-state: empty hostname is :tls-error, got "
                  (string (get err :error)))))

(let [st (client-state-fn "example.com")]
  (let [[ok? err] (protect (process-fn st "not-bytes"))]
    (assert (not ok?) "tls/process: a string argument must signal")
    (assert (= (get err :error) :type-error)
            (string "tls/process: a string argument is :type-error, got "
                    (string (get err :error))))))

## Writing before the handshake is not a signal — it returns a struct, so
## callers can react without unwinding.
(let [st (client-state-fn "example.com")
      result (write-plaintext-fn st (bytes "hello"))]
  (assert (= result:status :error)
          (string "tls/write-plaintext: before the handshake status is :error, got "
                  (string result:status)))
  (assert (> (length result:message) 0)
          "tls/write-plaintext: an :error result carries a message"))

(let [[ok? err] (protect (server-config-fn "/nonexistent/cert.pem"
                                           "/nonexistent/key.pem"))]
  (assert (not ok?) "tls/server-config: a missing cert must signal")
  (assert (= (get err :error) :io-error)
          (string "tls/server-config: a missing cert is :io-error, got "
                  (string (get err :error)))))

(with-temp-dir scratch
  (let [cert-path (path/join scratch "cert.pem")
        key-path (path/join scratch "key.pem")]
    (when (= (self-signed cert-path key-path "localhost") 0)
      (let [[ok? err] (protect (server-config-fn cert-path key-path
                                                 {:client-ca "/nonexistent/ca.pem"}))]
        (assert (not ok?) "tls/server-config: a missing :client-ca must signal")
        (assert (= (get err :error) :io-error)
                (string "tls/server-config: a missing :client-ca is :io-error, got "
                        (string (get err :error))))))))

(println "tls: error cases PASSED")

## ── ALPN against a real server ────────────────────────────────────
##
## The in-process tests above pair this plugin with itself, so they cannot
## catch a ClientHello that other implementations reject. This section runs
## the same handshake over a socket against a public HTTP/2 endpoint.
##
## Sockets, but still no elle-side TLS library: tcp/connect gives a port and
## the state machine is driven by hand, exactly as lib/tls.lisp does it.

(def net-host "www.google.com")

(defn write-all [port data]
  "Send every byte. port/write issues one write(2) and returns what the
   kernel took, which on a socket is often less than the whole buffer."
  (let [@sent 0]
    (forever
      (when (>= sent (length data)) (break nil))
      (assign sent (+ sent (port/write port (slice data sent)))))))

(defn socket-handshake [port st]
  "Drive a handshake over a real socket to completion."
  (process-fn st (bytes))
  (let [@rounds 0]
    (forever
      (assign rounds (+ rounds 1))
      (when (> rounds max-rounds)
        (error {:error :test-error :message "socket handshake did not settle"}))
      ## INVARIANT: send queued ciphertext before reading, and check for
      ## completion only after sending — the peer needs our Finished.
      (let [out (get-outgoing-fn st)]
        (when (> (length out) 0) (write-all port out)))
      (when (handshake-complete?-fn st) (break nil))
      (let [data (port/read port 16384)]
        (when (nil? data)
          (error {:error :test-error
                  :message "peer closed during handshake"}))
        (process-fn st data)))))

(defn negotiate [opts]
  "Handshake against net-host with the given client options.
   Returns the protocol agreed via ALPN."
  (let [port (tcp/connect net-host 443)
        st (client-state-fn net-host opts)]
    (defer
      (port/close port)
      (socket-handshake port st)
      (assert (handshake-complete?-fn st)
              "tls: the handshake against a real server must complete")
      (alpn-protocol-fn st))))

(let [agreed (negotiate {:alpn ["h2" "http/1.1"]})]
  (assert (= agreed "h2")
          (string "tls: " net-host " must negotiate h2, got " (string agreed))))

## No :alpn option means the historical default, ["http/1.1"], which the
## same server answers with http/1.1.
(let [agreed (negotiate {})]
  (assert (= agreed "http/1.1")
          (string "tls: the default offer must negotiate http/1.1, got "
                  (string agreed))))

(println "tls: ALPN against a real server PASSED")
