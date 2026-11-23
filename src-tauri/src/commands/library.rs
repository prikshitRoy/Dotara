//! Tauri commands for managing media library folders.

use crate::library::{self, LibraryResponse, ALL_LIBRARIES_KEY};
use crate::state::AppState;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Return all saved library folders, the active selection, and its scanned media items.
#[tauri::command]
pub async fn get_library(state: State<'_, AppState>) -> Result<LibraryResponse, String> {
    Ok(state.library_response())
}

/// Open a native folder picker, add the chosen folder to the user's libraries, and make it active.
#[tauri::command]
pub async fn add_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<LibraryResponse>, String> {
    let mut dialog = app.dialog().file().set_title("Add Media Library Folder");
    if let Some(dir) = picker_start_directory(&state) {
        dialog = dialog.set_directory(dir);
    }

    let Some(picked) = dialog.blocking_pick_folder() else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|e| format!("could not resolve picked folder path: {e}"))?;
    let path_str = path.to_string_lossy().into_owned();

    {
        let mut inner = state.lock_inner();
        let is_duplicate = inner
            .folders
            .iter()
            .any(|existing| existing.to_string_lossy().eq_ignore_ascii_case(&path_str));
        if !is_duplicate {
            inner.folders.push(path);
        }
        inner.active_folder = Some(path_str);
        library::save_libraries(&inner.folders, inner.active_folder.as_deref());
    }

    Ok(Some(state.library_response()))
}

/// Switch the active library folder (or `"*"` for all libraries combined).
#[tauri::command]
pub async fn select_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<LibraryResponse, String> {
    {
        let mut inner = state.lock_inner();
        inner.active_folder = Some(folder);
        library::save_libraries(&inner.folders, inner.active_folder.as_deref());
    }
    Ok(state.library_response())
}

/// Remove a library folder from the user's list and adjust the active selection if needed.
#[tauri::command]
pub async fn remove_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<LibraryResponse, String> {
    {
        let mut inner = state.lock_inner();
        inner
            .folders
            .retain(|p| !p.to_string_lossy().eq_ignore_ascii_case(&folder));

        let is_active_invalid = match inner.active_folder.as_deref() {
            None => true,
            Some(ALL_LIBRARIES_KEY) => inner.folders.is_empty(),
            Some(active) => !inner
                .folders
                .iter()
                .any(|p| p.to_string_lossy().eq_ignore_ascii_case(active)),
        };
        if is_active_invalid {
            inner.active_folder = inner
                .folders
                .first()
                .map(|p| p.to_string_lossy().into_owned());
        }
        library::save_libraries(&inner.folders, inner.active_folder.as_deref());
    }
    Ok(state.library_response())
}

fn picker_start_directory(state: &AppState) -> Option<PathBuf> {
    let inner = state.lock_inner();
    let candidate = inner
        .active_folder
        .as_deref()
        .filter(|active| *active != ALL_LIBRARIES_KEY)
        .map(PathBuf::from)
        .or_else(|| inner.folders.last().cloned())?;
    candidate.is_dir().then_some(candidate)
}
