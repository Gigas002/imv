//! Wayland client integration: surface management, SHM buffers, and input.
//!
//! [`WaylandContext`] is the public entry point. It wraps [`WaylandState`]
//! (the dispatch target) together with the [`EventQueue`] so that
//! [`WaylandContext::dispatch`] can borrow both fields without conflict.
//!
//! Submodules:
//! - [`shm`]: SHM pool backed by a `memfd` (Phase 5.1)
//! - [`keyboard`]: xkbcommon keymap and key-event handling (Phase 5.2)

pub mod keyboard;
pub mod shm;

use std::io;

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
    protocol::{wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm, wl_surface},
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use xkbcommon::xkb::Keysym;

#[cfg(feature = "decorations")]
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};

use crate::wayland::keyboard::{KeyboardState, key_event, update_keymap};

// ── Input events ────────────────────────────────────────────────────────────

/// Input events collected during dispatch and consumed by the event loop.
pub enum InputEvent {
    /// A key was pressed; carries the resolved XKB keysym.
    Key(Keysym),
    /// Vertical scroll wheel tick. Positive = scroll down.
    Scroll(f32),
    /// Left mouse button pressed (`true`) or released (`false`).
    PointerButton { pressed: bool },
    /// Mouse moved while the button was held; delta in surface pixels.
    PointerMotion { dx: f32, dy: f32 },
}

// ── WaylandState ────────────────────────────────────────────────────────────

/// Wayland protocol objects and collected event state.
///
/// This struct is the dispatch target for [`EventQueue`]; it must not contain
/// the queue itself. See [`WaylandContext`] for the combined public type.
pub struct WaylandState {
    qh: QueueHandle<WaylandState>,

    // Globals bound from the registry
    compositor: Option<wl_compositor::WlCompositor>,
    wl_shm: Option<wl_shm::WlShm>,
    xdg_wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,

    #[cfg(feature = "decorations")]
    decoration_manager: Option<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1>,

    // Surface objects created after the first roundtrip
    surface: Option<wl_surface::WlSurface>,
    xdg_surface: Option<xdg_surface::XdgSurface>,
    xdg_toplevel: Option<xdg_toplevel::XdgToplevel>,

    // Input objects and state
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_state: Option<KeyboardState>,
    pointer: Option<wl_pointer::WlPointer>,
    pointer_pos: (f64, f64),
    pointer_pressed: bool,

    /// Set to `true` when the compositor requests the window be closed.
    pub closed: bool,
    /// Set to `true` when a redraw is needed (configure, input, etc.).
    pub needs_redraw: bool,
    /// Current window size in pixels as reported by the compositor.
    pub window_size: (u32, u32),
    /// Events accumulated since the last drain by the event loop.
    pub pending_events: Vec<InputEvent>,
}

impl WaylandState {
    fn new(qh: QueueHandle<WaylandState>, initial_size: (u32, u32)) -> Self {
        WaylandState {
            qh,
            compositor: None,
            wl_shm: None,
            xdg_wm_base: None,
            seat: None,
            #[cfg(feature = "decorations")]
            decoration_manager: None,
            surface: None,
            xdg_surface: None,
            xdg_toplevel: None,
            keyboard: None,
            keyboard_state: None,
            pointer: None,
            pointer_pos: (0.0, 0.0),
            pointer_pressed: false,
            closed: false,
            needs_redraw: false,
            window_size: initial_size,
            pending_events: Vec::new(),
        }
    }

    /// Expose `wl_shm` for Phase 6 buffer creation.
    pub fn wl_shm(&self) -> Option<&wl_shm::WlShm> {
        self.wl_shm.as_ref()
    }

    /// Expose the `wl_surface` for attaching buffers.
    pub fn surface(&self) -> Option<&wl_surface::WlSurface> {
        self.surface.as_ref()
    }

    /// Expose `QueueHandle` for creating objects in Phase 6.
    pub fn qh(&self) -> &QueueHandle<WaylandState> {
        &self.qh
    }
}

// ── WaylandContext ───────────────────────────────────────────────────────────

/// Owns the Wayland connection, state, and event queue.
///
/// The [`EventQueue`] and [`WaylandState`] are separate fields so that
/// `dispatch` can borrow them disjointly without unsafe code.
pub struct WaylandContext {
    conn: Connection,
    /// Public Wayland state — surfaces, globals, pending events, flags.
    pub state: WaylandState,
    event_queue: EventQueue<WaylandState>,
}

impl WaylandContext {
    /// Connect to the Wayland compositor and initialise surfaces.
    ///
    /// Performs two roundtrips: one to enumerate globals, one to receive the
    /// initial `xdg_toplevel::configure`.
    pub fn connect(initial_size: (u32, u32)) -> io::Result<Self> {
        let conn = Connection::connect_to_env()
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;
        let mut event_queue = conn.new_event_queue::<WaylandState>();
        let qh = event_queue.handle();

        let mut state = WaylandState::new(qh.clone(), initial_size);

        conn.display().get_registry(&qh, ());
        event_queue
            .roundtrip(&mut state)
            .map_err(io::Error::other)?;

        if state.compositor.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "wl_compositor not found",
            ));
        }
        if state.xdg_wm_base.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "xdg_wm_base not found",
            ));
        }

        let surface = state.compositor.as_ref().unwrap().create_surface(&qh, ());
        let xdg_surf = state
            .xdg_wm_base
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surf.get_toplevel(&qh, ());
        toplevel.set_app_id("imgvwr".to_string());

        #[cfg(feature = "decorations")]
        if let Some(mgr) = &state.decoration_manager {
            let _deco = mgr.get_toplevel_decoration(&toplevel, &qh, ());
        }

        surface.commit();

        state.surface = Some(surface);
        state.xdg_surface = Some(xdg_surf);
        state.xdg_toplevel = Some(toplevel);

        event_queue
            .roundtrip(&mut state)
            .map_err(io::Error::other)?;

        Ok(WaylandContext {
            conn,
            state,
            event_queue,
        })
    }

    /// Write `pixels` (ARGB8888, `w × h × 4` bytes) into a Wayland SHM buffer
    /// and commit it to the surface.
    ///
    /// Implemented in Phase 6.2.
    pub fn commit_frame(&mut self, _pixels: &[u8], _w: u32, _h: u32) -> io::Result<()> {
        todo!("Phase 6.2: SHM buffer commit")
    }

    /// Flush the outgoing Wayland socket buffer.
    pub fn flush(&self) -> io::Result<()> {
        self.conn
            .flush()
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
    }

    /// Dispatch pending events, blocking until at least one arrives.
    ///
    /// `timeout_ms` is accepted for API symmetry with Phase 6 but the current
    /// implementation uses `blocking_dispatch` without a hard timeout.
    pub fn dispatch(&mut self, _timeout_ms: i32) -> io::Result<()> {
        self.event_queue
            .blocking_dispatch(&mut self.state)
            .map(|_| ())
            .map_err(io::Error::other)
    }
}

// ── Dispatch implementations ─────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_compositor" => {
                state.compositor = Some(registry.bind(name, version.min(6), qh, ()));
            }
            "wl_shm" => {
                state.wl_shm = Some(registry.bind(name, version.min(1), qh, ()));
            }
            "xdg_wm_base" => {
                state.xdg_wm_base = Some(registry.bind(name, version.min(5), qh, ()));
            }
            "wl_seat" => {
                state.seat = Some(registry.bind(name, version.min(9), qh, ()));
            }
            #[cfg(feature = "decorations")]
            "zxdg_decoration_manager_v1" => {
                state.decoration_manager = Some(registry.bind(name, version.min(1), qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for WaylandState {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            } => {
                if let Ok(ks) = update_keymap(fd, size) {
                    state.keyboard_state = Some(ks);
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                let sym = if let Some(ks) = state.keyboard_state.as_mut() {
                    if let WEnum::Value(ks_val) = key_state {
                        key_event(ks, key, ks_val)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(sym) = sym {
                    state.pending_events.push(InputEvent::Key(sym));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Button {
                state: WEnum::Value(btn),
                ..
            } => {
                let pressed = btn == wl_pointer::ButtonState::Pressed;
                state.pointer_pressed = pressed;
                state
                    .pending_events
                    .push(InputEvent::PointerButton { pressed });
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if state.pointer_pressed {
                    let dx = (surface_x - state.pointer_pos.0) as f32;
                    let dy = (surface_y - state.pointer_pos.1) as f32;
                    state
                        .pending_events
                        .push(InputEvent::PointerMotion { dx, dy });
                }
                state.pointer_pos = (surface_x, surface_y);
            }
            wl_pointer::Event::Axis {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } => {
                state.pending_events.push(InputEvent::Scroll(-value as f32));
            }
            _ => {}
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for WaylandState {
    fn event(
        state: &mut Self,
        xdg_surf: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surf.ack_configure(serial);
            if let Some(surface) = &state.surface {
                surface.commit();
            }
            state.needs_redraw = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                let w = if width == 0 {
                    state.window_size.0 as i32
                } else {
                    width
                };
                let h = if height == 0 {
                    state.window_size.1 as i32
                } else {
                    height
                };
                let new_size = (w as u32, h as u32);
                if new_size != state.window_size {
                    state.window_size = new_size;
                }
                state.needs_redraw = true;
            }
            xdg_toplevel::Event::Close => {
                state.closed = true;
            }
            _ => {}
        }
    }
}

#[cfg(feature = "decorations")]
impl Dispatch<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        _: zxdg_decoration_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(feature = "decorations")]
impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        _: zxdg_toplevel_decoration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
