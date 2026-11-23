//! Reactive UI state and action methods for the Dotara frontend.

use crate::api;
use crate::formatting::resolve_selected_paths;
use crate::models::{LibraryResponse, MediaInfo, Status};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashSet;

/// Holds the root reactive signals and provides typed async actions for UI components.
#[derive(Clone, Copy)]
pub struct AppController {
    pub status: ReadSignal<Status>,
    set_status: WriteSignal<Status>,
    pub error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    pub is_menu_open: ReadSignal<bool>,
    set_is_menu_open: WriteSignal<bool>,
    pub library: ReadSignal<LibraryResponse>,
    set_library: WriteSignal<LibraryResponse>,
    pub selected_path: ReadSignal<Option<String>>,
    set_selected_path: WriteSignal<Option<String>>,
    pub checked_paths: ReadSignal<HashSet<String>>,
    set_checked_paths: WriteSignal<HashSet<String>>,
}

impl AppController {
    /// Allocate all root signals with their initial defaults.
    pub fn new() -> Self {
        let (status, set_status) = signal(Status::default());
        let (error, set_error) = signal(None::<String>);
        let (is_menu_open, set_is_menu_open) = signal(true);
        let (library, set_library) = signal(LibraryResponse::default());
        let (selected_path, set_selected_path) = signal(None::<String>);
        let (checked_paths, set_checked_paths) = signal(HashSet::<String>::new());

        Self {
            status,
            set_status,
            error,
            set_error,
            is_menu_open,
            set_is_menu_open,
            library,
            set_library,
            selected_path,
            set_selected_path,
            checked_paths,
            set_checked_paths,
        }
    }

    /// Poll backend playback status once and update the `status` signal.
    pub fn refresh_status(self) {
        spawn_local(async move {
            match api::fetch_status().await {
                Ok(s) => self.set_status.set(s),
                Err(err) => self.set_error.set(Some(err)),
            }
        });
    }

    /// Load the initial media libraries on application startup.
    pub fn load_initial_library(self) {
        spawn_local(async move {
            match api::fetch_library().await {
                Ok(lib) => self.apply_library(lib),
                Err(err) => self.set_error.set(Some(err)),
            }
            self.refresh_status();
        });
    }

    /// Replace the library snapshot while keeping any still-valid selection and checkboxes.
    pub fn apply_library(self, lib: LibraryResponse) {
        let valid: HashSet<String> = lib.items.iter().map(|i| i.path.clone()).collect();
        self.set_checked_paths.update(|set| set.retain(|p| valid.contains(p)));
        let next_sel = self
            .selected_path
            .get_untracked()
            .filter(|sel| valid.contains(sel))
            .or_else(|| lib.items.first().map(|i| i.path.clone()));
        self.set_selected_path.set(next_sel);
        self.set_library.set(lib);
    }

    /// Play `path`, or toggle pause if `path` is already the active track.
    pub fn play_item(self, path: String) {
        self.set_selected_path.set(Some(path.clone()));
        let selection = self.current_selection();
        spawn_local(async move {
            let active_path = self.status.with_untracked(|s| {
                let is_active = s.state == "playing" || s.state == "paused";
                is_active
                    .then(|| s.media.as_ref().map(|m| m.path.clone()))
                    .flatten()
            });
            let result = if active_path.as_deref() == Some(path.as_str()) {
                api::toggle_pause().await
            } else {
                api::play_track(path, Some(selection)).await.map(|_| ())
            };
            self.finish_unit(result);
        });
    }

    /// Play the highlighted row if it differs from the loaded track; otherwise toggle pause.
    pub fn play_or_toggle(self) {
        let loaded = self
            .status
            .with_untracked(|s| s.media.as_ref().map(|m| m.path.clone()));
        if let Some(sel) = self.selected_path.get_untracked() {
            if loaded.as_deref() != Some(sel.as_str()) {
                self.play_item(sel);
                return;
            }
        }
        spawn_local(async move {
            self.finish_unit(api::toggle_pause().await);
        });
    }

    /// Start playing all items inside the active library folder.
    pub fn play_all(self) {
        spawn_local(async move {
            self.finish_media_opt(api::play_all_in_folder().await);
        });
    }

    /// Start playing the checked tracks (or single highlighted row).
    pub fn play_selected(self) {
        let paths = self.current_selection();
        if paths.is_empty() {
            return;
        }
        spawn_local(async move {
            self.finish_media_opt(api::play_selected_tracks(paths).await);
        });
    }

    /// Step backward (`is_forward = false`) or forward (`is_forward = true`) in the queue.
    pub fn step_track(self, is_forward: bool) {
        spawn_local(async move {
            let res = if is_forward {
                api::next_track().await
            } else {
                api::prev_track().await
            };
            self.finish_media_opt(res);
        });
    }

    /// Open a native single-file picker and play the chosen media file.
    pub fn open_file(self) {
        spawn_local(async move {
            self.finish_media_opt(api::open_media_file().await);
        });
    }

    /// Stop playback completely.
    pub fn stop(self) {
        spawn_local(async move {
            self.finish_unit(api::stop_playback().await);
        });
    }

    /// Seek to `ms` milliseconds from the start of the track.
    pub fn seek(self, ms: i64) {
        spawn_local(async move {
            let _ = api::seek_to(ms).await;
            self.refresh_status();
        });
    }

    /// Update audio volume optimistically and send it to the backend.
    pub fn set_volume(self, volume: i32) {
        self.set_status.update(|s| s.volume = volume);
        spawn_local(async move {
            let _ = api::set_volume(volume).await;
        });
    }

    /// Change the active playback mode.
    pub fn change_mode(self, mode: String) {
        self.set_status.update(|s| s.play_mode = mode.clone());
        spawn_local(async move {
            self.finish_unit(api::set_play_mode(mode).await);
        });
    }

    /// Open a native folder picker to add a new media library.
    pub fn add_library(self) {
        spawn_local(async move {
            match api::add_library_folder().await {
                Ok(Some(lib)) => {
                    self.apply_library(lib);
                    self.set_error.set(None);
                }
                Ok(None) => {}
                Err(err) => self.set_error.set(Some(err)),
            }
        });
    }

    /// Switch the active library tab to `folder` (or `"*"` for All).
    pub fn switch_library(self, folder: String) {
        spawn_local(async move {
            match api::select_library_folder(folder).await {
                Ok(lib) => {
                    self.apply_library(lib);
                    self.set_error.set(None);
                }
                Err(err) => self.set_error.set(Some(err)),
            }
        });
    }

    /// Remove `folder` from the user's saved libraries.
    pub fn remove_library(self, folder: String) {
        spawn_local(async move {
            match api::remove_library_folder(folder).await {
                Ok(lib) => {
                    self.apply_library(lib);
                    self.set_error.set(None);
                }
                Err(err) => self.set_error.set(Some(err)),
            }
        });
    }

    /// Highlight a single row in the sidebar.
    pub fn select_row(self, path: String) {
        self.set_selected_path.set(Some(path));
    }

    /// Toggle multi-select checkbox state for `path`.
    pub fn toggle_check(self, path: String) {
        self.set_selected_path.set(Some(path.clone()));
        self.set_checked_paths.update(|set| {
            if !set.remove(&path) {
                set.insert(path);
            }
        });
    }

    /// Toggle the docked library sidebar open or closed.
    pub fn toggle_menu(self) {
        self.set_is_menu_open.update(|open| *open = !*open);
    }

    fn current_selection(self) -> Vec<String> {
        self.library.with_untracked(|lib| {
            self.checked_paths.with_untracked(|checked| {
                self.selected_path.with_untracked(|sel| {
                    resolve_selected_paths(&lib.items, checked, sel.as_deref())
                })
            })
        })
    }

    fn finish_unit(self, result: Result<(), String>) {
        match result {
            Ok(()) => self.set_error.set(None),
            Err(err) => self.set_error.set(Some(err)),
        }
        self.refresh_status();
    }

    fn finish_media_opt(self, result: Result<Option<MediaInfo>, String>) {
        match result {
            Ok(Some(info)) => {
                self.set_selected_path.set(Some(info.path));
                self.set_error.set(None);
            }
            Ok(None) => {}
            Err(err) => self.set_error.set(Some(err)),
        }
        self.refresh_status();
    }
}
