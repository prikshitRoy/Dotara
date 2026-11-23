//! Dotara backend library: libVLC playback engine, media library, native video surface, and Tauri commands.

pub mod commands;
pub mod library;
pub mod playback;
pub mod state;
pub mod surface;
pub mod vlc;

use commands::{library as lib_cmd, playback as play_cmd, window as win_cmd};
use state::AppState;
use surface::VideoSurface;
use tauri::Manager;
use vlc::Player;

/// Label of the main Tauri window configured in `tauri.conf.json`.
const MAIN_WINDOW_LABEL: &str = "main";

/// Initialize VLC, create the native video surface, wire Tauri commands, and start the app.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let player = Player::new();
            if let Err(err) = &player {
                eprintln!("[dotara] VLC init failed: {err}");
            }

            let surface = create_video_surface(app, &player);
            let (folders, active_folder) = library::initial_libraries();
            app.manage(AppState::new(player, surface, folders, active_folder));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            lib_cmd::get_library,
            lib_cmd::add_folder,
            lib_cmd::select_folder,
            lib_cmd::remove_folder,
            play_cmd::set_play_mode,
            play_cmd::play_all,
            play_cmd::play_selected,
            play_cmd::play_file,
            play_cmd::next_track,
            play_cmd::prev_track,
            play_cmd::open_file,
            play_cmd::play,
            play_cmd::pause,
            play_cmd::toggle_pause,
            play_cmd::stop,
            play_cmd::seek,
            play_cmd::set_volume,
            win_cmd::set_video_bounds,
            win_cmd::status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dotara");
}

#[cfg(windows)]
fn create_video_surface(app: &tauri::App, player: &Result<Player, String>) -> Option<VideoSurface> {
    let window = app.get_webview_window(MAIN_WINDOW_LABEL)?;
    let parent_hwnd = window.hwnd().ok()?.0 as *mut std::ffi::c_void;
    match VideoSurface::create(parent_hwnd) {
        Ok(surface) => {
            if let Ok(vlc_player) = player {
                vlc_player.set_hwnd(surface.raw());
            }
            Some(surface)
        }
        Err(err) => {
            eprintln!("[dotara] {err}");
            None
        }
    }
}

#[cfg(not(windows))]
fn create_video_surface(
    _app: &tauri::App,
    _player: &Result<Player, String>,
) -> Option<VideoSurface> {
    None
}
