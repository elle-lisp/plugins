# Elle Plugins

Dynamically-loaded Rust libraries that extend [Elle](https://github.com/elle-lisp/elle)
with additional primitives.

Plugins use the **stable `elle-plugin` ABI** — they can be compiled
independently from elle and loaded at runtime without version matching.

## Building

```bash
# All plugins
cargo build --release

# Single plugin
cargo build --release -p elle-crypto
```

## Plugins

| Plugin | Description |
|--------|-------------|
| `elle-arrow` | Apache Arrow columnar data |
| `elle-crypto` | SHA-2 hashing and HMAC |
| `elle-csv` | CSV reading and writing |
| `elle-egui` | Immediate-mode GUI |
| `elle-hash` | Universal hashing (SHA-3, BLAKE3, CRC32, etc.) |
| `elle-image` | Image processing |
| `elle-jiff` | Date/time operations |
| `elle-mqtt` | MQTT client |
| `elle-msgpack` | MessagePack serialization |
| `elle-oxigraph` | RDF triple store |
| `elle-polars` | DataFrames (Polars) |
| `elle-protobuf` | Protocol Buffers |
| `elle-random` | Pseudo-random numbers |
| `elle-regex` | Regular expressions |
| `elle-selkie` | Mermaid diagram rendering |
| `elle-svg` | SVG generation |
| `elle-syn` | Rust source parsing |
| `elle-tls` | TLS client/server (rustls) |
| `elle-toml` | TOML parsing |
| `elle-tree-sitter` | Multi-language parsing |
| `elle-vulkan` | Vulkan GPU compute |
| `elle-wayland` | Wayland compositor interaction |
| `elle-xml` | XML parsing |
| `elle-yaml` | YAML parsing |

## Writing a new plugin

See the [elle docs](https://github.com/elle-lisp/elle/blob/main/docs/cookbook/plugins.md)
for a step-by-step guide.
