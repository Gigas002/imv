//! Wayland client integration: surface management, SHM buffers, and input.
//!
//! Implemented incrementally across Phase 5 steps:
//! - [`shm`]: SHM pool backed by a `memfd` (Phase 5.1)
//! - [`keyboard`]: xkbcommon keymap and key-event handling (Phase 5.2)
//! - The [`WaylandState`] toplevel (Phase 5.3)

pub mod keyboard;
pub mod shm;
