//! Shared frontend data models, constants, and Tauri IPC payloads.

use serde::{Deserialize, Serialize};

/// Sentinel key representing the combined view across all configured libraries.
pub const ALL_LIBRARIES_KEY: &str = "*";

/// Wire-format identifiers for backend `PlayMode` variants.
pub const MODE_CONTINUE_NEXT: &str = "continue_next";
pub const MODE_PLAY_ALL: &str = "play_all";
pub const MODE_SELECTED: &str = "selected";
pub const MODE_ONE: &str = "one";

/// Default audio volume (`0..=100`) before the first status response arrives.
pub const DEFAULT_VOLUME: i32 = 80;
/// Interval in milliseconds between backend `status` polls.
pub const POLL_INTERVAL_MS: u64 = 250;
/// Short delay after toggling the sidebar so layout reflow finishes before measuring `.video-area`.
pub const BOUNDS_SETTLE_DELAY_MS: u64 = 30;

/// Metadata for the currently loaded media file.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub thumbnail: String,
}

/// A configured media library folder displayed as a chip in the sidebar.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub path: String,
    pub name: String,
}

/// A single audio or video track returned by the backend library scanner.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub path: String,
    pub name: String,
    pub folder: String,
    pub kind: String,
    pub thumbnail: String,
}

/// Snapshot of configured libraries, active selection, and scanned media items.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryResponse {
    pub folders: Vec<LibraryFolder>,
    pub active_folder: Option<String>,
    pub items: Vec<LibraryItem>,
}

/// Live playback state polled from the backend.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: String,
    pub time_ms: i64,
    pub length_ms: i64,
    pub volume: i32,
    pub has_video: bool,
    pub play_mode: String,
    pub media: Option<MediaInfo>,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            state: String::new(),
            time_ms: 0,
            length_ms: 0,
            volume: DEFAULT_VOLUME,
            has_video: false,
            play_mode: MODE_CONTINUE_NEXT.to_string(),
            media: None,
        }
    }
}

/// Content rendered inside `.video-area` whenever the native VLC video surface is hidden.
#[derive(Clone, Debug, PartialEq)]
pub enum Placeholder {
    Empty,
    Audio { name: String, thumbnail: String },
    Ready(String),
}

#[derive(Serialize)]
pub struct BoundsArgs {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize)]
pub struct SeekArgs {
    pub ms: i64,
}

#[derive(Serialize)]
pub struct VolumeArgs {
    pub volume: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayFileArgs {
    pub path: String,
    pub selected_paths: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedArgs {
    pub selected_paths: Vec<String>,
}

#[derive(Serialize)]
pub struct ModeArgs {
    pub mode: String,
}

#[derive(Serialize)]
pub struct FolderArgs {
    pub folder: String,
}
