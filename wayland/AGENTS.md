# Wayland Plugin

Wayland compositor interaction via the stable elle-plugin ABI.

## Architecture

Event-buffer pattern: Dispatch impls push to `Vec<WlEvent>`, Elle
drains via `wl/poll-events`. No calloop — Elle's `ev/poll-fd` drives
the event loop.

`wl/dispatch` does `prepare_read()` + non-blocking `read()` +
`dispatch_pending()`. Caller must `ev/poll-fd` on the display fd first
to ensure data is available.

## Event loop pattern

```lisp
(while running
  (wl:flush conn)                    # send pending requests
  (ev/poll-fd fd :read 0.033)        # wait for compositor data (yields)
  (wl:dispatch conn)                 # read from wire + dispatch
  (each ev in (wl:poll-events conn)  # drain buffered events
    ...))
```

## Files

| File | Purpose |
|------|---------|
| `lib.rs` | Primitive table, entry point, event → value conversion, keyword→enum helpers |
| `state.rs` | WaylandState, WlEvent enum, Dispatch impls (registry, output, seat, shm, layer_shell) |
| `buffer.rs` | SHM pool/buffer management (memfd, mmap, write, fill, fill_rect) |
| `layer.rs` | Layer-shell surface lifecycle (Dispatch for ZwlrLayerSurfaceV1) |
| `capture.rs` | Screencopy frame capture |
| `toplevel.rs` | Foreign-toplevel window tracking |

## Primitives (25)

**Connection (6):** `wl/connect`, `wl/disconnect`, `wl/display-fd`,
`wl/dispatch`, `wl/flush`, `wl/poll-events`

**Queries (2):** `wl/outputs`, `wl/seats`

**Layer shell (3):** `wl/layer-surface`, `wl/layer-configure`,
`wl/layer-destroy`

**Surface ops (3):** `wl/attach`, `wl/damage`, `wl/commit`

**SHM buffers (5):** `wl/shm-buffer`, `wl/buffer-write`,
`wl/buffer-fill`, `wl/buffer-fill-rect`, `wl/buffer-destroy`

**Screencopy (2):** `wl/screencopy`, `wl/screencopy-destroy`

**Foreign toplevel (4):** `wl/toplevels`, `wl/toplevel-activate`,
`wl/toplevel-close`, `wl/toplevel-subscribe`

## Layer surface options

`wl/layer-surface` accepts an optional struct with:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `:layer` | keyword | `:overlay` | `:background`, `:bottom`, `:top`, `:overlay` |
| `:anchor` | array of keywords | `[:top :left :right]` | Edge anchors |
| `:width` | int | 0 | Width (0 = compositor decides from anchors) |
| `:height` | int | 50 | Height in pixels |
| `:exclusive-zone` | int | 0 | Reserved space (0 = none, -1 = auto) |
| `:namespace` | string | "elle" | Layer namespace |

## Events

Events from `wl/poll-events` are structs with `:type` keyword:

```
{:type :output     :id 1  :name "DP-1"  :width 2560  :height 1440  :scale 1}
{:type :seat       :id 1  :name "seat0"  :caps 3}
{:type :configure  :surface-id 1  :serial 42  :width 1920  :height 32}
{:type :closed     :surface-id 1}
{:type :buffer-release  :buffer-id 1}
{:type :screencopy-ready   :frame-id 1}
{:type :screencopy-failed  :frame-id 1}
{:type :toplevel-new    :id 1  :title "Firefox"  :app-id "firefox"}
{:type :toplevel-done   :id 1  :title "..."  :state #{:activated}}
{:type :toplevel-closed :id 1}
```

## Dependencies

- `elle-plugin` — stable ABI types and macros (NOT elle)
- `wayland-client` 0.31
- `wayland-protocols` 0.32
- `wayland-protocols-wlr` 0.3
- `libc` 0.2

## Invariants

1. Connection handle is an opaque external object — never inspect it.
2. Events are buffered until `wl/poll-events` drains them.
3. All fd-based I/O goes through `ev/poll-fd` — no blocking calls.
4. `wl/dispatch` does `prepare_read` + non-blocking `read` before
   `dispatch_pending`. Caller must `ev/poll-fd` first.
5. Plugin can be compiled independently from elle.
