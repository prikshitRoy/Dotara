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
    media: Option<MediaInfo>,
}

struct Inner {
    media: Option<MediaInfo>,
    folders: Vec<PathBuf>,
    active_folder: Option<String>,
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

/// Load a file path from the library and immediately start playing it.
#[tauri::command]
async fn play_file(state: State<'_, AppState>, path: String) -> Result<MediaInfo, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("File not found: {path}"));
    }
    let info = state.load(&p)?;
    state.player()?.play()?;
    Ok(info)
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

/// Polled by the UI a few times per second.
#[tauri::command]
async fn status(state: State<'_, AppState>) -> Result<Status, String> {
    let player = state.player()?;
    let st = player.state();
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
            app.manage(AppState {
                player,
                surface,
                inner: Mutex::new(Inner {
                    media: None,
                    folders,
                    active_folder,
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
            play_file,
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
