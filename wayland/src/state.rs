//! Wayland state and event buffer.
//!
//! Dispatch implementations push events to a Vec<WlEvent>; Elle drains
//! via wl/poll-events. No calloop — Elle's ev/poll-fd drives the loop.

use wayland_client::protocol::{wl_output, wl_registry, wl_seat, wl_shm};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;

// ── Events ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum WlEvent {
    Output {
        id: u32,
        name: String,
        width: i32,
        height: i32,
        scale: i32,
    },
    Seat {
        id: u32,
        name: String,
        caps: u32,
    },
    Configure {
        surface_id: u32,
        serial: u32,
        width: i32,
        height: i32,
    },
    Closed {
        surface_id: u32,
    },
    BufferRelease {
        buffer_id: u32,
    },
    ScreencopyReady {
        frame_id: u32,
    },
    ScreencopyFailed {
        frame_id: u32,
    },
    ToplevelNew {
        id: u32,
        title: String,
        app_id: String,
    },
    ToplevelDone {
        id: u32,
        title: String,
        state: Vec<String>,
    },
    ToplevelClosed {
        id: u32,
    },
}

// ── Wayland state ─────────────────────────────────────────────────────

pub struct OutputInfo {
    pub id: u32,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub scale: i32,
}

pub struct SeatInfo {
    pub id: u32,
    pub name: String,
    pub caps: u32,
}

pub struct WaylandState {
    pub events: Vec<WlEvent>,
    pub outputs: Vec<OutputInfo>,
    pub seats: Vec<SeatInfo>,
    pub shm: Option<wl_shm::WlShm>,
    pub compositor: Option<wayland_client::protocol::wl_compositor::WlCompositor>,
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    next_output_id: u32,
    next_seat_id: u32,
}

impl WaylandState {
    pub fn new() -> Self {
        WaylandState {
            events: Vec::new(),
            outputs: Vec::new(),
            seats: Vec::new(),
            shm: None,
            compositor: None,
            layer_shell: None,
            next_output_id: 1,
            next_seat_id: 1,
        }
    }
}

// ── Registry dispatch ─────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<WaylandState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_output" => {
                    registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
                }
                "wl_seat" => {
                    registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(5), qh, ());
                }
                "wl_shm" => {
                    state.shm =
                        Some(registry.bind::<wl_shm::WlShm, _, _>(name, version.min(1), qh, ()));
                }
                "wl_compositor" => {
                    state.compositor = Some(
                        registry
                            .bind::<wayland_client::protocol::wl_compositor::WlCompositor, _, _>(
                                name,
                                version.min(4),
                                qh,
                                (),
                            ),
                    );
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(
                        registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                            name,
                            version.min(4),
                            qh,
                            (),
                        ),
                    );
                }
                _ => {}
            }
        }
    }
}

// ── Output dispatch ───────────────────────────────────────────────────

impl Dispatch<wl_output::WlOutput, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _output: &wl_output::WlOutput,
        event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        match event {
            wl_output::Event::Name { name } => {
                let id = state.next_output_id;
                // Find or create the output info entry
                if state.outputs.iter().all(|o| o.name != name) {
                    state.outputs.push(OutputInfo {
                        id,
                        name: name.clone(),
                        width: 0,
                        height: 0,
                        scale: 1,
                    });
                    state.next_output_id += 1;
                }
            }
            wl_output::Event::Mode {
                flags: WEnum::Value(flags),
                width,
                height,
                ..
            } if flags.contains(wl_output::Mode::Current) => {
                if let Some(info) = state.outputs.last_mut() {
                    info.width = width;
                    info.height = height;
                }
            }
            wl_output::Event::Mode { .. } => {}
            wl_output::Event::Scale { factor } => {
                if let Some(info) = state.outputs.last_mut() {
                    info.scale = factor;
                }
            }
            wl_output::Event::Done => {
                if let Some(info) = state.outputs.last() {
                    state.events.push(WlEvent::Output {
                        id: info.id,
                        name: info.name.clone(),
                        width: info.width,
                        height: info.height,
                        scale: info.scale,
                    });
                }
            }
            _ => {}
        }
    }
}

// ── Seat dispatch ─────────────────────────────────────────────────────

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        match event {
            wl_seat::Event::Name { name } => {
                let id = state.next_seat_id;
                state.next_seat_id += 1;
                state.seats.push(SeatInfo {
                    id,
                    name: name.clone(),
                    caps: 0,
                });
            }
            wl_seat::Event::Capabilities {
                capabilities: WEnum::Value(caps),
            } => {
                let bits = caps.bits();
                if let Some(info) = state.seats.last_mut() {
                    info.caps = bits;
                    state.events.push(WlEvent::Seat {
                        id: info.id,
                        name: info.name.clone(),
                        caps: bits,
                    });
                }
            }
            _ => {}
        }
    }
}

// ── SHM dispatch ──────────────────────────────────────────────────────

impl Dispatch<wl_shm::WlShm, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _shm: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        // Format events are informational — we always use ARGB8888
    }
}

// ── Compositor dispatch ───────────────────────────────────────────────

impl Dispatch<wayland_client::protocol::wl_compositor::WlCompositor, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _compositor: &wayland_client::protocol::wl_compositor::WlCompositor,
        _event: wayland_client::protocol::wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
    }
}
