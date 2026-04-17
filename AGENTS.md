# plugins

Dynamically-loaded Rust libraries that extend Elle with additional primitives.

## Responsibility

Provide optional functionality via `.so` cdylib crates that:
- Depend on `elle-plugin` (not `elle`) for ABI stability
- Register primitives via the stable plugin ABI
- Can be compiled and loaded independently from elle

## Stable plugin ABI

Plugins depend on the `elle-plugin` crate, which provides:
- `ElleValue` — opaque 16-byte type (same layout as elle's `Value`)
- `ElleApiLoader` — named function lookup (like `vkGetInstanceProcAddr`)
- `Api` — resolved function pointer cache for constructors/accessors
- `EllePrimDef` — primitive metadata (C-compatible)
- `define_plugin!` — generates `elle_plugin_init` entry point

The ABI contract is a single resolve function. Plugins look up what they
need by name at init time. Adding API functions to elle never breaks
existing plugins.

### Plugin init protocol

1. Elle loads the `.so` via `libloading` with `RTLD_GLOBAL`
2. Calls `elle_plugin_init(loader, ctx)` — the plugin's exported symbol
3. Plugin calls `Api::load(loader)` to resolve all API function pointers
4. Plugin registers its primitives via `ctx.register()`
5. Elle converts collected `EllePrimDef`s to internal `PrimitiveDef`s
6. The plugin's primitives are dispatched through an address-keyed table

### Writing a plugin

```rust
use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK};

elle_plugin::define_plugin!("myplugin/", &PRIMITIVES);

extern "C" fn prim_hello(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    a.ok(a.string("hello"))
}

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("myplugin/hello", prim_hello, SIG_OK, 0,
        "Say hello.", "myplugin", "(myplugin/hello)"),
];
```

```toml
# Cargo.toml
[dependencies]
elle-plugin = { path = "../../elle-plugin" }  # NOT elle
```

## Available plugins

| Plugin | Purpose |
|--------|---------|
| `arrow/` | Apache Arrow columnar data and Parquet |
| `crypto/` | SHA-2 hashing and HMAC |
| `csv/` | CSV parsing and serialization |
| `egui/` | Immediate-mode GUI via egui |
| `hash/` | Universal hashing (MD5, SHA-1/2/3, BLAKE2/3, CRC32, xxHash) |
| `jiff/` | Date/time operations |
| `mqtt/` | MQTT packet codec |
| `msgpack/` | MessagePack serialization |
| `oxigraph/` | RDF quad store with SPARQL |
| `polars/` | Polars DataFrame operations |
| `protobuf/` | Protocol Buffers |
| `random/` | Random number generation |
| `regex/` | Regular expression matching |
| `selkie/` | Mermaid diagram rendering |
| `syn/` | Rust syntax parsing |
| `tls/` | TLS client and server via rustls |
| `toml/` | TOML parsing and serialization |
| `tree-sitter/` | Multi-language parsing |
| `vulkan/` | Vulkan GPU compute |
| `wayland/` | Wayland compositor interaction |
| `xml/` | XML parsing and serialization |
| `yaml/` | YAML parsing and serialization |

## Invariants

1. **Plugins are never unloaded.** The library handle is leaked.
2. **Plugins depend on `elle-plugin`, not `elle`.** This enables independent compilation.
3. **ABI is stable.** Plugins compiled against one elle version work with future versions.
4. **Keywords interned through the API** route to the host's global table.

## Dependents

- `src/plugin.rs` — plugin loading infrastructure
- `src/plugin_api.rs` — stable ABI function implementations
- `src/main.rs` — loads plugins via `import` primitive
- Elle code — via `(import "plugin/name")`

## Files

| Directory | Purpose |
|-----------|---------|
| `arrow/` | Apache Arrow columnar data |
| `crypto/` | SHA-2 hashing and HMAC |
| `csv/` | CSV parsing |
| `egui/` | Immediate-mode GUI |
| `hash/` | Universal hashing |
| `image/` | Image processing |
| `jiff/` | Date/time |
| `mqtt/` | MQTT codec |
| `msgpack/` | MessagePack |
| `oxigraph/` | RDF store |
| `polars/` | DataFrames |
| `protobuf/` | Protocol Buffers |
| `random/` | RNG |
| `regex/` | Regular expressions |
| `selkie/` | Mermaid rendering |
| `svg/` | SVG generation |
| `syn/` | Rust parsing |
| `tls/` | TLS |
| `toml/` | TOML |
| `tree-sitter/` | Multi-language parsing |
| `vulkan/` | GPU compute |
| `wayland/` | Wayland compositor |
| `xml/` | XML |
| `yaml/` | YAML |
