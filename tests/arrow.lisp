(elle/epoch 6)

## Arrow plugin integration tests

## Try to load the arrow plugin. If it fails, exit cleanly.
(def [ok? plugin] (protect (import-file "target/release/libelle_arrow.so")))
(when (not ok?)
  (print "SKIP: arrow plugin not built\n")
  (exit 0))

(def batch-fn     (get plugin :batch))
(def schema-fn    (get plugin :schema))
(def num-rows-fn  (get plugin :num-rows))
(def num-cols-fn  (get plugin :num-cols))
(def column-fn    (get plugin :column))
(def to-rows-fn   (get plugin :to-rows))

## ── A column named by a keyword ────────────────────────────────
##
## `arrow/batch` accepts a column name as a string or as a keyword. The
## keyword is a bare name hash, and the spelling behind it lives in the
## memo of the calling instance, which the primitive reaches through its
## per-call ctx. Read against the wrong ctx the spelling comes back empty,
## the name is neither string nor resolvable keyword, and the batch is
## rejected — so these assertions are what fails when the ctx is not
## threaded.

(def mixed (batch-fn [[:alpha [1 2 3]] ["beta" [4 5 6]]]))

(assert (= (num-rows-fn mixed) 3) "arrow/batch keeps the row count")
(assert (= (num-cols-fn mixed) 2) "arrow/batch keeps the column count")

## The keyword name and the string name are the same name.
(assert (= (schema-fn mixed) {:alpha "Int64" :beta "Int64"})
        "a keyword column name and a string column name land alike")

(assert (= (column-fn mixed "alpha") [1 2 3])
        "a keyword-named column answers to its spelling")

(assert (= (to-rows-fn mixed)
           [{:alpha 1 :beta 4} {:alpha 2 :beta 5} {:alpha 3 :beta 6}])
        "keyword and string column names round-trip through to-rows")

## Several keyword names, so this is not one spelling that happens to be
## in the runtime's static vocabulary already.
(def all-kw (batch-fn [[:quux [1]] [:zork [2]] [:frob [3]]]))
(assert (= (schema-fn all-kw) {:frob "Int64" :quux "Int64" :zork "Int64"})
        "every keyword column name carries its own spelling")

## ── Types and access ───────────────────────────────────────────

(def typed (batch-fn [[:n [1 2]] [:x [1.5 2.5]] [:s ["a" "b"]]]))
(assert (= (schema-fn typed) {:n "Int64" :s "Utf8" :x "Float64"})
        "arrow/schema reports a type per column")
(assert (= (column-fn typed "x") [1.5 2.5]) "float column reads back")
(assert (= (column-fn typed "s") ["a" "b"]) "string column reads back")

## A column that was never named is an error, not an empty answer.
(def [col-ok? _] (protect (column-fn typed "nope")))
(assert (not col-ok?) "an unknown column name is an error")

(print "arrow plugin tests passed\n")
