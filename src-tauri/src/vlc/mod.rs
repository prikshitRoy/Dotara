//! Runtime-loaded libVLC 3.x playback engine.
//!
//! Resolves `libvlc.dll` dynamically via `libloading` so no VLC SDK or import
//! libraries are needed at build time.

mod api;
mod player;

pub use player::{Player, State};
