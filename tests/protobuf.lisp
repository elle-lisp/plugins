(elle/epoch 6)

## protobuf plugin integration tests

## Try to load the protobuf plugin. If it fails, exit cleanly.
(def [ok? plugin] (protect (import-file "target/release/libelle_protobuf.so")))
(when (not ok?)
  (print "SKIP: protobuf plugin not built\n")
  (exit 0))

## Extract plugin functions from the returned struct
(def schema-fn   (get plugin :schema))
(def schema-bytes-fn (get plugin :schema-bytes))
(def encode-fn   (get plugin :encode))
(def decode-fn   (get plugin :decode))
(def messages-fn (get plugin :messages))
(def fields-fn   (get plugin :fields))
(def enums-fn    (get plugin :enums))

## ── Schema definition ────────────────────────────────────────────────

## Full schema covering all scalar types, enums, nested messages,
## repeated fields, map fields, and bytes.
(def test-proto "
syntax = \"proto3\";

enum Status {
  UNKNOWN = 0;
  OK = 1;
  ERROR = 2;
}

message Scalars {
  int32 i32 = 1;
  int64 i64 = 2;
  uint32 u32 = 3;
  uint64 u64 = 4;
  sint32 si32 = 5;
  sint64 si64 = 6;
  fixed32 fx32 = 7;
  fixed64 fx64 = 8;
  sfixed32 sfx32 = 9;
  sfixed64 sfx64 = 10;
  float f = 11;
  double d = 12;
  bool b = 13;
  string s = 14;
  bytes raw = 15;
}

message Person {
  string name = 1;
  int32 age = 2;
  repeated string tags = 3;
  Status status = 4;
  map<string, int32> scores = 5;
}

message Team {
  string team_name = 1;
  repeated Person members = 2;
}

message Repeats {
  repeated int32 ints = 1;
  repeated bool bools = 2;
  repeated double doubles = 3;
  repeated bytes blobs = 4;
  repeated Status statuses = 5;
  repeated Person people = 6;
}

message Maps {
  map<string, string> ss = 1;
  map<int32, string> is = 2;
  map<bool, int32> bi = 3;
  map<int64, bool> lb = 4;
  map<uint32, string> us = 5;
}

message Nested {
  Person person = 1;
  Scalars scalars = 2;
}

message Empty {
}
")

(def pool (schema-fn test-proto))

## ── protobuf/messages ────────────────────────────────────────────────

(def msgs (messages-fn pool))

(assert (array? msgs) "protobuf/messages returns array")
(assert (> (length msgs) 0) "protobuf/messages returns non-empty array")

## Helper: check if array contains a value
(def contains? (fn (arr val)
  (letrec [(check (fn (i)
    (if (>= i (length arr)) false
      (if (= (get arr i) val) true
        (check (+ i 1))))))]
  (check 0))))

(assert (contains? msgs "Person") "protobuf/messages includes Person")
(assert (contains? msgs "Team") "protobuf/messages includes Team")
(assert (contains? msgs "Scalars") "protobuf/messages includes Scalars")
(assert (contains? msgs "Repeats") "protobuf/messages includes Repeats")
(assert (contains? msgs "Maps") "protobuf/messages includes Maps")
(assert (contains? msgs "Nested") "protobuf/messages includes Nested")
(assert (contains? msgs "Empty") "protobuf/messages includes Empty")

## ── protobuf/fields ─────────────────────────────────────────────────

(def person-fields (fields-fn pool "Person"))

(assert (array? person-fields) "protobuf/fields returns array")
(assert (= (length person-fields) 5) "Person has 5 fields")

## Find a field by name in the fields array
(def find-field (fn (fields name)
  (letrec [(search (fn (i)
    (if (>= i (length fields)) nil
      (if (= (get (get fields i) :name) name)
        (get fields i)
        (search (+ i 1))))))]
  (search 0))))

(def f-name   (find-field person-fields "name"))
(def f-age    (find-field person-fields "age"))
(def f-tags   (find-field person-fields "tags"))
(def f-status (find-field person-fields "status"))
(def f-scores (find-field person-fields "scores"))

(assert (not (nil? f-name)) "Person has field 'name'")
(assert (not (nil? f-age)) "Person has field 'age'")
(assert (not (nil? f-tags)) "Person has field 'tags'")
(assert (not (nil? f-status)) "Person has field 'status'")
(assert (not (nil? f-scores)) "Person has field 'scores'")

(assert (= (get f-name   :type) :string) "name field type is string")
(assert (= (get f-age    :type) :int32) "age field type is int32")
(assert (= (get f-tags   :label) :repeated) "tags field label is repeated")
(assert (= (get f-status :type) :enum) "status field type is enum")
(assert (= (get f-scores :type) :message) "scores field type is message (map entry)")

(assert (= (get f-name :number) 1) "name field number is 1")
(assert (= (get f-age  :number) 2) "age field number is 2")
(assert (= (get f-tags :number) 3) "tags field number is 3")

## Scalars fields
(def scalar-fields (fields-fn pool "Scalars"))
(assert (= (length scalar-fields) 15) "Scalars has 15 fields")

## Empty message has no fields
(def empty-fields (fields-fn pool "Empty"))
(assert (= (length empty-fields) 0) "Empty has 0 fields")

## ── protobuf/enums ──────────────────────────────────────────────────

(def enums (enums-fn pool))

(assert (array? enums) "protobuf/enums returns array")
(assert (> (length enums) 0) "protobuf/enums returns non-empty result")

## Find the Status enum
(def find-enum (fn (enums name)
  (letrec [(search (fn (i)
    (if (>= i (length enums)) nil
      (if (= (get (get enums i) :name) name)
        (get enums i)
        (search (+ i 1))))))]
  (search 0))))

(def status-enum (find-enum enums "Status"))

(assert (not (nil? status-enum)) "protobuf/enums includes Status")

(def status-values (get status-enum :values))
(assert (= (length status-values) 3) "Status has 3 values")

## Find enum value by name
(def find-enum-val (fn (values name)
  (letrec [(search (fn (i)
    (if (>= i (length values)) nil
      (if (= (get (get values i) :name) name)
        (get values i)
        (search (+ i 1))))))]
  (search 0))))

(def v-unknown (find-enum-val status-values "UNKNOWN"))
(def v-ok      (find-enum-val status-values "OK"))
(def v-error   (find-enum-val status-values "ERROR"))

(assert (not (nil? v-unknown)) "Status has UNKNOWN value")
(assert (not (nil? v-ok)) "Status has OK value")
(assert (not (nil? v-error)) "Status has ERROR value")

(assert (= (get v-unknown :number) 0) "UNKNOWN = 0")
(assert (= (get v-ok      :number) 1) "OK = 1")
(assert (= (get v-error   :number) 2) "ERROR = 2")

## ── Round-trip: all scalar types ────────────────────────────────────

(def scalar-val {:i32 -42 :i64 1000000000000 :u32 4000000000 :u64 9000000000000
                 :si32 -100 :si64 -999999999999 :fx32 123456 :fx64 7890123456
                 :sfx32 -54321 :sfx64 -1234567890
                 :f 3.14 :d 2.718281828
                 :b true :s "hello world"
                 :raw (bytes 0 1 2 255)})
(def scalar-buf (encode-fn pool "Scalars" scalar-val))
(assert (bytes? scalar-buf) "Scalars encode returns bytes")

(def scalar-dec (decode-fn pool "Scalars" scalar-buf))
(assert (struct? scalar-dec) "Scalars decode returns struct")

(assert (= (get scalar-dec :i32) -42) "Scalars round-trip: i32")
(assert (= (get scalar-dec :i64) 1000000000000) "Scalars round-trip: i64")
(assert (= (get scalar-dec :u32) 4000000000) "Scalars round-trip: u32")
(assert (= (get scalar-dec :u64) 9000000000000) "Scalars round-trip: u64")
(assert (= (get scalar-dec :si32) -100) "Scalars round-trip: si32")
(assert (= (get scalar-dec :si64) -999999999999) "Scalars round-trip: si64")
(assert (= (get scalar-dec :fx32) 123456) "Scalars round-trip: fx32")
(assert (= (get scalar-dec :fx64) 7890123456) "Scalars round-trip: fx64")
(assert (= (get scalar-dec :sfx32) -54321) "Scalars round-trip: sfx32")
(assert (= (get scalar-dec :sfx64) -1234567890) "Scalars round-trip: sfx64")
(assert (= (get scalar-dec :b) true) "Scalars round-trip: bool")
(assert (= (get scalar-dec :s) "hello world") "Scalars round-trip: string")
(assert (= (get scalar-dec :raw) (bytes 0 1 2 255)) "Scalars round-trip: bytes")

## Float comparisons (approximate)
(assert (< (abs (- (get scalar-dec :d) 2.718281828)) 0.0001) "Scalars round-trip: double")
## float is f32, so less precision
(def f-decoded (get scalar-dec :f))
(assert (< (abs (- f-decoded 3.14)) 0.01) "Scalars round-trip: float (f32 precision)")

## ── Round-trip: simple Person ───────────────────────────────────────

(def alice {:name "Alice" :age 30 :tags ["dev" "lisp"]})
(def alice-buf (encode-fn pool "Person" alice))

(assert (bytes? alice-buf) "protobuf/encode returns bytes")
(assert (> (length alice-buf) 0) "encoded bytes are non-empty")

(def alice-decoded (decode-fn pool "Person" alice-buf))

(assert (struct? alice-decoded) "protobuf/decode returns struct")
(assert (= (get alice-decoded :name) "Alice") "Person round-trip: name")
(assert (= (get alice-decoded :age) 30) "Person round-trip: age")
(assert (= (length (get alice-decoded :tags)) 2) "Person round-trip: tags length")
(assert (= (get (get alice-decoded :tags) 0) "dev") "Person round-trip: tags[0]")
(assert (= (get (get alice-decoded :tags) 1) "lisp") "Person round-trip: tags[1]")

## ── Repeated fields with arrays ─────────────────────────────────────

(def rep-arr {:ints [10 20 30 -5]
              :bools [true false true]
              :doubles [1.1 2.2 3.3]
              :statuses [:OK :ERROR :UNKNOWN]})
(def rep-arr-buf (encode-fn pool "Repeats" rep-arr))
(def rep-arr-dec (decode-fn pool "Repeats" rep-arr-buf))

(assert (= (length (get rep-arr-dec :ints)) 4) "repeated int32 array: length")
(assert (= (get (get rep-arr-dec :ints) 0) 10) "repeated int32 array: [0]")
(assert (= (get (get rep-arr-dec :ints) 1) 20) "repeated int32 array: [1]")
(assert (= (get (get rep-arr-dec :ints) 2) 30) "repeated int32 array: [2]")
(assert (= (get (get rep-arr-dec :ints) 3) -5) "repeated int32 array: [3]")

(assert (= (length (get rep-arr-dec :bools)) 3) "repeated bool array: length")
(assert (= (get (get rep-arr-dec :bools) 0) true) "repeated bool array: [0]")
(assert (= (get (get rep-arr-dec :bools) 1) false) "repeated bool array: [1]")
(assert (= (get (get rep-arr-dec :bools) 2) true) "repeated bool array: [2]")

(assert (= (length (get rep-arr-dec :doubles)) 3) "repeated double array: length")

(assert (= (length (get rep-arr-dec :statuses)) 3) "repeated enum array: length")
(assert (= (get (get rep-arr-dec :statuses) 0) :OK) "repeated enum array: [0]")
(assert (= (get (get rep-arr-dec :statuses) 1) :ERROR) "repeated enum array: [1]")
(assert (= (get (get rep-arr-dec :statuses) 2) :UNKNOWN) "repeated enum array: [2]")

## ── Repeated fields with lists (cons chains) ────────────────────────

(def rep-list {:ints (list 100 200 300)
               :bools (list false true)
               :doubles (list 9.9 8.8)})
(def rep-list-buf (encode-fn pool "Repeats" rep-list))
(def rep-list-dec (decode-fn pool "Repeats" rep-list-buf))

(assert (= (length (get rep-list-dec :ints)) 3) "repeated int32 list: length")
(assert (= (get (get rep-list-dec :ints) 0) 100) "repeated int32 list: [0]")
(assert (= (get (get rep-list-dec :ints) 1) 200) "repeated int32 list: [1]")
(assert (= (get (get rep-list-dec :ints) 2) 300) "repeated int32 list: [2]")

(assert (= (length (get rep-list-dec :bools)) 2) "repeated bool list: length")
(assert (= (get (get rep-list-dec :bools) 0) false) "repeated bool list: [0]")
(assert (= (get (get rep-list-dec :bools) 1) true) "repeated bool list: [1]")

(assert (= (length (get rep-list-dec :doubles)) 2) "repeated double list: length")

## ── Repeated fields: nested messages with list ──────────────────────

(def p1 {:name "One" :age 1})
(def p2 {:name "Two" :age 2})
(def p3 {:name "Three" :age 3})

(def rep-people-list {:people (list p1 p2 p3)})
(def rep-people-buf (encode-fn pool "Repeats" rep-people-list))
(def rep-people-dec (decode-fn pool "Repeats" rep-people-buf))

(def people-dec (get rep-people-dec :people))
(assert (= (length people-dec) 3) "repeated message list: length")
(assert (= (get (get people-dec 0) :name) "One") "repeated message list: [0].name")
(assert (= (get (get people-dec 1) :name) "Two") "repeated message list: [1].name")
(assert (= (get (get people-dec 2) :name) "Three") "repeated message list: [2].name")
(assert (= (get (get people-dec 0) :age) 1) "repeated message list: [0].age")

## ── Repeated fields: nested messages with array ─────────────────────

(def rep-people-arr {:people [p1 p2]})
(def rep-pa-buf (encode-fn pool "Repeats" rep-people-arr))
(def rep-pa-dec (decode-fn pool "Repeats" rep-pa-buf))
(assert (= (length (get rep-pa-dec :people)) 2) "repeated message array: length")
(assert (= (get (get (get rep-pa-dec :people) 0) :name) "One") "repeated message array: [0].name")
(assert (= (get (get (get rep-pa-dec :people) 1) :name) "Two") "repeated message array: [1].name")

## ── Repeated fields: single element ─────────────────────────────────

(def rep-single {:ints [42]})
(def rep-single-dec (decode-fn pool "Repeats" (encode-fn pool "Repeats" rep-single)))
(assert (= (length (get rep-single-dec :ints)) 1) "repeated single element: length")
(assert (= (get (get rep-single-dec :ints) 0) 42) "repeated single element: value")

## ── Repeated fields: empty array ────────────────────────────────────

## In proto3, an empty repeated field is the default — it won't appear
## in the decoded message (has_field returns false). Encode should still
## succeed; decode just omits the field.
(def rep-empty {:ints []})
(def rep-empty-buf (encode-fn pool "Repeats" rep-empty))
(assert (bytes? rep-empty-buf) "empty repeated array encodes to bytes")

## ── Repeated fields: empty list ─────────────────────────────────────

(def rep-empty-list {:ints (list)})
(def rep-el-buf (encode-fn pool "Repeats" rep-empty-list))
(assert (bytes? rep-el-buf) "empty repeated list encodes to bytes")

## ── Repeated bytes field ────────────────────────────────────────────

(def rep-blobs {:blobs [(bytes 1 2 3) (bytes 4 5 6)]})
(def rep-blobs-dec (decode-fn pool "Repeats" (encode-fn pool "Repeats" rep-blobs)))
(assert (= (length (get rep-blobs-dec :blobs)) 2) "repeated bytes: length")
(assert (= (get (get rep-blobs-dec :blobs) 0) (bytes 1 2 3)) "repeated bytes: [0]")
(assert (= (get (get rep-blobs-dec :blobs) 1) (bytes 4 5 6)) "repeated bytes: [1]")

## ── Repeated bytes with list ────────────────────────────────────────

(def rep-blobs-list {:blobs (list (bytes 10 20) (bytes 30 40))})
(def rep-bl-dec (decode-fn pool "Repeats" (encode-fn pool "Repeats" rep-blobs-list)))
(assert (= (length (get rep-bl-dec :blobs)) 2) "repeated bytes list: length")
(assert (= (get (get rep-bl-dec :blobs) 0) (bytes 10 20)) "repeated bytes list: [0]")
(assert (= (get (get rep-bl-dec :blobs) 1) (bytes 30 40)) "repeated bytes list: [1]")

## ── Round-trip: Team with nested Persons ────────────────────────────

(def bob {:name "Bob" :age 25 :tags ["ops"]})
(def carol {:name "Carol" :age 28 :tags ["ml" "python"]})

(def team {:team_name "Alpha" :members [alice bob carol]})
(def team-buf (encode-fn pool "Team" team))
(assert (bytes? team-buf) "Team encode returns bytes")

(def team-decoded (decode-fn pool "Team" team-buf))
(assert (= (get team-decoded :team_name) "Alpha") "Team round-trip: team_name")

(def members (get team-decoded :members))
(assert (= (length members) 3) "Team round-trip: 3 members")
(assert (= (get (get members 0) :name) "Alice") "Team round-trip: member[0].name")
(assert (= (get (get members 1) :name) "Bob") "Team round-trip: member[1].name")
(assert (= (get (get members 2) :name) "Carol") "Team round-trip: member[2].name")
(assert (= (length (get (get members 2) :tags)) 2) "Team round-trip: member[2].tags length")

## ── Team with list of members ───────────────────────────────────────

(def team-list {:team_name "Beta" :members (list alice bob)})
(def team-list-dec (decode-fn pool "Team" (encode-fn pool "Team" team-list)))
(assert (= (get team-list-dec :team_name) "Beta") "Team list: team_name")
(def mlist (get team-list-dec :members))
(assert (= (length mlist) 2) "Team list: 2 members")
(assert (= (get (get mlist 0) :name) "Alice") "Team list: member[0].name")
(assert (= (get (get mlist 1) :name) "Bob") "Team list: member[1].name")

## ── Enum fields round-trip as keywords ──────────────────────────────

(def person-ok {:name "Dave" :status :OK})
(def person-ok-decoded (decode-fn pool "Person" (encode-fn pool "Person" person-ok)))
(assert (= (get person-ok-decoded :status) :OK) "enum field :OK round-trips as keyword")

(def person-error {:name "Eve" :status :ERROR})
(def person-error-decoded (decode-fn pool "Person" (encode-fn pool "Person" person-error)))
(assert (= (get person-error-decoded :status) :ERROR) "enum field :ERROR round-trips as keyword")

## Enum by integer
(def person-enum-int {:name "Fay" :status 2})
(def pei-dec (decode-fn pool "Person" (encode-fn pool "Person" person-enum-int)))
(assert (= (get pei-dec :status) :ERROR) "enum by int 2 decodes as :ERROR")

## ── Map fields ──────────────────────────────────────────────────────

(def person-scores {:name "Frank" :scores {:math 95 :science 88 :history 72}})
(def scores-buf (encode-fn pool "Person" person-scores))
(def scores-decoded (decode-fn pool "Person" scores-buf))

(def scores (get scores-decoded :scores))
(assert (struct? scores) "map field decodes as struct")
(assert (= (get scores :math) 95) "map field round-trip: math = 95")
(assert (= (get scores :science) 88) "map field round-trip: science = 88")
(assert (= (get scores :history) 72) "map field round-trip: history = 72")

## Map with string values
(def maps-ss {:ss {:foo "bar" :baz "qux"}})
(def maps-ss-dec (decode-fn pool "Maps" (encode-fn pool "Maps" maps-ss)))
(assert (= (get (get maps-ss-dec :ss) :foo) "bar") "map<string,string>: foo=bar")
(assert (= (get (get maps-ss-dec :ss) :baz) "qux") "map<string,string>: baz=qux")

## ── Nested message fields ───────────────────────────────────────────

(def nested-val {:person {:name "Nested" :age 99} :scalars {:i32 7 :s "hi"}})
(def nested-dec (decode-fn pool "Nested" (encode-fn pool "Nested" nested-val)))
(assert (= (get (get nested-dec :person) :name) "Nested") "nested message: person.name")
(assert (= (get (get nested-dec :person) :age) 99) "nested message: person.age")
(assert (= (get (get nested-dec :scalars) :i32) 7) "nested message: scalars.i32")
(assert (= (get (get nested-dec :scalars) :s) "hi") "nested message: scalars.s")

## ── Empty message round-trip ────────────────────────────────────────

(def empty-buf (encode-fn pool "Empty" {}))
(assert (bytes? empty-buf) "Empty message encode returns bytes")
(def empty-dec (decode-fn pool "Empty" empty-buf))
(assert (struct? empty-dec) "Empty message decode returns struct")

## ── Omitted fields (nil) ────────────────────────────────────────────

## Fields set to nil should be omitted (proto3 default behavior)
(def sparse {:name "Sparse"})
(def sparse-dec (decode-fn pool "Person" (encode-fn pool "Person" sparse)))
(assert (= (get sparse-dec :name) "Sparse") "sparse Person: name present")

## ── Deeply nested repeated ──────────────────────────────────────────

## Team members each have tags (repeated string) — test that inner
## repeated fields survive the outer repeated encode/decode.
(def deep-team {:team_name "Deep"
                :members [{:name "A" :age 1 :tags ["x" "y" "z"]}
                           {:name "B" :age 2 :tags (list "p" "q")}]})
(def deep-dec (decode-fn pool "Team" (encode-fn pool "Team" deep-team)))
(def dm (get deep-dec :members))
(assert (= (length (get (get dm 0) :tags)) 3) "deep nested: member[0] has 3 tags")
(assert (= (get (get (get dm 0) :tags) 2) "z") "deep nested: member[0].tags[2] = z")
(assert (= (length (get (get dm 1) :tags)) 2) "deep nested: member[1] has 2 tags (from list)")
(assert (= (get (get (get dm 1) :tags) 0) "p") "deep nested: member[1].tags[0] = p")

## ── Error: unknown message name ─────────────────────────────────────

(let [([ok? err] (protect ((fn () (encode-fn pool "NoSuchMessage" {:x 1})))))]
  (assert (not ok?) "encode unknown message errors")
  (assert (= (get err :error) :protobuf-error) "encode unknown message: protobuf-error"))

(let [([ok? err] (protect ((fn () (decode-fn pool "NoSuchMessage" (bytes 0))))))]
  (assert (not ok?) "decode unknown message errors")
  (assert (= (get err :error) :protobuf-error) "decode unknown message: protobuf-error"))

(let [([ok? err] (protect ((fn () (fields-fn pool "NoSuchMessage")))))]
  (assert (not ok?) "fields unknown message errors")
  (assert (= (get err :error) :protobuf-error) "fields unknown message: protobuf-error"))

## ── Error: wrong types ──────────────────────────────────────────────

(let [([ok? _] (protect ((fn () (schema-fn 42)))))]
  (assert (not ok?) "protobuf/schema non-string errors"))

(let [([ok? err] (protect ((fn () (encode-fn pool "Person" "not a struct")))))]
  (assert (not ok?) "encode non-struct errors")
  (assert (= (get err :error) :type-error) "encode non-struct: type-error"))

(let [([ok? err] (protect ((fn () (decode-fn pool "Person" "not bytes")))))]
  (assert (not ok?) "decode non-bytes errors")
  (assert (= (get err :error) :type-error) "decode non-bytes: type-error"))

(let [([ok? err] (protect ((fn () (messages-fn 42)))))]
  (assert (not ok?) "messages non-pool errors")
  (assert (= (get err :error) :type-error) "messages non-pool: type-error"))

(let [([ok? err] (protect ((fn () (fields-fn 42 "Person")))))]
  (assert (not ok?) "fields non-pool errors")
  (assert (= (get err :error) :type-error) "fields non-pool: type-error"))

(let [([ok? err] (protect ((fn () (enums-fn 42)))))]
  (assert (not ok?) "enums non-pool errors")
  (assert (= (get err :error) :type-error) "enums non-pool: type-error"))

## ── Error: bad schema ───────────────────────────────────────────────

(let [([ok? err] (protect ((fn () (schema-fn "this is not valid proto")))))]
  (assert (not ok?) "bad proto schema errors")
  (assert (= (get err :error) :protobuf-error) "bad proto schema: protobuf-error"))

## ── Error: schema-bytes with non-bytes ──────────────────────────────

(let [([ok? err] (protect ((fn () (schema-bytes-fn "not bytes")))))]
  (assert (not ok?) "schema-bytes non-bytes errors")
  (assert (= (get err :error) :type-error) "schema-bytes non-bytes: type-error"))

## ── Error: encode wrong field type ──────────────────────────────────

(let [([ok? err] (protect ((fn () (encode-fn pool "Person" {:name 42})))))]
  (assert (not ok?) "encode wrong field type errors")
  (assert (= (get err :error) :protobuf-error) "encode wrong field type: protobuf-error"))

## ── Error: repeated field given non-array/list ──────────────────────

(let [([ok? err] (protect ((fn () (encode-fn pool "Person" {:name "X" :tags 42})))))]
  (assert (not ok?) "encode repeated with int errors")
  (assert (= (get err :error) :protobuf-error) "encode repeated with int: protobuf-error"))

## ── Error: unknown enum keyword ─────────────────────────────────────

(let [([ok? err] (protect ((fn () (encode-fn pool "Person" {:name "X" :status :BOGUS})))))]
  (assert (not ok?) "encode unknown enum keyword errors")
  (assert (= (get err :error) :protobuf-error) "encode unknown enum: protobuf-error"))
