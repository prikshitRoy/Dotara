//! Pure playback mode and playlist queue logic, independent of VLC or Tauri.

mod mode;
mod queue;

pub use mode::PlayMode;
pub use queue::PlaybackQueue;
