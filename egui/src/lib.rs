//! Elle egui plugin — thin wrapper over egui + winit + glow.
//!
//! Provides window lifecycle and synchronous frame rendering.
//! All I/O awareness (ev/poll-fd) lives in the Elle library.

mod ui;
mod window;

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK, SIG_ERROR};
use std::cell::RefCell;

use crate::ui::Interactions;
use crate::window::WindowState;

elle_plugin::define_plugin!("egui/", &PRIMITIVES);

// ── Helpers ──────────────────────────────────────────────────────────

fn get_state<'a>(val: ElleValue) -> Result<&'a RefCell<WindowState>, ElleResult> {
    let a = api();
    a.get_external::<RefCell<WindowState>>(val, "egui-window").ok_or_else(|| {
        a.err("type-error", &format!("expected egui-window handle, got {}", a.type_name(val)))
    })
}

fn egui_err(name: &str, msg: impl std::fmt::Display) -> ElleResult {
    api().err("egui-error", &format!("{}: {}", name, msg))
}

// ── Primitives ───────────────────────────────────────────────────────

extern "C" fn prim_open(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let mut title = "Elle".to_string();
    let mut width = 800.0;
    let mut height = 600.0;

    if nargs > 0 {
        let opts = unsafe { a.arg(args, nargs, 0) };
        if a.check_struct(opts) {
            let tv = a.get_struct_field(opts, "title");
            if let Some(t) = a.get_string(tv) { title = t.to_string(); }
            let wv = a.get_struct_field(opts, "width");
            if let Some(w) = a.get_float(wv).or_else(|| a.get_int(wv).map(|i| i as f64)) { width = w; }
            let hv = a.get_struct_field(opts, "height");
            if let Some(h) = a.get_float(hv).or_else(|| a.get_int(hv).map(|i| i as f64)) { height = h; }
        }
    }

    match WindowState::new(&title, width, height) {
        Ok(state) => a.ok(a.external("egui-window", RefCell::new(state))),
        Err(e) => egui_err("egui/open", e),
    }
}

extern "C" fn prim_display_fd(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let fd = state.borrow().display_fd;
    a.ok(a.int(fd as i64))
}

extern "C" fn prim_frame(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let tree_val = unsafe { a.arg(args, nargs, 1) };
    let nodes = match ui::value_to_tree(tree_val) { Ok(n) => n, Err(e) => return e };
    let mut state = state.borrow_mut();
    state.pump_events();
    let mut ix = Interactions::default();
    state.frame_with_tree(&nodes, &mut ix);
    a.ok(ui::interactions_to_value(&ix))
}

extern "C" fn prim_close(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let mut state = state.borrow_mut();
    state.close_requested = true;
    state.painter = None;
    state.egui_winit = None;
    state.gl = None;
    state.gl_context = None;
    state.gl_surface = None;
    state.window = None;
    state.event_loop = None;
    a.ok(a.nil())
}

extern "C" fn prim_open_p(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let state = state.borrow();
    a.ok(a.boolean(!state.close_requested && state.window.is_some()))
}

extern "C" fn prim_set_text(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let id = match a.get_keyword_name(v1) { Some(s) => s.to_string(), None => return egui_err("egui/set-text", format!("id must be a keyword, got {}", a.type_name(v1))) };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let val = match a.get_string(v2) { Some(s) => s.to_string(), None => return egui_err("egui/set-text", format!("value must be a string, got {}", a.type_name(v2))) };
    state.borrow_mut().widget_state.text_buffers.insert(id, val);
    a.ok(a.nil())
}

extern "C" fn prim_set_check(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let id = match a.get_keyword_name(v1) { Some(s) => s.to_string(), None => return egui_err("egui/set-check", "id must be a keyword") };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let val = match a.get_bool(v2) { Some(b) => b, None => return egui_err("egui/set-check", "value must be a boolean") };
    state.borrow_mut().widget_state.check_states.insert(id, val);
    a.ok(a.nil())
}

extern "C" fn prim_set_slider(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let id = match a.get_keyword_name(v1) { Some(s) => s.to_string(), None => return egui_err("egui/set-slider", "id must be a keyword") };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let val = a.get_float(v2).or_else(|| a.get_int(v2).map(|i| i as f64));
    match val {
        Some(v) => { state.borrow_mut().widget_state.slider_states.insert(id, v); a.ok(a.nil()) }
        None => egui_err("egui/set-slider", "value must be a number"),
    }
}

extern "C" fn prim_set_title(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let title = match a.get_string(v1) { Some(s) => s.to_string(), None => return egui_err("egui/set-title", "title must be a string") };
    let state = state.borrow();
    if let Some(ref window) = state.window { window.set_title(&title); }
    a.ok(a.nil())
}

extern "C" fn prim_set_combo(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_state(unsafe { a.arg(args, nargs, 0) }) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let id = match a.get_keyword_name(v1) { Some(s) => s.to_string(), None => return egui_err("egui/set-combo", "id must be a keyword") };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let val = match a.get_string(v2) { Some(s) => s.to_string(), None => return egui_err("egui/set-combo", "value must be a string") };
    state.borrow_mut().widget_state.combo_states.insert(id, val);
    a.ok(a.nil())
}

// ── Primitive table ──────────────────────────────────────────────────

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range("egui/open", prim_open, SIG_ERROR, 0, 1, "Open a GUI window. Optional opts: {:title \"name\"}", "egui", "(egui/open {:title \"My App\"})"),
    EllePrimDef::exact("egui/display-fd", prim_display_fd, SIG_OK, 1, "Return the display connection fd for ev/poll-fd.", "egui", "(egui/display-fd handle)"),
    EllePrimDef::exact("egui/frame", prim_frame, SIG_ERROR, 2, "Render one frame. Pumps events, renders tree, returns interactions.", "egui", "(egui/frame handle [:label \"hello\"])"),
    EllePrimDef::exact("egui/close", prim_close, SIG_ERROR, 1, "Close the window and release resources.", "egui", "(egui/close handle)"),
    EllePrimDef::exact("egui/open?", prim_open_p, SIG_OK, 1, "Check if the window is still open.", "egui", "(egui/open? handle)"),
    EllePrimDef::exact("egui/set-text", prim_set_text, SIG_ERROR, 3, "Set a text input's buffer value.", "egui", "(egui/set-text handle :name \"world\")"),
    EllePrimDef::exact("egui/set-check", prim_set_check, SIG_ERROR, 3, "Set a checkbox state.", "egui", "(egui/set-check handle :agree true)"),
    EllePrimDef::exact("egui/set-slider", prim_set_slider, SIG_ERROR, 3, "Set a slider value.", "egui", "(egui/set-slider handle :volume 50.0)"),
    EllePrimDef::exact("egui/set-title", prim_set_title, SIG_ERROR, 2, "Change the window title.", "egui", "(egui/set-title handle \"New Title\")"),
    EllePrimDef::exact("egui/set-combo", prim_set_combo, SIG_ERROR, 3, "Set a combo box selection.", "egui", "(egui/set-combo handle :theme \"dark\")"),
];
