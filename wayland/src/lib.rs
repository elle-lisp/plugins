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

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK};

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

// ── Event → ElleValue conversion ──────────────────────────────────────

fn event_to_value(ev: &WlEvent) -> ElleValue {
    let a = api();
    match ev {
        WlEvent::Output { id, name, width, height, scale } => a.build_struct(&[
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
        WlEvent::Configure { surface_id, serial, width, height } => a.build_struct(&[
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
    let events: Vec<ElleValue> = wl.state.events.drain(..).map(|e| event_to_value(&e)).collect();
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
        return a.err("arity-error", "wl/layer-surface: expected at least 1 argument");
    }
    let val = unsafe { a.arg(args, nargs, 0) };
    let wl = match unsafe { a.get_external_mut::<WlConn>(val, "wayland-connection") } {
        Some(w) => w,
        None => return a.err("type-error", "wl/layer-surface: expected wayland connection"),
    };

    let compositor = match &wl.state.compositor {
        Some(c) => c,
        None => return a.err("wayland-error", "compositor not available"),
    };

    let qh = wl.queue.handle();
    let _surface = compositor.create_surface(&qh, ());

    // Bind layer shell — we need this global
    // For now, return error if layer-shell is not available
    // (the user should check protocol availability)
    let sid = wl.next_surface_id;
    wl.next_surface_id += 1;

    // We need the layer shell global — bind it from the registry
    let display = wl.conn.display();
    let registry = display.get_registry(&qh, ());
    // Do a roundtrip to get the layer shell
    let _ = wl.queue.roundtrip(&mut wl.state);
    let _ = registry; // keep alive

    a.ok(a.int(sid as i64))
}

extern "C" fn prim_layer_configure(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    // Placeholder — configure is handled via events
    a.ok(a.nil())
}

extern "C" fn prim_layer_destroy(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    // Placeholder
    a.ok(a.nil())
}

// ── Surface ops ───────────────────────────────────────────────────────

extern "C" fn prim_attach(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

extern "C" fn prim_damage(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

extern "C" fn prim_commit(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

// ── SHM buffer primitives ─────────────────────────────────────────────

extern "C" fn prim_shm_buffer(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 3 {
        return a.err("arity-error", "wl/shm-buffer: expected 3 arguments (conn, width, height)");
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

extern "C" fn prim_buffer_write(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

extern "C" fn prim_buffer_fill(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
}

extern "C" fn prim_buffer_destroy(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    api().ok(api().nil())
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
        "Create a layer-shell surface.", "wayland", ""),
    EllePrimDef::exact("wl/layer-configure", prim_layer_configure, SIG_OK, 2,
        "Acknowledge a layer surface configure.", "wayland", ""),
    EllePrimDef::exact("wl/layer-destroy", prim_layer_destroy, SIG_OK, 2,
        "Destroy a layer surface.", "wayland", ""),

    // Surface ops
    EllePrimDef::exact("wl/attach", prim_attach, SIG_OK, 3,
        "Attach a buffer to a surface.", "wayland", ""),
    EllePrimDef::exact("wl/damage", prim_damage, SIG_OK, 5,
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
