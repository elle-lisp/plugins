(elle/epoch 15)

## Markdown plugin integration tests
##
## Unlike the other plugin tests, this one does not skip when the library
## is missing: CI builds every plugin before it runs a test, so a skip
## would only hide a plugin that failed to build.

(def plugin (import-file "target/release/libelle_markdown.so"))
(def html (get plugin :html))

(defn holds? [s part]
  (string/contains? s part))

## ── plain CommonMark ───────────────────────────────────────────

(assert (= (html "# Title\n\nSome *text*.\n")
           "<h1>Title</h1>\n<p>Some <em>text</em>.</p>\n")
        "a heading and a paragraph render as the CommonMark spec writes them")

(assert (= (html "é and 漢字\n") "<p>é and 漢字</p>\n")
        "text outside ASCII passes through unchanged")

(assert (= (html "") "") "an empty text renders to an empty string")

(assert (= (html "a\nb\n" {}) (html "a\nb\n"))
        "empty options are the same as none")

## ── each extension is off until its key turns it on ────────────

(def table "| a |\n|---|\n| 1 |\n")
(assert (not (holds? (html table) "<table>")) "a table is plain text by default")
(def tabled (html table {:tables true}))
(assert (holds? tabled "<table>") ":tables renders a table")
(assert (holds? tabled "<th>a</th>") ":tables renders the header row as th")
(assert (holds? tabled "<td>1</td>") ":tables renders a body row as td")
(assert (not (holds? (html table {:tables false}) "<table>"))
        "a key set to false leaves its extension off")

(assert (= (html "a ~~b~~\n") "<p>a ~~b~~</p>\n")
        "strikethrough is plain text by default")
(assert (= (html "a ~~b~~\n" {:strikethrough true}) "<p>a <del>b</del></p>\n")
        ":strikethrough renders del")

(def tasks "- [ ] open\n- [x] done\n")
(assert (not (holds? (html tasks) "checkbox")) "a task item is plain text by default")
(def ticked (html tasks {:tasklists true}))
(assert (holds? ticked "type=\"checkbox\"") ":tasklists renders a checkbox")
(assert (holds? ticked "checked") ":tasklists marks a done item checked")

(def noted "a[^n]\n\n[^n]: the note\n")
(assert (not (holds? (html noted) "href=\"#n\"")) "a footnote is no footnote by default")
(def footed (html noted {:footnotes true}))
(assert (holds? footed "href=\"#n\"") ":footnotes links the reference to its definition")
(assert (holds? footed "id=\"n\"") ":footnotes gives the definition its id")
(assert (holds? footed "the note") ":footnotes renders the definition's text")

(def every {:tables true :strikethrough true :tasklists true :footnotes true})
(assert (= (html (string table "\n" tasks) every) (html (string table "\n" tasks) every))
        "the same text and options render to the same string")

## ── errors ──────────────────────────────────────────────────────

(defn raised [thunk]
  (def [ok? err] (protect (thunk)))
  (when (not ok?) (get err :error)))

(assert (= (raised (fn [] (html 42))) :type-error) "a text that is not a string is a type-error")
(assert (= (raised (fn [] (html "a" [:tables]))) :type-error)
        "options that are not a struct are a type-error")
(assert (= (raised (fn [] (html "a" {:tables 1}))) :type-error)
        "an option that is not a boolean is a type-error")
(assert (= (raised (fn [] (html "a" {:table true}))) :value-error)
        "a key that names no extension is a value-error")
