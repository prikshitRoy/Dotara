//! Shared application state bridging libVLC, the video surface, libraries, and playback queue.

use crate::library::{self, LibraryResponse};
use crate::playback::{PlayMode, PlaybackQueue};
use crate::surface::VideoSurface;
use crate::vlc::{Player, State as VlcState};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Initial volume level applied once VLC initializes an audio output (`0..=100`).
pub const DEFAULT_VOLUME: i32 = 80;
pub const MIN_VOLUME: i32 = 0;
pub const MAX_VOLUME: i32 = 100;

/// Metadata for the currently loaded media file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
    pub kind: &'static str,
    pub thumbnail: String,
}

/// Snapshot returned to the UI on every status poll.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: &'static str,
    pub time_ms: i64,
    pub length_ms: i64,
    pub volume: i32,
    pub has_video: bool,
    pub play_mode: String,
    pub media: Option<MediaInfo>,
}

/// Mutable runtime state protected by a mutex inside `AppState`.
pub struct InnerState {
    pub media: Option<MediaInfo>,
    pub folders: Vec<PathBuf>,
    pub active_folder: Option<String>,
    pub play_mode: PlayMode,
    pub queue: PlaybackQueue,
    /// Target volume applied once VLC opens an audio output.
    pub volume: i32,
    pub is_surface_visible: bool,
}

/// Root state managed by Tauri and shared across all IPC commands.
pub struct AppState {
    player: Result<Player, String>,
    surface: Option<VideoSurface>,
    inner: Mutex<InnerState>,
}

impl AppState {
    /// Construct application state from an initialized player, optional surface, and saved libraries.
    pub fn new(
        player: Result<Player, String>,
        surface: Option<VideoSurface>,
        folders: Vec<PathBuf>,
        active_folder: Option<String>,
    ) -> Self {
        let initial_paths = library::scan_libraries(&folders, active_folder.as_deref())
            .into_iter()
            .map(|item| item.path)
            .collect();

        Self {
            player,
            surface,
            inner: Mutex::new(InnerState {
                media: None,
                folders,
                active_folder,
                play_mode: PlayMode::default(),
                queue: PlaybackQueue::new(initial_paths),
                volume: DEFAULT_VOLUME,
                is_surface_visible: false,
            }),
        }
    }

    /// Borrow the underlying libVLC player or return the startup initialization error.
    pub fn player(&self) -> Result<&Player, String> {
        self.player.as_ref().map_err(Clone::clone)
    }

    /// Borrow the optional native video surface.
    pub fn surface(&self) -> Option<&VideoSurface> {
        self.surface.as_ref()
    }

    /// Acquire the mutable inner state lock.
    pub fn lock_inner(&self) -> MutexGuard<'_, InnerState> {
        self.inner.lock().unwrap()
    }

    /// Load `path` into libVLC, extract its thumbnail metadata, and start playback.
    pub fn load_and_play(&self, path: &Path) -> Result<MediaInfo, String> {
        let info = self.load_media(path)?;
        self.player()?.play()?;
        Ok(info)
    }

    /// Load `path` into libVLC without starting playback and update `inner.media`.
    pub fn load_media(&self, path: &Path) -> Result<MediaInfo, String> {
        self.player()?.load(path)?;
        let kind = library::media_kind(path).unwrap_or("audio");
        let thumbnail = library::extract_thumbnail(path, kind);
        let info = MediaInfo {
            path: path.to_string_lossy().into_owned(),
            name: library::display_name_for_path(path),
            kind,
            thumbnail,
        };
        self.lock_inner().media = Some(info.clone());
        Ok(info)
    }

    /// Scan and return all track paths inside the active library view.
    pub fn current_folder_paths(&self) -> Vec<String> {
        let (folders, active_folder) = {
            let inner = self.lock_inner();
            (inner.folders.clone(), inner.active_folder.clone())
        };
        library::scan_libraries(&folders, active_folder.as_deref())
            .into_iter()
            .map(|item| item.path)
            .collect()
    }

    /// Build the full `LibraryResponse` snapshot for the UI.
    pub fn library_response(&self) -> LibraryResponse {
        let (folders, active_folder) = {
            let inner = self.lock_inner();
            (inner.folders.clone(), inner.active_folder.clone())
        };
        let items = library::scan_libraries(&folders, active_folder.as_deref());
        let folder_entries = folders.iter().map(|p| library::folder_entry(p)).collect();
        LibraryResponse {
            folders: folder_entries,
            active_folder,
            items,
        }
    }

    /// Populate the queue from the active folder if it is currently empty.
    pub fn ensure_queue_populated(&self) {
        if self.lock_inner().queue.is_empty() {
            let folder_paths = self.current_folder_paths();
            self.lock_inner().queue.set_paths(folder_paths);
        }
    }

    /// Advance to the next queued track when VLC reports `Ended`, if allowed by `play_mode`.
    pub fn advance_if_ended(&self, player: &Player, current_state: VlcState) -> VlcState {
        if current_state != VlcState::Ended {
            return current_state;
        }
        let next_path = {
            let inner = self.lock_inner();
            let current = inner.media.as_ref().map(|m| m.path.as_str());
            inner.queue.next_after_end(current, inner.play_mode)
        };
        let Some(path_str) = next_path else {
            return current_state;
        };
        let next_file = PathBuf::from(&path_str);
        if next_file.is_file() && self.load_media(&next_file).is_ok() {
            let _ = player.play();
            return player.state();
        }
        current_state
    }
}
