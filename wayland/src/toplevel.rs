//! Foreign-toplevel management — window listing, activate, close.

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1, zwlr_foreign_toplevel_manager_v1,
};

use crate::state::{WaylandState, WlEvent};

// ── Toplevel tracking ─────────────────────────────────────────────────

pub struct ToplevelInfo {
    pub id: u32,
    pub handle: zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    pub title: String,
    pub app_id: String,
    pub state: Vec<String>,
}

// ── Dispatch impls ────────────────────────────────────────────────────

impl Dispatch<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        _event: zwlr_foreign_toplevel_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        // Toplevel events are handled in the handle dispatch
    }
}

impl Dispatch<zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, u32> for WaylandState {
    fn event(
        state: &mut Self,
        _handle: &zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        toplevel_id: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                state.events.push(WlEvent::ToplevelNew {
                    id: *toplevel_id,
                    title,
                    app_id: String::new(),
                });
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                state
                    .events
                    .push(WlEvent::ToplevelClosed { id: *toplevel_id });
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                state.events.push(WlEvent::ToplevelDone {
                    id: *toplevel_id,
                    title: String::new(),
                    state: Vec::new(),
                });
            }
            _ => {}
        }
    }
}
