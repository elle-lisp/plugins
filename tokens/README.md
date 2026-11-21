# elle-tokens

<!-- audited: 2026-11-21 -->

A tokens plugin for Elle: a text counted, encoded and decoded in the BPE encodings of the Rust `tiktoken-rs` crate.

## Building

Built as part of the workspace:

```sh
cargo build --workspace
```

Produces `target/debug/libelle_tokens.so` (or `target/release/libelle_tokens.so`).

## Usage

```lisp
(def tokens (import "plugin/tokens"))

(tokens:count "hello world")
;; => 2

(tokens:encode "hello world")
;; => [24912 2375]

(tokens:encode "hello world" :cl100k_base)
;; => [15339 1917]

(tokens:decode [24912 2375])
;; => "hello world"
```

## Primitives

| Name | Arguments | Output | Description |
|------|-----------|--------|-------------|
| `tokens/count` | text, encoding | integer | how many tokens the text encodes to |
| `tokens/encode` | text, encoding | array of integers | the id of each token the text encodes to, in order |
| `tokens/decode` | ids, encoding | string | the text that the ids decode to |
| `tokens/encodings` | none | set of keywords | every encoding the plugin knows |

The encoding is optional, and `:o200k_base` is the default. The text is a
string. The ids are an immutable array or a list of integers.
`(tokens/count text)` always equals the length of `(tokens/encode text)`.

## Encodings

| Keyword | Its vocabulary |
|---------|----------------|
| `:o200k_base` | the encoding of GPT-4o, GPT-4.1, GPT-5 and the o series |
| `:o200k_harmony` | `:o200k_base`, with the special tokens of gpt-oss |
| `:cl100k_base` | the encoding of GPT-4, GPT-3.5 and the text-embedding-3 models |
| `:p50k_base` | the encoding of Codex and text-davinci-002 and -003 |
| `:p50k_edit` | `:p50k_base`, with the fill-in-the-middle tokens of the edit models |
| `:r50k_base` | the encoding of GPT-3 and GPT-2 |

Each keyword is spelled as `tiktoken` spells the encoding. The plugin builds
an encoding the first time a call names it, which takes a fraction of a
second, and every later call reuses it.

## Special tokens

`tokens/count` and `tokens/encode` read every character of the text as
ordinary text. The spelling of a special token, such as `<|endoftext|>`,
encodes as the characters it is, and never as the one id the encoding
reserves for it. A text that quotes a special token therefore counts what
a reader of that text pays.

`tokens/decode` knows the ids of the special tokens, so
`(tokens/decode [199999])` answers `"<|endoftext|>"`.

## Errors

| Call | Error |
|------|-------|
| a text that is not a string | `:type-error` |
| ids that are not an immutable array or a list, or an id that is not an integer | `:type-error` |
| an encoding that is not a keyword | `:type-error` |
| a keyword that names no encoding | `:value-error` |
| an id that names no token of the encoding | `:value-error` |
| ids whose bytes are not UTF-8 | `:value-error` |

The bytes of one character can span more than one token. A slice of the
ids can therefore cut a character in two, and its decode is a
`:value-error`. To cut a text to at most n tokens, decode its first n ids,
and drop one more id while the decode fails.
