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
(def close-notify-fn        (get plugin :close-notify))

## ── The registered primitive table ────────────────────────────────

(each k in [:client-state :server-config :server-state :process
            :get-outgoing :get-plaintext :read-plaintext
            :plaintext-indexof :handshake-complete? :write-plaintext
            :close-notify]
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

## ── Loopback: handshake, data transfer, shutdown ──────────────────
##
## Needs a certificate, so it needs openssl. Skip the section if openssl is
## missing rather than failing the whole file.

(with-temp-dir scratch
  (let [cert-path (path/join scratch "cert.pem")
        key-path (path/join scratch "key.pem")
        gen (subprocess/system "openssl"
                               ["req" "-x509" "-newkey" "rsa:2048"
                                "-keyout" key-path "-out" cert-path
                                "-days" "1" "-nodes" "-subj" "/CN=localhost"])]
    (if (not (= gen:exit 0))
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

(println "tls: error cases PASSED")
