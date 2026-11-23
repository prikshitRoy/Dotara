//! Tauri commands for media transport, queue navigation, and playback modes.

use crate::library::MEDIA_EXTENSIONS;
use crate::playback::{PlayMode, PlaybackQueue};
use crate::state::{AppState, MediaInfo, MAX_VOLUME, MIN_VOLUME};
use crate::vlc::State as VlcState;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Update the playback progression mode (`continue_next`, `play_all`, `selected`, `one`).
#[tauri::command]
pub async fn set_play_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let parsed = PlayMode::parse(&mode)?;
    state.lock_inner().play_mode = parsed;
    Ok(())
}

/// Queue every track in the active library folder and play from the first track.
#[tauri::command]
pub async fn play_all(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    let all_paths = state.current_folder_paths();
    let Some(first) = all_paths.first().cloned() else {
        return Ok(None);
    };
    {
        let mut inner = state.lock_inner();
        inner.queue.set_paths(all_paths);
        inner.play_mode = inner.play_mode.resolved_for_play_all();
    }
    state.load_and_play(Path::new(&first)).map(Some)
}

/// Play the user's selected track(s), queueing either the selection or the full folder.
#[tauri::command]
pub async fn play_selected(
    state: State<'_, AppState>,
    selected_paths: Vec<String>,
) -> Result<Option<MediaInfo>, String> {
    let Some(first) = selected_paths.first().cloned() else {
        return Ok(None);
    };
    let folder_paths = state.current_folder_paths();
    {
        let mut inner = state.lock_inner();
        let queue_paths =
            PlaybackQueue::build_for_selected(selected_paths, folder_paths, inner.play_mode);
        inner.queue.set_paths(queue_paths);
    }
    state.load_and_play(Path::new(&first)).map(Some)
}

/// Load a specific media file from the library and start playing it immediately.
#[tauri::command]
pub async fn play_file(
    state: State<'_, AppState>,
    path: String,
    selected_paths: Option<Vec<String>>,
) -> Result<MediaInfo, String> {
    let file_path = PathBuf::from(&path);
    if !file_path.is_file() {
        return Err(format!("File not found: {path}"));
    }
    let folder_paths = state.current_folder_paths();
    {
        let mut inner = state.lock_inner();
        let queue_paths = PlaybackQueue::build_for_file(
            &path,
            selected_paths,
            folder_paths,
            inner.play_mode,
        );
        inner.queue.set_paths(queue_paths);
    }
    state.load_and_play(&file_path)
}

/// Advance manually to the next track in the playback queue.
#[tauri::command]
pub async fn next_track(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    state.ensure_queue_populated();
    let next_path = {
        let inner = state.lock_inner();
        let current = inner.media.as_ref().map(|m| m.path.as_str());
        inner.queue.next_manual(current, inner.play_mode)
    };
    match next_path {
        Some(path) => state.load_and_play(Path::new(&path)).map(Some),
        None => Ok(None),
    }
}

/// Step manually to the previous track in the playback queue.
#[tauri::command]
pub async fn prev_track(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    state.ensure_queue_populated();
    let prev_path = {
        let inner = state.lock_inner();
        let current = inner.media.as_ref().map(|m| m.path.as_str());
        inner.queue.prev_manual(current, inner.play_mode)
    };
    match prev_path {
        Some(path) => state.load_and_play(Path::new(&path)).map(Some),
        None => Ok(None),
    }
}

/// Open a native single-file picker, load the chosen media file, and start playing it.
#[tauri::command]
pub async fn open_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<MediaInfo>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Open media")
        .add_filter("Media files", MEDIA_EXTENSIONS)
        .add_filter("All files", &["*"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|e| format!("could not resolve picked file path: {e}"))?;
    state.load_and_play(&path).map(Some)
}

/// Start or resume playback of the currently loaded track.
#[tauri::command]
pub async fn play(state: State<'_, AppState>) -> Result<(), String> {
    state.player()?.play()
}

/// Pause playback of the currently loaded track.
#[tauri::command]
pub async fn pause(state: State<'_, AppState>) -> Result<(), String> {
    state.player()?.pause();
    Ok(())
}

/// Toggle between playing and paused states.
#[tauri::command]
pub async fn toggle_pause(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player()?;
    if player.state() == VlcState::Playing {
        player.pause();
        Ok(())
    } else {
        player.play()
    }
}

/// Stop playback completely.
#[tauri::command]
pub async fn stop(state: State<'_, AppState>) -> Result<(), String> {
    state.player()?.stop();
    Ok(())
}

/// Seek to `ms` milliseconds from the start of the track.
#[tauri::command]
pub async fn seek(state: State<'_, AppState>, ms: i64) -> Result<(), String> {
    state.player()?.set_time(ms.max(0));
    Ok(())
}

/// Set audio output volume clamped to `0..=100`.
#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, volume: i32) -> Result<(), String> {
    let clamped = volume.clamp(MIN_VOLUME, MAX_VOLUME);
    state.lock_inner().volume = clamped;
    state.player()?.set_volume(clamped);
    Ok(())
}
