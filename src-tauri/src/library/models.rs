//! Data structures, file extension lists, and pure path helpers for media libraries.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported video file extensions (lowercase, without leading dot).
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", "ts", "mpg", "mpeg",
];

/// Supported audio file extensions (lowercase, without leading dot).
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "opus", "m4a", "aac", "wma", "alac",
];

/// Combined list used by the native single-file open dialog.
pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", "ts", "mpg", "mpeg", "mp3", "flac",
    "wav", "ogg", "opus", "m4a", "aac", "wma", "alac",
];

/// Default folder added automatically on first launch when present on disk.
pub const DEFAULT_MEDIA_FOLDER: &str = r"E:\Songs";

/// Sentinel key representing the combined view across all configured libraries.
pub const ALL_LIBRARIES_KEY: &str = "*";

/// A single configured library folder shown as a selectable chip in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub path: String,
    pub name: String,
}

/// A single audio or video file discovered inside a library folder.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub path: String,
    pub name: String,
    pub folder: String,
    /// `"video"` or `"audio"`.
    pub kind: &'static str,
    /// Data URL (`data:image/...;base64,...`).
    pub thumbnail: String,
}

/// Snapshot of all configured libraries, the active view, and its media items.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryResponse {
    pub folders: Vec<LibraryFolder>,
    /// Active library path, `"*"` for all libraries combined, or `None` when empty.
    pub active_folder: Option<String>,
    pub items: Vec<LibraryItem>,
}

/// Build a `LibraryFolder` entry using the folder basename as the display label.
pub fn folder_entry(path: &Path) -> LibraryFolder {
    let full = path.to_string_lossy().into_owned();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| full.clone());
    LibraryFolder { path: full, name }
}

/// Classify a file path as `"video"`, `"audio"`, or `None` for unsupported extensions.
pub fn media_kind(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some("video")
    } else if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some("audio")
    } else {
        None
    }
}

/// Extract a clean display title from a media file path (stem, falling back to filename).
pub fn display_name_for_path(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_video_and_audio_extensions_case_insensitively() {
        assert_eq!(media_kind(Path::new("clip.WEBM")), Some("video"));
        assert_eq!(media_kind(Path::new("movie.mp4")), Some("video"));
        assert_eq!(media_kind(Path::new("song.FLAC")), Some("audio"));
        assert_eq!(media_kind(Path::new("track.mp3")), Some("audio"));
        assert_eq!(media_kind(Path::new("cover.jpg")), None);
        assert_eq!(media_kind(Path::new("README")), None);
    }

    #[test]
    fn builds_folder_entry_using_leaf_name_or_full_path_for_roots() {
        let entry = folder_entry(Path::new(r"E:\Songs"));
        assert_eq!(entry.name, "Songs");
        assert_eq!(entry.path, r"E:\Songs");
    }

    #[test]
    fn extracts_display_name_without_extension() {
        assert_eq!(
            display_name_for_path(Path::new(r"E:\Songs\My Song.mp3")),
            "My Song"
        );
    }
}
