//! Layer-shell surface management.

use wayland_client::protocol::wl_surface;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::state::{WaylandState, WlEvent};

// ── Layer surface tracking ────────────────────────────────────────────

pub struct LayerSurface {
    pub id: u32,
    pub surface: wl_surface::WlSurface,
    pub layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    pub configured: bool,
    pub width: i32,
    pub height: i32,
}

// ── Dispatch impls ────────────────────────────────────────────────────

impl Dispatch<wl_surface::WlSurface, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _surface: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
    }
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _event: zwlr_layer_shell_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, u32> for WaylandState {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        surface_id: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer_surface.ack_configure(serial);
                state.events.push(WlEvent::Configure {
                    surface_id: *surface_id,
                    serial,
                    width: width as i32,
                    height: height as i32,
                });
            }
            zwlr_layer_surface_v1::Event::Closed => {
                state.events.push(WlEvent::Closed {
                    surface_id: *surface_id,
                });
            }
            _ => {}
        }
    }
}
