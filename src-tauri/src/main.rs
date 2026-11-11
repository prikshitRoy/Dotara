// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod library;
mod surface;
mod vlc;

use library::{LibraryResponse, AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use surface::VideoSurface;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use vlc::Player;

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", "ts", "mpg", "mpeg", // video
    "mp3", "flac", "wav", "ogg", "opus", "m4a", "aac", "wma", "alac", // audio
];

pub const MODE_CONTINUE_NEXT: &str = "continue_next";
pub const MODE_PLAY_ALL: &str = "play_all";
pub const MODE_SELECTED: &str = "selected";
pub const MODE_ONE: &str = "one";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaInfo {
    path: String,
    name: String,
    kind: &'static str,
    thumbnail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    state: &'static str,
    time_ms: i64,
    length_ms: i64,
    volume: i32,
    has_video: bool,
    play_mode: String,
    media: Option<MediaInfo>,
}

struct Inner {
    media: Option<MediaInfo>,
    folders: Vec<PathBuf>,
    active_folder: Option<String>,
    /// `"continue_next"` (default), `"play_all"`, `"selected"`, or `"one"`.
    play_mode: String,
    /// Ordered playback queue of file paths.
    queue: Vec<String>,
    /// Volume the user asked for; applied once VLC has an audio output.
    volume: i32,
    surface_visible: bool,
}

struct AppState {
    player: Result<Player, String>,
    surface: Option<VideoSurface>,
    inner: Mutex<Inner>,
}

impl AppState {
    fn player(&self) -> Result<&Player, String> {
        self.player.as_ref().map_err(Clone::clone)
    }

    fn load(&self, path: &Path) -> Result<MediaInfo, String> {
        self.player()?.load(path)?;
        let kind = library::media_kind(path).unwrap_or("audio");
        let thumbnail = library::extract_thumbnail(path, kind);
        let info = MediaInfo {
            path: path.to_string_lossy().into_owned(),
            name: path
                .file_stem()
                .or_else(|| path.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            kind,
            thumbnail,
        };
        self.inner.lock().unwrap().media = Some(info.clone());
        Ok(info)
    }

    fn current_folder_paths(&self) -> Vec<String> {
        let (folders, active_folder) = {
            let inner = self.inner.lock().unwrap();
            (inner.folders.clone(), inner.active_folder.clone())
        };
        library::scan_libraries(&folders, active_folder.as_deref())
            .into_iter()
            .map(|i| i.path)
            .collect()
    }

    fn library_response(&self) -> LibraryResponse {
        let (folders, active_folder) = {
            let inner = self.inner.lock().unwrap();
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
}

// ---------------------------------------------------------------------------
// Commands (all async so they never block the UI thread)
// ---------------------------------------------------------------------------

/// Return all saved media library folders, the active selection, and its media items.
#[tauri::command]
async fn get_library(state: State<'_, AppState>) -> Result<LibraryResponse, String> {
    Ok(state.library_response())
}

/// Let the user pick a media folder to add as a library, make it active, and scan it.
#[tauri::command]
async fn add_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<LibraryResponse>, String> {
    let mut dialog = app.dialog().file().set_title("Add Media Library Folder");
    let start_dir = {
        let inner = state.inner.lock().unwrap();
        inner
            .active_folder
            .as_deref()
            .filter(|a| *a != library::ALL_LIBRARIES_KEY)
            .map(PathBuf::from)
            .or_else(|| inner.folders.last().cloned())
    };
    if let Some(dir) = start_dir {
        if dir.is_dir() {
            dialog = dialog.set_directory(dir);
        }
    }
    let Some(picked) = dialog.blocking_pick_folder() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().into_owned();
    {
        let mut inner = state.inner.lock().unwrap();
        if !inner
            .folders
            .iter()
            .any(|p| p.to_string_lossy().eq_ignore_ascii_case(&path_str))
        {
            inner.folders.push(path);
        }
        inner.active_folder = Some(path_str);
        library::save_libraries(&inner.folders, inner.active_folder.as_deref());
    }
    Ok(Some(state.library_response()))
}

/// Switch the active media library folder (or `"*"` to view all libraries combined).
#[tauri::command]
async fn select_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<LibraryResponse, String> {
    {
        let mut inner = state.inner.lock().unwrap();
        inner.active_folder = Some(folder);
        library::save_libraries(&inner.folders, inner.active_folder.as_deref());
    }
    Ok(state.library_response())
}

/// Remove a media library folder from the user's libraries list.
#[tauri::command]
async fn remove_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<LibraryResponse, String> {
    {
        let mut inner = state.inner.lock().unwrap();
        inner
            .folders
            .retain(|p| !p.to_string_lossy().eq_ignore_ascii_case(&folder));
        let active_invalid = match inner.active_folder.as_deref() {
            None => true,
            Some(library::ALL_LIBRARIES_KEY) => inner.folders.is_empty(),
            Some(active) => !inner
                .folders
                .iter()
                .any(|p| p.to_string_lossy().eq_ignore_ascii_case(active)),
        };
        if active_invalid {
            inner.active_folder = inner
                .folders
                .first()
                .map(|p| p.to_string_lossy().into_owned());
        }
        library::save_libraries(&inner.folders, inner.active_folder.as_deref());
    }
    Ok(state.library_response())
}

/// Change the playback mode (`"continue_next"`, `"play_all"`, `"selected"`, `"one"`).
#[tauri::command]
async fn set_play_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let valid = matches!(
        mode.as_str(),
        MODE_CONTINUE_NEXT | MODE_PLAY_ALL | MODE_SELECTED | MODE_ONE
    );
    if !valid {
        return Err(format!("Invalid play mode: {mode}"));
    }
    state.inner.lock().unwrap().play_mode = mode;
    Ok(())
}

/// Play all songs inside the active library folder starting from the first track.
#[tauri::command]
async fn play_all(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    let all_paths = state.current_folder_paths();
    let Some(first) = all_paths.first().cloned() else {
        return Ok(None);
    };
    {
        let mut inner = state.inner.lock().unwrap();
        inner.queue = all_paths;
        // If mode was "one" or "selected", switch to "continue_next" so Play All plays the list.
        if inner.play_mode == MODE_ONE || inner.play_mode == MODE_SELECTED {
            inner.play_mode = MODE_CONTINUE_NEXT.to_string();
        }
    }
    let info = state.load(Path::new(&first))?;
    state.player()?.play()?;
    Ok(Some(info))
}

/// Play the selected song(s) according to the active playback mode.
#[tauri::command]
async fn play_selected(
    state: State<'_, AppState>,
    selected_paths: Vec<String>,
) -> Result<Option<MediaInfo>, String> {
    let Some(first) = selected_paths.first().cloned() else {
        return Ok(None);
    };
    let folder_paths = state.current_folder_paths();
    {
        let mut inner = state.inner.lock().unwrap();
        if selected_paths.len() > 1 || inner.play_mode == MODE_SELECTED {
            inner.queue = selected_paths;
        } else {
            inner.queue = folder_paths;
        }
    }
    let info = state.load(Path::new(&first))?;
    state.player()?.play()?;
    Ok(Some(info))
}

/// Load a file path from the library and immediately start playing it.
#[tauri::command]
async fn play_file(
    state: State<'_, AppState>,
    path: String,
    selected_paths: Option<Vec<String>>,
) -> Result<MediaInfo, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("File not found: {path}"));
    }
    let folder_paths = state.current_folder_paths();
    {
        let mut inner = state.inner.lock().unwrap();
        if inner.play_mode == MODE_SELECTED {
            if let Some(sel) = selected_paths.filter(|s| !s.is_empty()) {
                if sel.iter().any(|s| s == &path) {
                    inner.queue = sel;
                } else {
                    inner.queue = vec![path.clone()];
                }
            } else {
                inner.queue = vec![path.clone()];
            }
        } else if folder_paths.iter().any(|fp| fp == &path) {
            inner.queue = folder_paths;
        } else {
            inner.queue = vec![path.clone()];
        }
    }
    let info = state.load(&p)?;
    state.player()?.play()?;
    Ok(info)
}

/// Advance to the next song in the current queue (or folder list).
#[tauri::command]
async fn next_track(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    let next = {
        let mut inner = state.inner.lock().unwrap();
        if inner.queue.is_empty() {
            drop(inner);
            let folder_paths = state.current_folder_paths();
            inner = state.inner.lock().unwrap();
            inner.queue = folder_paths;
        }
        let current = inner.media.as_ref().map(|m| m.path.as_str());
        match current.and_then(|c| inner.queue.iter().position(|p| p == c)) {
            Some(idx) if idx + 1 < inner.queue.len() => Some(inner.queue[idx + 1].clone()),
            Some(_) if inner.play_mode == MODE_PLAY_ALL && !inner.queue.is_empty() => {
                Some(inner.queue[0].clone())
            }
            None => inner.queue.first().cloned(),
            _ => None,
        }
    };
    if let Some(path) = next {
        let info = state.load(Path::new(&path))?;
        state.player()?.play()?;
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

/// Go back to the previous song in the current queue (or folder list).
#[tauri::command]
async fn prev_track(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    let prev = {
        let mut inner = state.inner.lock().unwrap();
        if inner.queue.is_empty() {
            drop(inner);
            let folder_paths = state.current_folder_paths();
            inner = state.inner.lock().unwrap();
            inner.queue = folder_paths;
        }
        let current = inner.media.as_ref().map(|m| m.path.as_str());
        match current.and_then(|c| inner.queue.iter().position(|p| p == c)) {
            Some(idx) if idx > 0 => Some(inner.queue[idx - 1].clone()),
            Some(0) if inner.play_mode == MODE_PLAY_ALL && !inner.queue.is_empty() => {
                inner.queue.last().cloned()
            }
            _ => None,
        }
    };
    if let Some(path) = prev {
        let info = state.load(Path::new(&path))?;
        state.player()?.play()?;
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

/// Show a native file picker, load the chosen file and start playing it.
#[tauri::command]
async fn open_file(app: AppHandle, state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    let _ = (VIDEO_EXTENSIONS, AUDIO_EXTENSIONS);
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
    let path = file.into_path().map_err(|e| e.to_string())?;
    let info = state.load(&path)?;
    state.player()?.play()?;
    Ok(Some(info))
}

#[tauri::command]
async fn play(state: State<'_, AppState>) -> Result<(), String> {
    state.player()?.play()
}

#[tauri::command]
async fn pause(state: State<'_, AppState>) -> Result<(), String> {
    state.player()?.pause();
    Ok(())
}

#[tauri::command]
async fn toggle_pause(state: State<'_, AppState>) -> Result<(), String> {
    let player = state.player()?;
    if player.state() == vlc::State::Playing {
        player.pause();
        Ok(())
    } else {
        player.play()
    }
}

#[tauri::command]
async fn stop(state: State<'_, AppState>) -> Result<(), String> {
    state.player()?.stop();
    Ok(())
}

#[tauri::command]
async fn seek(state: State<'_, AppState>, ms: i64) -> Result<(), String> {
    state.player()?.set_time(ms.max(0));
    Ok(())
}

#[tauri::command]
async fn set_volume(state: State<'_, AppState>, volume: i32) -> Result<(), String> {
    let volume = volume.clamp(0, 100);
    state.inner.lock().unwrap().volume = volume;
    state.player()?.set_volume(volume);
    Ok(())
}

/// Bounds of the `.video-area` element, in physical pixels.
#[tauri::command]
async fn set_video_bounds(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(surface) = &state.surface {
        surface.set_bounds(
            x.round() as i32,
            y.round() as i32,
            width.round() as i32,
            height.round() as i32,
        );
    }
    Ok(())
}

/// Polled by the UI a few times per second; also handles auto-advancing when a track ends.
#[tauri::command]
async fn status(state: State<'_, AppState>) -> Result<Status, String> {
    let player = state.player()?;
    let mut st = player.state();

    if st == vlc::State::Ended {
        let next_to_play = {
            let inner = state.inner.lock().unwrap();
            let current = inner.media.as_ref().map(|m| m.path.as_str());
            match inner.play_mode.as_str() {
                MODE_ONE => None,
                MODE_PLAY_ALL => {
                    match current.and_then(|c| inner.queue.iter().position(|p| p == c)) {
                        Some(idx) if idx + 1 < inner.queue.len() => {
                            Some(inner.queue[idx + 1].clone())
                        }
                        Some(_) if !inner.queue.is_empty() => Some(inner.queue[0].clone()),
                        _ => None,
                    }
                }
                // MODE_CONTINUE_NEXT (default) and MODE_SELECTED advance to idx + 1 in queue
                _ => match current.and_then(|c| inner.queue.iter().position(|p| p == c)) {
                    Some(idx) if idx + 1 < inner.queue.len() => Some(inner.queue[idx + 1].clone()),
                    _ => None,
                },
            }
        };

        if let Some(next_path) = next_to_play {
            let p = PathBuf::from(&next_path);
            if p.is_file() && state.load(&p).is_ok() {
                let _ = player.play();
                st = player.state();
            }
        }
    }

    let has_video = player.has_video();
    let mut inner = state.inner.lock().unwrap();

    // Only show the native surface while VLC actually has a video output, so the
    // HTML placeholder (e.g. for audio files) is visible otherwise.
    if let Some(surface) = &state.surface {
        if inner.surface_visible != has_video {
            surface.set_visible(has_video);
            inner.surface_visible = has_video;
        }
    }

    // Apply the requested volume once an audio output exists.
    let current_volume = player.volume();
    if current_volume >= 0 && current_volume != inner.volume {
        player.set_volume(inner.volume);
    }

    Ok(Status {
        state: st.as_str(),
        time_ms: player.time().max(0),
        length_ms: player.length().max(0),
        volume: inner.volume,
        has_video,
        play_mode: inner.play_mode.clone(),
        media: inner.media.clone(),
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let player = Player::new();
            if let Err(e) = &player {
                eprintln!("[dotara] VLC init failed: {e}");
            }

            // setup() runs on the main thread, which is required for creating the child window.
            let mut surface = None;
            #[cfg(windows)]
            {
                let window = app.get_webview_window("main").expect("main window missing");
                let parent = window.hwnd()?.0 as *mut std::ffi::c_void;
                match VideoSurface::create(parent) {
                    Ok(s) => {
                        if let Ok(p) = &player {
                            p.set_hwnd(s.raw());
                        }
                        surface = Some(s);
                    }
                    Err(e) => eprintln!("[dotara] {e}"),
                }
            }

            let (folders, active_folder) = library::initial_libraries();
            let initial_queue = library::scan_libraries(&folders, active_folder.as_deref())
                .into_iter()
                .map(|i| i.path)
                .collect();

            app.manage(AppState {
                player,
                surface,
                inner: Mutex::new(Inner {
                    media: None,
                    folders,
                    active_folder,
                    play_mode: MODE_CONTINUE_NEXT.to_string(),
                    queue: initial_queue,
                    volume: 80,
                    surface_visible: false,
                }),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_library,
            add_folder,
            select_folder,
            remove_folder,
            set_play_mode,
            play_all,
            play_selected,
            play_file,
            next_track,
            prev_track,
            open_file,
            play,
            pause,
            toggle_pause,
            stop,
            seek,
            set_volume,
            set_video_bounds,
            status
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dotara");
}
