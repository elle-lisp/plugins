#![allow(dead_code)]
//! Elle Wayland plugin — compositor interaction via the stable plugin ABI.
//!
//! Provides primitives for Wayland connection, surface management, SHM
//! buffers, layer-shell surfaces, screencopy, and foreign-toplevel.

mod buffer;
mod capture;
mod layer;
mod state;
mod toplevel;

use std::os::unix::io::{AsFd, AsRawFd};

use wayland_client::{Connection, EventQueue};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use elle_plugin::{EllePrimDef, ElleResult, ElleValue, SIG_OK};

use buffer::ShmBuffer;
use layer::LayerSurface;
use state::{WaylandState, WlEvent};

elle_plugin::define_plugin!("wl/", &PRIMITIVES);

// ── Connection state ──────────────────────────────────────────────────

struct WlConn {
    conn: Connection,
    queue: EventQueue<WaylandState>,
    state: WaylandState,
    surfaces: Vec<LayerSurface>,
    buffers: Vec<ShmBuffer>,
    next_surface_id: u32,
    next_buffer_id: u32,
}

impl WlConn {
    fn find_surface(&self, id: u32) -> Option<usize> {
        self.surfaces.iter().position(|s| s.id == id)
    }

    fn find_buffer(&self, id: u32) -> Option<usize> {
        self.buffers.iter().position(|b| b.id == id)
    }
}

// ── Keyword → enum helpers ────────────────────────────────────────────

fn parse_layer(a: &elle_plugin::Api, v: ElleValue) -> zwlr_layer_shell_v1::Layer {
    if let Some(name) = a.get_keyword_name(v) {
        match name {
            "background" => zwlr_layer_shell_v1::Layer::Background,
            "bottom" => zwlr_layer_shell_v1::Layer::Bottom,
            "top" => zwlr_layer_shell_v1::Layer::Top,
            "overlay" => zwlr_layer_shell_v1::Layer::Overlay,
            _ => zwlr_layer_shell_v1::Layer::Overlay,
        }
    } else {
        zwlr_layer_shell_v1::Layer::Overlay
    }
}

fn parse_anchor(a: &elle_plugin::Api, v: ElleValue) -> zwlr_layer_surface_v1::Anchor {
    let mut anchor = zwlr_layer_surface_v1::Anchor::empty();
    // Accept an array of keyword anchors
    if let Some(n) = a.get_array_len(v) {
        for i in 0..n {
            let elem = a.get_array_item(v, i);
            if let Some(name) = a.get_keyword_name(elem) {
                match name {
                    "top" => anchor.insert(zwlr_layer_surface_v1::Anchor::Top),
                    "bottom" => anchor.insert(zwlr_layer_surface_v1::Anchor::Bottom),
                    "left" => anchor.insert(zwlr_layer_surface_v1::Anchor::Left),
                    "right" => anchor.insert(zwlr_layer_surface_v1::Anchor::Right),
                    _ => {}
                }
            }
        }
    }
    anchor
}

// ── Event → ElleValue conversion ──────────────────────────────────────

fn event_to_value(ev: &WlEvent) -> ElleValue {
    let a = api();
    match ev {
        WlEvent::Output {
            id,
            name,
            width,
            height,
            scale,
        } => a.build_struct(&[
            ("type", a.keyword("output")),
            ("id", a.int(*id as i64)),
            ("name", a.string(name)),
            ("width", a.int(*width as i64)),
            ("height", a.int(*height as i64)),
            ("scale", a.int(*scale as i64)),
        ]),
        WlEvent::Seat { id, name, caps } => a.build_struct(&[
            ("type", a.keyword("seat")),
            ("id", a.int(*id as i64)),
            ("name", a.string(name)),
            ("caps", a.int(*caps as i64)),
        ]),
        WlEvent::Configure {
            surface_id,
            serial,
            width,
            height,
        } => a.build_struct(&[
            ("type", a.keyword("configure")),
            ("surface-id", a.int(*surface_id as i64)),
            ("serial", a.int(*serial as i64)),
            ("width", a.int(*width as i64)),
            ("height", a.int(*height as i64)),
        ]),
        WlEvent::Closed { surface_id } => a.build_struct(&[
            ("type", a.keyword("closed")),
            ("surface-id", a.int(*surface_id as i64)),
        ]),
        WlEvent::BufferRelease { buffer_id } => a.build_struct(&[
            ("type", a.keyword("buffer-release")),
            ("buffer-id", a.int(*buffer_id as i64)),
        ]),
        WlEvent::ScreencopyReady { frame_id } => a.build_struct(&[
            ("type", a.keyword("screencopy-ready")),
            ("frame-id", a.int(*frame_id as i64)),
        ]),
        WlEvent::ScreencopyFailed { frame_id } => a.build_struct(&[
            ("type", a.keyword("screencopy-failed")),
            ("frame-id", a.int(*frame_id as i64)),
        ]),
        WlEvent::ToplevelNew { id, title, app_id } => a.build_struct(&[
            ("type", a.keyword("toplevel-new")),
            ("id", a.int(*id as i64)),
            ("title", a.string(title)),
            ("app-id", a.string(app_id)),
        ]),
        WlEvent::ToplevelDone { id, title, state } => {
            let state_vals: Vec<ElleValue> = state.iter().map(|s| a.keyword(s)).collect();
            a.build_struct(&[
                ("type", a.keyword("toplevel-done")),
                ("id", a.int(*id as i64)),
                ("title", a.string(title)),
                ("state", a.set(&state_vals)),
            ])
        }
        WlEvent::ToplevelClosed { id } => a.build_struct(&[
            ("type", a.keyword("toplevel-closed")),
            ("id", a.int(*id as i64)),
        ]),
    }
}

// ── Connection primitives ─────────────────────────────────────────────

extern "C" fn prim_connect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let _ = unsafe { a.args(args, nargs) };

    let conn = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => return a.err("wayland-error", &format!("connect failed: {}", e)),
    };

    let display = conn.display();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = display.get_registry(&qh, ());

    let mut state = WaylandState::new();
    // Do initial roundtrip to bind globals
    if let Err(e) = queue.roundtrip(&mut state) {
        return a.err("wayland-error", &format!("roundtrip failed: {}", e));
    }

    let wl = WlConn {
        conn,
        queue,
        state,
        surfaces: Vec::new(),
        buffers: Vec::new(),
        next_surface_id: 1,
        next_buffer_id: 1,
    };

    a.ok(a.external("wayland-connection", wl))
}

extern "C" fn prim_disconnect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let args = unsafe { a.args(args, nargs) };
    if nargs != 1 {
        return a.err("arity-error", "wl/disconnect: expected 1 argument");
    }
    // Just drop the connection — the external's drop fn handles cleanup
    let _ = args[0];
    a.ok(a.nil())
}

extern "C" fn prim_display_fd(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err("arity-error", "wl/display-fd: expected 1 argument");
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    let wl = match a.get_external::<WlConn>(val, "wayland-connection") {
        Some(w) => w,
        None => return a.err("type-error", "wl/display-fd: expected wayland connection"),
    };
    let fd = wl.conn.as_fd().as_raw_fd();
    a.ok(a.int(fd as i64))
}

extern "C" fn prim_dispatch(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err("arity-error", "wl/dispatch: expected 1 argument");
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    let wl = match unsafe { a.get_external_mut::<WlConn>(val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/dispatch: expected wayland connection"),
    };
    // Non-blocking read from the wire. Caller should ev/poll-fd first.
    if let Some(guard) = wl.queue.prepare_read() {
        let _ = guard.read(); // May be WouldBlock — that's fine
    }
    match wl.queue.dispatch_pending(&mut wl.state) {
        Ok(n) => a.ok(a.int(n as i64)),
        Err(e) => a.err("wayland-error", &format!("dispatch failed: {}", e)),
    }
}

extern "C" fn prim_flush(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err("arity-error", "wl/flush: expected 1 argument");
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    let wl = match a.get_external::<WlConn>(val, "wayland-connection") {
        Some(w) => w,
        None => return a.err("type-error", "wl/flush: expected wayland connection"),
    };
    if let Err(e) = wl.conn.flush() {
        return a.err("wayland-error", &format!("flush failed: {}", e));
    }
    a.ok(a.nil())
}

extern "C" fn prim_poll_events(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err("arity-error", "wl/poll-events: expected 1 argument");
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    let wl = match unsafe { a.get_external_mut::<WlConn>(val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/poll-events: expected wayland connection"),
    };
    let events: Vec<ElleValue> = wl
        .state
        .events
        .drain(..)
        .map(|e| event_to_value(&e))
        .collect();
    a.ok(a.array(&events))
}

// ── Query primitives ──────────────────────────────────────────────────

extern "C" fn prim_outputs(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err("arity-error", "wl/outputs: expected 1 argument");
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    let wl = match a.get_external::<WlConn>(val, "wayland-connection") {
        Some(w) => w,
        None => return a.err("type-error", "wl/outputs: expected wayland connection"),
    };
    let items: Vec<ElleValue> = wl
        .state
        .outputs
        .iter()
        .map(|o| {
            a.build_struct(&[
                ("id", a.int(o.id as i64)),
                ("name", a.string(&o.name)),
                ("width", a.int(o.width as i64)),
                ("height", a.int(o.height as i64)),
                ("scale", a.int(o.scale as i64)),
            ])
        })
        .collect();
    a.ok(a.array(&items))
}

extern "C" fn prim_seats(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err("arity-error", "wl/seats: expected 1 argument");
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    let wl = match a.get_external::<WlConn>(val, "wayland-connection") {
        Some(w) => w,
        None => return a.err("type-error", "wl/seats: expected wayland connection"),
    };
    let items: Vec<ElleValue> = wl
        .state
        .seats
        .iter()
        .map(|s| {
            a.build_struct(&[
                ("id", a.int(s.id as i64)),
                ("name", a.string(&s.name)),
                ("caps", a.int(s.caps as i64)),
            ])
        })
        .collect();
    a.ok(a.array(&items))
}

// ── Layer shell primitives ────────────────────────────────────────────

extern "C" fn prim_layer_surface(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs < 1 {
        return a.err(
            "arity-error",
            "wl/layer-surface: expected at least 1 argument",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => {
            return a.err(
                "type-error",
                "wl/layer-surface: expected wayland connection",
            )
        }
    };

    let compositor = match &wl.state.compositor {
        Some(c) => c.clone(),
        None => return a.err("wayland-error", "compositor not available"),
    };
    let layer_shell =
        match &wl.state.layer_shell {
            Some(ls) => ls.clone(),
            None => return a.err(
                "wayland-error",
                "zwlr_layer_shell_v1 not available — compositor does not support wlr-layer-shell",
            ),
        };

    // Parse options struct (second argument, optional)
    let mut layer = zwlr_layer_shell_v1::Layer::Overlay;
    let mut anchor = zwlr_layer_surface_v1::Anchor::Top
        | zwlr_layer_surface_v1::Anchor::Left
        | zwlr_layer_surface_v1::Anchor::Right;
    let mut width = 0;
    let mut height = 50;
    let mut exclusive_zone: i32 = 0;

    if nargs >= 2 {
        let opts = unsafe { a.arg(args, nargs, 1) };
        if a.check_struct(opts) {
            let lv = a.get_struct_field(opts, "layer");
            if !a.check_nil(lv) {
                layer = parse_layer(a, lv);
            }

            let av = a.get_struct_field(opts, "anchor");
            if !a.check_nil(av) {
                anchor = parse_anchor(a, av);
            }

            let wv = a.get_struct_field(opts, "width");
            if let Some(w) = a.get_int(wv) {
                width = w as i32;
            }

            let hv = a.get_struct_field(opts, "height");
            if let Some(h) = a.get_int(hv) {
                height = h as i32;
            }

            let ev = a.get_struct_field(opts, "exclusive-zone");
            if let Some(e) = a.get_int(ev) {
                exclusive_zone = e as i32;
            }
        }
    }

    let sid = wl.next_surface_id;
    wl.next_surface_id += 1;

    let qh = wl.queue.handle();
    let surface = compositor.create_surface(&qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None, // output: None = all outputs
        layer,
        String::from("elle-progress"), // namespace
        &qh,
        sid, // user data = surface id for dispatch
    );

    layer_surface.set_anchor(anchor);
    layer_surface.set_size(width as u32, height as u32);
    layer_surface.set_exclusive_zone(exclusive_zone);

    // Commit to trigger the initial configure
    surface.commit();

    wl.surfaces.push(LayerSurface {
        id: sid,
        surface,
        layer_surface,
        configured: false,
        width,
        height,
    });

    if let Err(e) = wl.conn.flush() {
        return a.err(
            "wayland-error",
            &format!("flush after layer-surface creation failed: {}", e),
        );
    }

    a.ok(a.int(sid as i64))
}

extern "C" fn prim_layer_configure(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    // Configure is handled automatically via events — ack is done in the
    // dispatch impl. This primitive exists for API completeness.
    a.ok(a.nil())
}

extern "C" fn prim_layer_destroy(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(
            "arity-error",
            "wl/layer-destroy: expected 2 arguments (conn, surface-id)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let surface_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/layer-destroy: surface-id must be integer"),
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => {
            return a.err(
                "type-error",
                "wl/layer-destroy: expected wayland connection",
            )
        }
    };
    if let Some(idx) = wl.find_surface(surface_id) {
        let ls = wl.surfaces.swap_remove(idx);
        ls.layer_surface.destroy();
        ls.surface.destroy();
    }
    if let Err(e) = wl.conn.flush() {
        return a.err("wayland-error", &format!("flush failed: {}", e));
    }
    a.ok(a.nil())
}

// ── Surface ops ───────────────────────────────────────────────────────

extern "C" fn prim_attach(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 3 {
        return a.err(
            "arity-error",
            "wl/attach: expected 3 arguments (conn, surface-id, buffer-id)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let surface_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/attach: surface-id must be integer"),
    };
    let buffer_id = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/attach: buffer-id must be integer"),
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/attach: expected wayland connection"),
    };
    let sidx = match wl.find_surface(surface_id) {
        Some(i) => i,
        None => {
            return a.err(
                "wayland-error",
                &format!("surface {} not found", surface_id),
            )
        }
    };
    let bidx = match wl.find_buffer(buffer_id) {
        Some(i) => i,
        None => return a.err("wayland-error", &format!("buffer {} not found", buffer_id)),
    };
    wl.surfaces[sidx]
        .surface
        .attach(Some(&wl.buffers[bidx].buffer), 0, 0);
    a.ok(a.nil())
}

extern "C" fn prim_damage(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 6 {
        return a.err(
            "arity-error",
            "wl/damage: expected 6 arguments (conn, surface-id, x, y, w, h)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let surface_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/damage: surface-id must be integer"),
    };
    let x = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/damage: x must be integer"),
    };
    let y = match a.get_int(unsafe { a.arg(args, nargs, 3) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/damage: y must be integer"),
    };
    let w = match a.get_int(unsafe { a.arg(args, nargs, 4) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/damage: width must be integer"),
    };
    let h = match a.get_int(unsafe { a.arg(args, nargs, 5) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/damage: height must be integer"),
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/damage: expected wayland connection"),
    };
    let sidx = match wl.find_surface(surface_id) {
        Some(i) => i,
        None => {
            return a.err(
                "wayland-error",
                &format!("surface {} not found", surface_id),
            )
        }
    };
    wl.surfaces[sidx].surface.damage(x, y, w, h);
    a.ok(a.nil())
}

extern "C" fn prim_commit(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(
            "arity-error",
            "wl/commit: expected 2 arguments (conn, surface-id)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let surface_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/commit: surface-id must be integer"),
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/commit: expected wayland connection"),
    };
    let sidx = match wl.find_surface(surface_id) {
        Some(i) => i,
        None => {
            return a.err(
                "wayland-error",
                &format!("surface {} not found", surface_id),
            )
        }
    };
    wl.surfaces[sidx].surface.commit();
    if let Err(e) = wl.conn.flush() {
        return a.err("wayland-error", &format!("flush failed: {}", e));
    }
    a.ok(a.nil())
}

// ── SHM buffer primitives ─────────────────────────────────────────────

extern "C" fn prim_shm_buffer(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 3 {
        return a.err(
            "arity-error",
            "wl/shm-buffer: expected 3 arguments (conn, width, height)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let width = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(w) => w as i32,
        None => return a.err("type-error", "wl/shm-buffer: width must be integer"),
    };
    let height = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(h) => h as i32,
        None => return a.err("type-error", "wl/shm-buffer: height must be integer"),
    };

    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/shm-buffer: expected wayland connection"),
    };

    let shm = match &wl.state.shm {
        Some(s) => s.clone(),
        None => return a.err("wayland-error", "wl_shm not available"),
    };

    let bid = wl.next_buffer_id;
    wl.next_buffer_id += 1;

    let qh = wl.queue.handle();
    match ShmBuffer::create(&shm, &qh, bid, width, height) {
        Ok(buf) => {
            wl.buffers.push(buf);
            a.ok(a.int(bid as i64))
        }
        Err(e) => a.err("wayland-error", &e),
    }
}

extern "C" fn prim_buffer_write(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 4 {
        return a.err(
            "arity-error",
            "wl/buffer-write: expected 4 arguments (conn, buffer-id, offset, data)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let buffer_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/buffer-write: buffer-id must be integer"),
    };
    let offset = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(o) => o as usize,
        None => return a.err("type-error", "wl/buffer-write: offset must be integer"),
    };
    let data_val = unsafe { a.arg(args, nargs, 3) };
    let data = match a.get_bytes(data_val) {
        Some(b) => b,
        None => return a.err("type-error", "wl/buffer-write: data must be bytes"),
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/buffer-write: expected wayland connection"),
    };
    let bidx = match wl.find_buffer(buffer_id) {
        Some(i) => i,
        None => return a.err("wayland-error", &format!("buffer {} not found", buffer_id)),
    };
    match wl.buffers[bidx].write(offset, data) {
        Ok(()) => a.ok(a.nil()),
        Err(e) => a.err("wayland-error", &e),
    }
}

extern "C" fn prim_buffer_fill(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 3 {
        return a.err(
            "arity-error",
            "wl/buffer-fill: expected 3 arguments (conn, buffer-id, color)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let buffer_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/buffer-fill: buffer-id must be integer"),
    };
    let color = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(c) => c as u32,
        None => return a.err("type-error", "wl/buffer-fill: color must be integer (ARGB)"),
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/buffer-fill: expected wayland connection"),
    };
    let bidx = match wl.find_buffer(buffer_id) {
        Some(i) => i,
        None => return a.err("wayland-error", &format!("buffer {} not found", buffer_id)),
    };
    wl.buffers[bidx].fill(color);
    a.ok(a.nil())
}

extern "C" fn prim_buffer_fill_rect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 7 {
        return a.err(
            "arity-error",
            "wl/buffer-fill-rect: expected 7 arguments (conn, buffer-id, x, y, w, h, color)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let buffer_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-rect: buffer-id must be integer",
            )
        }
    };
    let x = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-rect: x must be integer"),
    };
    let y = match a.get_int(unsafe { a.arg(args, nargs, 3) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-rect: y must be integer"),
    };
    let w = match a.get_int(unsafe { a.arg(args, nargs, 4) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-rect: width must be integer"),
    };
    let h = match a.get_int(unsafe { a.arg(args, nargs, 5) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-rect: height must be integer"),
    };
    let color = match a.get_int(unsafe { a.arg(args, nargs, 6) }) {
        Some(c) => c as u32,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-rect: color must be integer (ARGB)",
            )
        }
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-rect: expected wayland connection",
            )
        }
    };
    let bidx = match wl.find_buffer(buffer_id) {
        Some(i) => i,
        None => return a.err("wayland-error", &format!("buffer {} not found", buffer_id)),
    };
    wl.buffers[bidx].fill_rect(x, y, w, h, color);
    a.ok(a.nil())
}

extern "C" fn prim_buffer_fill_circle(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 6 {
        return a.err(
            "arity-error",
            "wl/buffer-fill-circle: expected 6 arguments (conn, buffer-id, cx, cy, r, color)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let buffer_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-circle: buffer-id must be integer",
            )
        }
    };
    let cx = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-circle: cx must be integer"),
    };
    let cy = match a.get_int(unsafe { a.arg(args, nargs, 3) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-circle: cy must be integer"),
    };
    let r = match a.get_int(unsafe { a.arg(args, nargs, 4) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-circle: r must be integer"),
    };
    let color = match a.get_int(unsafe { a.arg(args, nargs, 5) }) {
        Some(c) => c as u32,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-circle: color must be integer (ARGB)",
            )
        }
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-circle: expected wayland connection",
            )
        }
    };
    let bidx = match wl.find_buffer(buffer_id) {
        Some(i) => i,
        None => return a.err("wayland-error", &format!("buffer {} not found", buffer_id)),
    };
    wl.buffers[bidx].fill_circle(cx, cy, r, color);
    a.ok(a.nil())
}

extern "C" fn prim_buffer_fill_triangle(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 9 {
        return a.err("arity-error", "wl/buffer-fill-triangle: expected 9 arguments (conn, buffer-id, x1, y1, x2, y2, x3, y3, color)");
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let buffer_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-triangle: buffer-id must be integer",
            )
        }
    };
    let x1 = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-triangle: x1 must be integer"),
    };
    let y1 = match a.get_int(unsafe { a.arg(args, nargs, 3) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-triangle: y1 must be integer"),
    };
    let x2 = match a.get_int(unsafe { a.arg(args, nargs, 4) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-triangle: x2 must be integer"),
    };
    let y2 = match a.get_int(unsafe { a.arg(args, nargs, 5) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-triangle: y2 must be integer"),
    };
    let x3 = match a.get_int(unsafe { a.arg(args, nargs, 6) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-triangle: x3 must be integer"),
    };
    let y3 = match a.get_int(unsafe { a.arg(args, nargs, 7) }) {
        Some(v) => v as i32,
        None => return a.err("type-error", "wl/buffer-fill-triangle: y3 must be integer"),
    };
    let color = match a.get_int(unsafe { a.arg(args, nargs, 8) }) {
        Some(c) => c as u32,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-triangle: color must be integer (ARGB)",
            )
        }
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-fill-triangle: expected wayland connection",
            )
        }
    };
    let bidx = match wl.find_buffer(buffer_id) {
        Some(i) => i,
        None => return a.err("wayland-error", &format!("buffer {} not found", buffer_id)),
    };
    wl.buffers[bidx].fill_triangle(x1, y1, x2, y2, x3, y3, color);
    a.ok(a.nil())
}

extern "C" fn prim_buffer_destroy(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(
            "arity-error",
            "wl/buffer-destroy: expected 2 arguments (conn, buffer-id)",
        );
    }
    let conn_val = unsafe { a.arg(args, nargs, 0) };
    let buffer_id = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(id) => id as u32,
        None => return a.err("type-error", "wl/buffer-destroy: buffer-id must be integer"),
    };
    let wl = match unsafe { a.get_external_mut::<WlConn>(conn_val, "wayland-connection") } {
        Some(w) => w,
        None => {
            return a.err(
                "type-error",
                "wl/buffer-destroy: expected wayland connection",
            )
        }
    };
    if let Some(idx) = wl.find_buffer(buffer_id) {
        wl.buffers.swap_remove(idx);
    }
    a.ok(a.nil())
}

// ── Screencopy primitives ─────────────────────────────────────────────

extern "C" fn prim_screencopy(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

extern "C" fn prim_screencopy_destroy(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

// ── Foreign toplevel primitives ───────────────────────────────────────

extern "C" fn prim_toplevels(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().array(&[]))
}

extern "C" fn prim_toplevel_activate(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

extern "C" fn prim_toplevel_close(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

extern "C" fn prim_toplevel_subscribe(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

// ── Registration table ────────────────────────────────────────────────

static PRIMITIVES: &[EllePrimDef] = &[
    // Connection
    EllePrimDef::exact("wl/connect", prim_connect, SIG_OK, 0,
        "Connect to the Wayland display server.", "wayland", ""),
    EllePrimDef::exact("wl/disconnect", prim_disconnect, SIG_OK, 1,
        "Disconnect from the Wayland display server.", "wayland", ""),
    EllePrimDef::exact("wl/display-fd", prim_display_fd, SIG_OK, 1,
        "Get the display connection file descriptor.", "wayland", ""),
    EllePrimDef::exact("wl/dispatch", prim_dispatch, SIG_OK, 1,
        "Dispatch pending Wayland events.", "wayland", ""),
    EllePrimDef::exact("wl/flush", prim_flush, SIG_OK, 1,
        "Flush the Wayland connection.", "wayland", ""),
    EllePrimDef::exact("wl/poll-events", prim_poll_events, SIG_OK, 1,
        "Drain buffered events as an array of structs.", "wayland", ""),

    // Queries
    EllePrimDef::exact("wl/outputs", prim_outputs, SIG_OK, 1,
        "List connected outputs.", "wayland", ""),
    EllePrimDef::exact("wl/seats", prim_seats, SIG_OK, 1,
        "List available seats.", "wayland", ""),

    // Layer shell
    EllePrimDef::at_least("wl/layer-surface", prim_layer_surface, SIG_OK, 1,
        "Create a layer-shell surface. Optional opts struct with :layer, :anchor, :width, :height, :exclusive-zone.", "wayland", ""),
    EllePrimDef::exact("wl/layer-configure", prim_layer_configure, SIG_OK, 2,
        "Acknowledge a layer surface configure.", "wayland", ""),
    EllePrimDef::exact("wl/layer-destroy", prim_layer_destroy, SIG_OK, 2,
        "Destroy a layer surface.", "wayland", ""),

    // Surface ops
    EllePrimDef::exact("wl/attach", prim_attach, SIG_OK, 3,
        "Attach a buffer to a surface.", "wayland", ""),
    EllePrimDef::exact("wl/damage", prim_damage, SIG_OK, 6,
        "Damage a region of a surface.", "wayland", ""),
    EllePrimDef::exact("wl/commit", prim_commit, SIG_OK, 2,
        "Commit a surface.", "wayland", ""),

    // SHM buffers
    EllePrimDef::exact("wl/shm-buffer", prim_shm_buffer, SIG_OK, 3,
        "Create an SHM buffer (conn, width, height).", "wayland", ""),
    EllePrimDef::exact("wl/buffer-write", prim_buffer_write, SIG_OK, 4,
        "Write bytes to an SHM buffer at offset.", "wayland", ""),
    EllePrimDef::exact("wl/buffer-fill", prim_buffer_fill, SIG_OK, 3,
        "Fill an SHM buffer with an ARGB color.", "wayland", ""),
    EllePrimDef::exact("wl/buffer-fill-rect", prim_buffer_fill_rect, SIG_OK, 7,
        "Fill a rectangular region of an SHM buffer with an ARGB color.", "wayland", ""),
    EllePrimDef::exact("wl/buffer-fill-circle", prim_buffer_fill_circle, SIG_OK, 6,
        "Fill a circle region of an SHM buffer with an ARGB color.", "wayland", ""),
    EllePrimDef::exact("wl/buffer-fill-triangle", prim_buffer_fill_triangle, SIG_OK, 9,
        "Fill a triangle region of an SHM buffer with an ARGB color.", "wayland", ""),
    EllePrimDef::exact("wl/buffer-destroy", prim_buffer_destroy, SIG_OK, 2,
        "Destroy an SHM buffer.", "wayland", ""),

    // Screencopy
    EllePrimDef::exact("wl/screencopy", prim_screencopy, SIG_OK, 2,
        "Capture a screencopy frame from an output.", "wayland", ""),
    EllePrimDef::exact("wl/screencopy-destroy", prim_screencopy_destroy, SIG_OK, 2,
        "Destroy a screencopy frame.", "wayland", ""),

    // Foreign toplevel
    EllePrimDef::exact("wl/toplevels", prim_toplevels, SIG_OK, 1,
        "List foreign toplevels (windows).", "wayland", ""),
    EllePrimDef::exact("wl/toplevel-activate", prim_toplevel_activate, SIG_OK, 3,
        "Activate (focus) a toplevel window.", "wayland", ""),
    EllePrimDef::exact("wl/toplevel-close", prim_toplevel_close, SIG_OK, 2,
        "Request a toplevel window to close.", "wayland", ""),
    EllePrimDef::exact("wl/toplevel-subscribe", prim_toplevel_subscribe, SIG_OK, 1,
        "Subscribe to toplevel events.", "wayland", ""),
];
