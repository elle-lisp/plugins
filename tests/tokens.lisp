(elle/epoch 18)

## Tokens plugin integration tests
##
## Every id below was read out of the encoding's vocabulary file, one line
## per token, apart from the encoder. The counts of the Rust line and of
## `<|endoftext|>` were measured apart from this plugin. A count that split
## on whitespace, or counted bytes, fails them.
##
## The trap is the special token: an encoder that honors `<|endoftext|>`
## prices that spelling as one token, so a text that quotes it would cost
## less than its reader pays.
##
## CI builds every plugin before it runs a test, so the import does not
## skip: a skip would only hide a plugin that failed to build.

(def tokens (import "target/release/libelle_tokens.so"))

(defn raised [thunk]
  (def [ok? err] (protect (thunk)))
  (when (not ok?) (get err :error)))

(defn member? [x coll]
  (= true
     (each y in coll
       (when (= x y) (break true)))))

## ── o200k_base is the default ──────────────────────────────────

(assert (= (tokens:encode "hello world") [24912 2375])
        "hello and \" world\" are o200k_base tokens 24912 and 2375")
(assert (= (tokens:encode "hello world" :o200k_base) [24912 2375])
        ":o200k_base names the default")
(assert (= (tokens:count "hello world") 2) "hello world is two tokens")
(assert (= (tokens:count "") 0) "an empty text is no tokens")
(assert (= (tokens:encode "") []) "an empty text encodes to an empty array")

(def rust "let walked = reach.charge(edge, pace);\n")
(assert (= (tokens:count rust) 10) "a line of Rust is ten o200k_base tokens")
(assert (= (tokens:count rust) (length (tokens:encode rust)))
        "count is the length of encode")

## ── each encoding reads its own vocabulary ─────────────────────

(assert (= (tokens:encode "hello world" :cl100k_base) [15339 1917])
        "cl100k_base has its own ids")
(assert (= (tokens:encode "hello world" :r50k_base) [31373 995])
        "r50k_base has its own ids")
(assert (= (tokens:encode "hello world" :p50k_base) [31373 995])
        "p50k_base shares r50k_base's ids for words")
(assert (= (tokens:encode "hello world" :o200k_harmony) [24912 2375])
        "o200k_harmony encodes text as o200k_base does")

## p50k_base adds a token for a run of spaces, and r50k_base has none: its
## only token of spaces is the single space, 220.
(assert (= (tokens:encode "    " :p50k_base) [50259])
        "p50k_base has a token of four spaces")
(assert (= (tokens:encode "    " :p50k_edit) [50259])
        "p50k_edit encodes text as p50k_base does")
(assert (= (tokens:encode "    " :r50k_base) [220 220 220 220])
        "r50k_base spends a token on each space")
(assert (= (tokens:count "    " :r50k_base) 4)
        "count names its encoding as encode does")

(assert (= (tokens:encodings)
           |:o200k_base :o200k_harmony :cl100k_base :p50k_base :p50k_edit
            :r50k_base|) "encodings names every encoding")

## ── special tokens ─────────────────────────────────────────────

(assert (= (tokens:count "<|endoftext|>") 7)
        "the spelling of a special token counts as the text it is")
(assert (not (member? 199999 (tokens:encode "<|endoftext|>")))
        "the spelling of a special token never encodes to its id")
(assert (= (tokens:decode [199999]) "<|endoftext|>")
        "decode knows o200k_base's special ids")
(assert (= (tokens:decode [100257] :cl100k_base) "<|endoftext|>")
        "decode knows cl100k_base's special ids")
(assert (= (tokens:decode [50281] :p50k_edit) "<|fim_prefix|>")
        "decode knows p50k_edit's fill-in-the-middle ids")
(assert (= (raised (fn [] (tokens:decode [50281] :p50k_base))) :value-error)
        "p50k_base has no fill-in-the-middle ids")
(assert (= (tokens:decode [200002] :o200k_harmony) "<|return|>")
        "decode knows o200k_harmony's special ids")
(assert (= (raised (fn [] (tokens:decode [200002]))) :value-error)
        "o200k_base has no harmony ids")

## ── decode ─────────────────────────────────────────────────────

(assert (= (tokens:decode [24912 2375]) "hello world") "decode inverts encode")
(assert (= (tokens:decode (list 24912 2375)) "hello world")
        "decode takes a list")
(assert (= (tokens:decode [15339 1917] :cl100k_base) "hello world")
        "decode names its encoding")
(assert (= (tokens:decode []) "") "no ids decode to an empty text")

(def mixed "é and 漢字 🙂 <|endoftext|>\n\ttabs  and  spaces")
(each e in [:o200k_base :o200k_harmony :cl100k_base :p50k_base :p50k_edit
            :r50k_base]
  (assert (= (tokens:decode (tokens:encode mixed e) e) mixed)
          (string "decode inverts encode in " e)))

## No r50k_base token holds all four bytes of 🙂, so its first id holds part
## of the character, and those bytes alone are no UTF-8.
(def smile (tokens:encode "🙂" :r50k_base))
(assert (> (length smile) 1) "r50k_base spends more than one token on 🙂")
(assert (= (raised (fn [] (tokens:decode (slice smile 0 1) :r50k_base)))
           :value-error) "ids that cut a character in two are a value-error")

## ── errors ─────────────────────────────────────────────────────

(assert (= (raised (fn [] (tokens:count 42))) :type-error)
        "count: a text that is not a string")
(assert (= (raised (fn [] (tokens:encode 42))) :type-error)
        "encode: a text that is not a string")
(assert (= (raised (fn [] (tokens:count "a" "o200k_base"))) :type-error)
        "an encoding that is not a keyword is a type-error")
(assert (= (raised (fn [] (tokens:encode "a" :o200k))) :value-error)
        "a keyword that names no encoding is a value-error")
(assert (= (raised (fn [] (tokens:decode [1] :o200k))) :value-error)
        "decode: a keyword that names no encoding is a value-error")
(assert (= (raised (fn [] (tokens:decode "hello"))) :type-error)
        "ids that are not an array or a list are a type-error")
(assert (= (raised (fn [] (tokens:decode ["a"]))) :type-error)
        "an id that is not an integer")
(assert (= (raised (fn [] (tokens:decode [1.5]))) :type-error)
        "an id that is a float")
(assert (= (raised (fn [] (tokens:decode [-1]))) :value-error)
        "a negative id names no token")
(assert (= (raised (fn [] (tokens:decode [4294967296]))) :value-error)
        "an id past 32 bits names no token")
(assert (= (raised (fn [] (tokens:decode [300000]))) :value-error)
        "an id past the vocabulary names no token")
(assert (= (raised (fn [] (tokens:count))) :arity-error) "count needs a text")
(assert (= (raised (fn [] (tokens:count "a" :o200k_base 1))) :arity-error)
        "count takes at most a text and an encoding")
