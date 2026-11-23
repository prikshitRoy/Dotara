//! Typed frontend client for all Dotara Tauri backend commands.

use crate::models::{
    BoundsArgs, FolderArgs, LibraryResponse, MediaInfo, ModeArgs, PlayFileArgs, PlaySelectedArgs,
    SeekArgs, Status, VolumeArgs,
};
use crate::tauri::{invoke, invoke_unit, NoArgs};

/// Fetch the current player status snapshot.
pub async fn fetch_status() -> Result<Status, String> {
    invoke("status", &NoArgs {}).await
}

/// Fetch all configured library folders and the active view's media items.
pub async fn fetch_library() -> Result<LibraryResponse, String> {
    invoke("get_library", &NoArgs {}).await
}

/// Open a native folder picker to add a new media library folder.
pub async fn add_library_folder() -> Result<Option<LibraryResponse>, String> {
    invoke("add_folder", &NoArgs {}).await
}

/// Switch the active library view to `folder` (or `"*"` for all libraries).
pub async fn select_library_folder(folder: String) -> Result<LibraryResponse, String> {
    invoke("select_folder", &FolderArgs { folder }).await
}

/// Remove `folder` from the user's configured libraries.
pub async fn remove_library_folder(folder: String) -> Result<LibraryResponse, String> {
    invoke("remove_folder", &FolderArgs { folder }).await
}

/// Start playing all items in the active folder from the first track.
pub async fn play_all_in_folder() -> Result<Option<MediaInfo>, String> {
    invoke("play_all", &NoArgs {}).await
}

/// Play the given selected track paths according to the active play mode.
pub async fn play_selected_tracks(selected_paths: Vec<String>) -> Result<Option<MediaInfo>, String> {
    invoke("play_selected", &PlaySelectedArgs { selected_paths }).await
}

/// Load and immediately play a specific library file.
pub async fn play_track(
    path: String,
    selected_paths: Option<Vec<String>>,
) -> Result<MediaInfo, String> {
    invoke(
        "play_file",
        &PlayFileArgs {
            path,
            selected_paths,
        },
    )
    .await
}

/// Toggle between playing and paused states.
pub async fn toggle_pause() -> Result<(), String> {
    invoke_unit("toggle_pause", &NoArgs {}).await
}

/// Stop media playback.
pub async fn stop_playback() -> Result<(), String> {
    invoke_unit("stop", &NoArgs {}).await
}

/// Advance to the next track in the playback queue.
pub async fn next_track() -> Result<Option<MediaInfo>, String> {
    invoke("next_track", &NoArgs {}).await
}

/// Go back to the previous track in the playback queue.
pub async fn prev_track() -> Result<Option<MediaInfo>, String> {
    invoke("prev_track", &NoArgs {}).await
}

/// Open the native single-file picker and play the chosen media file.
pub async fn open_media_file() -> Result<Option<MediaInfo>, String> {
    invoke("open_file", &NoArgs {}).await
}

/// Seek to `ms` milliseconds from the start of the active track.
pub async fn seek_to(ms: i64) -> Result<(), String> {
    invoke_unit("seek", &SeekArgs { ms }).await
}

/// Set audio output volume (`0..=100`).
pub async fn set_volume(volume: i32) -> Result<(), String> {
    invoke_unit("set_volume", &VolumeArgs { volume }).await
}

/// Update the active playback progression mode.
pub async fn set_play_mode(mode: String) -> Result<(), String> {
    invoke_unit("set_play_mode", &ModeArgs { mode }).await
}

/// Report the `.video-area` element's physical-pixel bounds to the native video surface.
pub async fn set_video_bounds(bounds: BoundsArgs) -> Result<(), String> {
    invoke_unit("set_video_bounds", &bounds).await
}
