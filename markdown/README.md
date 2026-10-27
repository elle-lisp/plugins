# elle-markdown

<!-- audited: 2026-10-27 -->

A markdown plugin for Elle: CommonMark rendered to HTML via the Rust `pulldown-cmark` crate.

## Building

Built as part of the workspace:

```sh
cargo build --workspace
```

Produces `target/debug/libelle_markdown.so` (or `target/release/libelle_markdown.so`).

## Usage

```lisp
(def markdown (import "plugin/markdown"))

(markdown:html "# Title\n\nSome *text*.\n")
;; => "<h1>Title</h1>\n<p>Some <em>text</em>.</p>\n"

(markdown:html "a ~~b~~\n" {:strikethrough true})
;; => "<p>a <del>b</del></p>\n"
```

## Primitives

| Name | Arguments | Output | Description |
|------|-----------|--------|-------------|
| `markdown/html` | text, options | string | the HTML that text renders to |

The text is a string. The options are optional: a struct whose keys
are keywords and whose values are booleans. Each key turns one
extension of `pulldown-cmark` on or off, and a key left out is off, so
with no options the text renders as plain CommonMark.

| Key | Extension |
|-----|-----------|
| `:tables` | tables, with a delimiter row under the header row |
| `:strikethrough` | `~~text~~` as `<del>` |
| `:tasklists` | `- [ ]` and `- [x]` list items as checkboxes |
| `:footnotes` | `[^name]` references and their definitions |

A text that is not a string, options that are not a struct, or an
option whose value is not a boolean is a `:type-error`. A key that
names no extension is a `:value-error`, so a misspelled key never
renders the text without the extension it asked for.

The same text and options always render to the same string.
