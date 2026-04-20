
## CSV plugin integration tests

## Try to load the csv plugin. If it fails, exit cleanly.
(def [ok? plugin] (protect (import-file "target/release/libelle_csv.so")))
(when (not ok?)
  (print "SKIP: csv plugin not built\n")
  (exit 0))

## Extract plugin functions from the returned struct
(def parse-fn      (get plugin :parse))
(def parse-rows-fn (get plugin :parse-rows))
(def write-fn      (get plugin :write))
(def write-rows-fn (get plugin :write-rows))

## ── csv/parse ───────────────────────────────────────────────────

(def result (parse-fn "name,age\nAlice,30\nBob,25"))

(assert (= (length result) 2) "csv/parse row count")

(assert (= (get (get result 0) :name) "Alice") "csv/parse first name")

(assert (= (get (get result 0) :age) "30") "csv/parse age is string")

(assert (= (get (get result 1) :name) "Bob") "csv/parse second name")

## ── csv/parse-rows ──────────────────────────────────────────────

(def raw (parse-rows-fn "a,b,c\n1,2,3"))

(assert (= (length raw) 2) "csv/parse-rows row count")

(assert (= (get (get raw 0) 0) "a") "csv/parse-rows first field")

(assert (= (get (get raw 1) 1) "2") "csv/parse-rows second row second field")

## ── csv/write roundtrip ────────────────────────────────────────

(def input-structs [{:age "30" :name "Alice"} {:age "25" :name "Bob"}])
(def written (write-fn input-structs))

(assert (string? written) "csv/write returns string")

## Roundtrip: parse the written CSV back and verify fields
(def roundtrip (parse-fn written))
(assert (= (length roundtrip) 2) "csv/write roundtrip row count")
(assert (= (get (get roundtrip 0) :name) "Alice") "csv/write roundtrip first name")
(assert (= (get (get roundtrip 0) :age) "30") "csv/write roundtrip first age")
(assert (= (get (get roundtrip 1) :name) "Bob") "csv/write roundtrip second name")
(assert (= (get (get roundtrip 1) :age) "25") "csv/write roundtrip second age")

## ── csv/write-rows roundtrip ──────────────────────────────────

(def rows-text (write-rows-fn [["a" "b"] ["1" "2"]]))

(assert (string? rows-text) "csv/write-rows returns string")

## Roundtrip: parse-rows the written CSV back and verify fields
(def rows-rt (parse-rows-fn rows-text))
(assert (= (length rows-rt) 2) "csv/write-rows roundtrip row count")
(assert (= (get (get rows-rt 0) 0) "a") "csv/write-rows roundtrip r0c0")
(assert (= (get (get rows-rt 0) 1) "b") "csv/write-rows roundtrip r0c1")
(assert (= (get (get rows-rt 1) 0) "1") "csv/write-rows roundtrip r1c0")
(assert (= (get (get rows-rt 1) 1) "2") "csv/write-rows roundtrip r1c1")

## ── custom delimiter (tab) ──────────────────────────────────────

(def tsv (parse-fn "name\tage\nAlice\t30" {:delimiter "\t"}))

(assert (= (length tsv) 1) "csv/parse tab delimiter row count")

(assert (= (get (get tsv 0) :name) "Alice") "csv/parse tab delimiter name")

(assert (= (get (get tsv 0) :age) "30") "csv/parse tab delimiter age")

## ── tab delimiter write-rows roundtrip ─────────────────────────

(def tsv-out (write-rows-fn [["x" "y"] ["1" "2"]] {:delimiter "\t"}))

(assert (string? tsv-out) "csv/write-rows tab delimiter returns string")

## Roundtrip: parse-rows with tab delimiter
(def tsv-rt (parse-rows-fn tsv-out {:delimiter "\t"}))
(assert (= (length tsv-rt) 2) "csv/write-rows tab roundtrip row count")
(assert (= (get (get tsv-rt 0) 0) "x") "csv/write-rows tab roundtrip r0c0")
(assert (= (get (get tsv-rt 0) 1) "y") "csv/write-rows tab roundtrip r0c1")
(assert (= (get (get tsv-rt 1) 0) "1") "csv/write-rows tab roundtrip r1c0")
(assert (= (get (get tsv-rt 1) 1) "2") "csv/write-rows tab roundtrip r1c1")

## ── tab delimiter csv/write roundtrip ─────────────────────────

(def tsv-structs (parse-fn "name\tage\nCarol\t40" {:delimiter "\t"}))
(def tsv-written (write-fn tsv-structs {:delimiter "\t"}))
(def tsv-structs-rt (parse-fn tsv-written {:delimiter "\t"}))
(assert (= (length tsv-structs-rt) 1) "csv/write tab roundtrip row count")
(assert (= (get (get tsv-structs-rt 0) :name) "Carol") "csv/write tab roundtrip name")
(assert (= (get (get tsv-structs-rt 0) :age) "40") "csv/write tab roundtrip age")

## ── empty input ─────────────────────────────────────────────────

(def empty-result (parse-fn "name,age"))

(assert (= (length empty-result) 0) "csv/parse headers-only = empty result")

## ── empty write ───────────────────────────────────────────────

(def empty-write (write-fn []))
(assert (string? empty-write) "csv/write empty array returns string")

(def empty-write-rows (write-rows-fn []))
(assert (string? empty-write-rows) "csv/write-rows empty array returns string")

## ── error cases ─────────────────────────────────────────────────

(let ([ok? _] (protect ((fn () (parse-fn 42))))) (assert (not ok?) "csv/parse wrong type"))

(let ([ok? _] (protect ((fn () (parse-rows-fn 42))))) (assert (not ok?) "csv/parse-rows wrong type"))

(let ([ok? _] (protect ((fn () (write-fn 42))))) (assert (not ok?) "csv/write non-array"))

(let ([ok? _] (protect ((fn () (write-rows-fn 42))))) (assert (not ok?) "csv/write-rows non-array"))

(let ([ok? _] (protect ((fn () (write-fn ["not-a-struct"]))))) (assert (not ok?) "csv/write non-struct row"))

(let ([ok? _] (protect ((fn () (write-rows-fn ["not-an-array"]))))) (assert (not ok?) "csv/write-rows non-array row"))
