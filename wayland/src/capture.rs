//! Screencopy frame capture via wlr-screencopy-unstable-v1.

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};

use crate::state::{WaylandState, WlEvent};

// ── Screencopy tracking ───────────────────────────────────────────────

pub struct ScreencopyFrame {
    pub id: u32,
    pub frame: zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub stride: u32,
}

// ── Dispatch impls ────────────────────────────────────────────────────

impl Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
        _event: zwlr_screencopy_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
    }
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, u32> for WaylandState {
    fn event(
        state: &mut Self,
        _frame: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        frame_id: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                state.events.push(WlEvent::ScreencopyReady {
                    frame_id: *frame_id,
                });
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                state.events.push(WlEvent::ScreencopyFailed {
                    frame_id: *frame_id,
                });
            }
            _ => {}
        }
    }
}
