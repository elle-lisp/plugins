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
(def pk   (hex->bytes "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"))
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

## ── HKDF-SHA256 ──────────────────────────────────────────────────
## Test cases 1 and 3 of RFC 5869 Appendix A. Case 3 has an empty salt and
## an empty info.

(def hkdf (get plugin :hkdf-sha256))

(assert (= (bytes->hex (hkdf (hex->bytes "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b")
                             (hex->bytes "000102030405060708090a0b0c")
                             (hex->bytes "f0f1f2f3f4f5f6f7f8f9")
                             42))
           (string "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
                   "34007208d5b887185865"))
        "hkdf-sha256 reproduces RFC 5869 test case 1")
(assert (= (bytes->hex (hkdf (hex->bytes "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b") "" "" 42))
           (string "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d"
                   "9d201395faa4b61a96c8"))
        "hkdf-sha256 reproduces RFC 5869 test case 3, with an empty salt and info")
(assert (= (length (hkdf "ikm" "salt" "info" 8160)) 8160) "hkdf-sha256 expands to 255 blocks")
(assert (= (length (hkdf "ikm" "salt" "info" 0)) 0) "hkdf-sha256 expands to nothing")
(let [[ok? err] (protect (hkdf "ikm" "salt" "info" 8161))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "hkdf-sha256: a length past 8160 is a range-error"))
(let [[ok? err] (protect (hkdf "ikm" "salt" "info" -1))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "hkdf-sha256: a negative length is a range-error"))
(let [[ok? err] (protect (hkdf "ikm" "salt" "info" "32"))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "hkdf-sha256: a length that is not an integer is a type-error"))
(let [[ok? err] (protect (hkdf 1 "salt" "info" 32))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "hkdf-sha256: ikm that is not bytes is a type-error"))
(let [[ok? err] (protect (hkdf "ikm" 1 "info" 32))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "hkdf-sha256: a salt that is not bytes is a type-error"))
(let [[ok? err] (protect (hkdf "ikm" "salt" 1 32))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "hkdf-sha256: info that is not bytes is a type-error"))

## ── X25519 ───────────────────────────────────────────────────────
## The Diffie-Hellman vector of RFC 7748 §6.1.

(def x-public (get plugin :x25519-public))
(def x-shared (get plugin :x25519))

(def alice-secret (hex->bytes "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a"))
(def alice-public "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
(def bob-secret   (hex->bytes "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb"))
(def bob-public   "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")
(def shared-k     "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742")

(assert (= (bytes->hex (x-public alice-secret)) alice-public) "x25519-public derives Alice's RFC 7748 public key")
(assert (= (bytes->hex (x-public bob-secret)) bob-public) "x25519-public derives Bob's RFC 7748 public key")
(assert (= (bytes->hex (x-shared alice-secret (hex->bytes bob-public))) shared-k)
        "x25519 gives Alice the RFC 7748 shared secret")
(assert (= (bytes->hex (x-shared bob-secret (hex->bytes alice-public))) shared-k)
        "x25519 gives Bob the RFC 7748 shared secret")

(def zero-32 (hex->bytes "0000000000000000000000000000000000000000000000000000000000000000"))
(let [[ok? err] (protect (x-shared alice-secret zero-32))]
  (assert (and (not ok?) (= (get err :error) :value-error))
          "x25519: a public key of small order is a value-error"))
(let [[ok? err] (protect (x-public (bytes 1 2 3)))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "x25519-public: a short secret is a range-error"))
(let [[ok? err] (protect (x-shared (bytes 1 2 3) (hex->bytes bob-public)))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "x25519: a short secret is a range-error"))
(let [[ok? err] (protect (x-shared alice-secret (bytes 1 2 3)))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "x25519: a short public key is a range-error"))
(let [[ok? err] (protect (x-public 42))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "x25519-public: a secret that is not bytes is a type-error"))

## ── XChaCha20-Poly1305 ───────────────────────────────────────────
## The vector of draft-irtf-cfrg-xchacha Appendix A.3.1.

(def seal (get plugin :xchacha20poly1305-seal))
(def open (get plugin :xchacha20poly1305-open))

(def xc-key   (hex->bytes "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f"))
(def xc-nonce (hex->bytes "404142434445464748494a4b4c4d4e4f5051525354555657"))
(def xc-aad   (hex->bytes "50515253c0c1c2c3c4c5c6c7"))
(def xc-plain "Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.")
(def xc-sealed (string "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb"
                       "731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452"
                       "2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9"
                       "21f9664c97637da9768812f615c68b13b52e"
                       "c0875924c1c7987947deafd8780acf49"))

(assert (= (bytes->hex (seal xc-key xc-nonce xc-plain xc-aad)) xc-sealed)
        "xchacha20poly1305-seal reproduces the draft-irtf-cfrg-xchacha ciphertext and tag")
(assert (= (bytes->hex (open xc-key xc-nonce (hex->bytes xc-sealed) xc-aad))
           (bytes->hex (bytes xc-plain)))
        "xchacha20poly1305-open recovers the plaintext")
(assert (= (bytes->hex (open xc-key xc-nonce (seal xc-key xc-nonce "" "") ""))
           "")
        "an empty message round-trips")

(defn flip-first [b]
  "b with its first byte inverted."
  (let [out (thaw b)]
    (put out 0 (bit/xor (get out 0) 255))
    (freeze out)))

(assert (nil? (open xc-key xc-nonce (flip-first (hex->bytes xc-sealed)) xc-aad))
        "xchacha20poly1305-open answers nil for a changed ciphertext")
(assert (nil? (open (flip-first xc-key) xc-nonce (hex->bytes xc-sealed) xc-aad))
        "xchacha20poly1305-open answers nil under another key")
(assert (nil? (open xc-key (flip-first xc-nonce) (hex->bytes xc-sealed) xc-aad))
        "xchacha20poly1305-open answers nil under another nonce")
(assert (nil? (open xc-key xc-nonce (hex->bytes xc-sealed) "other"))
        "xchacha20poly1305-open answers nil for other associated data")
(assert (nil? (open xc-key xc-nonce (bytes 1 2 3) xc-aad))
        "xchacha20poly1305-open answers nil for input shorter than a tag")

(let [[ok? err] (protect (seal (bytes 1 2 3) xc-nonce "m" ""))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "xchacha20poly1305-seal: a short key is a range-error"))
(let [[ok? err] (protect (seal xc-key (bytes 1 2 3) "m" ""))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "xchacha20poly1305-seal: a short nonce is a range-error"))
(let [[ok? err] (protect (open (bytes 1 2 3) xc-nonce (hex->bytes xc-sealed) ""))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "xchacha20poly1305-open: a short key is a range-error"))
(let [[ok? err] (protect (open xc-key (bytes 1 2 3) (hex->bytes xc-sealed) ""))]
  (assert (and (not ok?) (= (get err :error) :range-error)) "xchacha20poly1305-open: a short nonce is a range-error"))
(let [[ok? err] (protect (seal xc-key xc-nonce 42 ""))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "xchacha20poly1305-seal: a plaintext that is not bytes is a type-error"))
(let [[ok? err] (protect (seal xc-key xc-nonce "m" 42))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "xchacha20poly1305-seal: aad that is not bytes is a type-error"))
(let [[ok? err] (protect (open xc-key xc-nonce 42 ""))]
  (assert (and (not ok?) (= (get err :error) :type-error)) "xchacha20poly1305-open: a ciphertext that is not bytes is a type-error"))
