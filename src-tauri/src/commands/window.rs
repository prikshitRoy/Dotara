//! Tauri commands for synchronizing the native video surface and polling player status.

use crate::state::{window_title_for_media, AppState, Status};
use tauri::{State, WebviewWindow};

/// Move and resize the native video child window to match `.video-area` (in physical pixels).
#[tauri::command]
pub async fn set_video_bounds(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(surface) = state.surface() {
        surface.set_bounds(
            x.round() as i32,
            y.round() as i32,
            width.round() as i32,
            height.round() as i32,
        );
    }
    Ok(())
}

/// Poll player state, advance the queue on track end, and keep window title, surface, & volume in sync.
#[tauri::command]
pub async fn status(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Status, String> {
    let player = state.player()?;
    let current_state = state.advance_if_ended(player, player.state());
    let has_video = player.has_video();

    let mut inner = state.lock_inner();

    // Show the active media name in the top window title bar (empty when idle so only the logo shows).
    let desired_title = window_title_for_media(inner.media.as_ref());
    if inner.last_window_title != desired_title {
        if window.set_title(desired_title).is_ok() {
            inner.last_window_title = desired_title.to_string();
        }
    }

    // Show the native child window only while VLC has an active video output so audio art shows otherwise.
    if let Some(surface) = state.surface() {
        if inner.is_surface_visible != has_video {
            surface.set_visible(has_video);
            inner.is_surface_visible = has_video;
        }
    }

    // Apply the requested volume once VLC finishes creating its audio output device.
    let current_volume = player.volume();
    if current_volume >= 0 && current_volume != inner.volume {
        player.set_volume(inner.volume);
    }

    Ok(Status {
        state: current_state.as_str(),
        time_ms: player.time().max(0),
        length_ms: player.length().max(0),
        volume: inner.volume,
        has_video,
        play_mode: inner.play_mode.as_str().to_string(),
        media: inner.media.clone(),
    })
}
