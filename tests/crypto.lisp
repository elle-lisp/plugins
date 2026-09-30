(elle/epoch 8)

## Crypto plugin integration tests
## Tests the crypto plugin (.so loaded via import-file)

## Try to load the crypto plugin. If it fails, exit cleanly.
(def [ok? plugin] (protect (import-file "target/release/libelle_crypto.so")))
(when (not ok?)
  (print "SKIP: crypto plugin not built\n")
  (exit 0))

## Extract plugin functions from the returned struct
(def sha256-fn      (get plugin :sha256))
(def hmac-sha256-fn (get plugin :hmac-sha256))

## ── crypto/sha256 ──────────────────────────────────────────────

(assert (= (bytes->hex (sha256-fn "")) "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855") "sha256 empty string")

(assert (= (bytes->hex (sha256-fn "hello")) "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824") "sha256 hello")

## ── crypto/hmac-sha256 ────────────────────────────────────────

(assert (= (bytes->hex (hmac-sha256-fn "key" "message")) "6e9ef29b75fffc5b7abae527d58fdadb2fe42e7219011976917343065f58ed4a") "hmac-sha256 key message")

## ── Ed25519 ──────────────────────────────────────────────────────
## The first test vector of RFC 8032 §7.1: an empty message under a fixed seed.

(def ed-public (get plugin :ed25519-public))
(def ed-sign   (get plugin :ed25519-sign))
(def ed-verify (get plugin :ed25519-verify))

(defn hex->bytes [h]
  (let [out @[]]
    (each i in (range (/ (length h) 2))
      (push out (parse-int (slice h (* 2 i) (+ 2 (* 2 i))) 16)))
    (apply bytes (->list (freeze out)))))

(def seed (hex->bytes "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"))
(def pk   (hex->bytes "d75a980182b10ab7d54bfed3c964073a0ee172f3daec9ee2be48c7f2e9d7fb6f"))
(def sig  (hex->bytes (string "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a3"
                              "3bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b")))

(assert (= (bytes->hex (ed-public seed)) (bytes->hex pk)) "ed25519-public derives the RFC 8032 public key")
(assert (= (bytes->hex (ed-sign seed "")) (bytes->hex sig)) "ed25519-sign reproduces the RFC 8032 signature")
(assert (= (ed-verify pk "" sig) true) "ed25519-verify accepts the RFC 8032 signature")
(assert (= (ed-verify pk "x" sig) false) "ed25519-verify refuses the signature over another message")
(assert (= (ed-verify pk "" (ed-sign (hex->bytes "0000000000000000000000000000000000000000000000000000000000000000") "")) false)
        "ed25519-verify refuses a signature under another key")

## A signature over a longer message round-trips, and bytes and strings
## are the same message.
(let [m "the wallet ledger entry"]
  (assert (= (ed-verify (ed-public seed) (bytes 116 104 101) (ed-sign seed "the")) true)
          "a message given as bytes verifies against the same message as a string")
  (assert (= (ed-verify (ed-public seed) m (ed-sign seed m)) true) "a longer message round-trips"))

## Length rules: a seed, a public key and a signature have fixed sizes.
(let [[ok? err] (protect (ed-public (bytes 1 2 3)))]
  (assert (not ok?) "ed25519-public: a short seed is refused")
  (assert (= (get err :error) :range-error) "ed25519-public: a short seed is a range-error"))
(let [[ok? err] (protect (ed-sign (bytes 1 2 3) "m"))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "ed25519-sign: a short seed is a range-error"))
(let [[ok? err] (protect (ed-verify (bytes 1 2 3) "m" sig))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "ed25519-verify: a short public key is a range-error"))
(let [[ok? err] (protect (ed-verify pk "m" (bytes 1 2 3)))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "ed25519-verify: a short signature is a range-error"))
(let [[ok? err] (protect (ed-sign 42 "m"))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "ed25519-sign: a seed that is not bytes is a type-error"))
