//! Sidebar header: library folder chips, Play All / Play Selected actions, and play-mode selector.

use crate::formatting::active_library_label;
use crate::models::{
    LibraryResponse, Status, ALL_LIBRARIES_KEY, MODE_CONTINUE_NEXT, MODE_ONE, MODE_PLAY_ALL,
    MODE_SELECTED,
};
use leptos::prelude::*;
use std::collections::HashSet;

/// Top section of the docked library sidebar.
#[component]
pub fn LibraryHeader(
    library: ReadSignal<LibraryResponse>,
    status: ReadSignal<Status>,
    selected_path: ReadSignal<Option<String>>,
    checked_paths: ReadSignal<HashSet<String>>,
    on_add_library: Callback<()>,
    on_switch_library: Callback<String>,
    on_remove_library: Callback<String>,
    on_play_all: Callback<()>,
    on_play_selected: Callback<()>,
    on_change_mode: Callback<String>,
) -> impl IntoView {
    let has_items = move || library.with(|l| !l.items.is_empty());

    view! {
        <div class="side-header">
            <div class="side-title-row">
                <span class="side-title">"Media Libraries"</span>
                <div class="side-header-actions">
                    <button class="small-btn" on:click=move |_| on_add_library.run(())>
                        "+ Add Library…"
                    </button>
                </div>
            </div>

            {move || {
                let lib = library.get();
                if lib.folders.is_empty() {
                    return None;
                }
                let active = lib.active_folder.clone().unwrap_or_default();
                let has_multiple = lib.folders.len() > 1;
                let active_label = active_library_label(&active, lib.folders.len());
                let active_label_title = active_label.clone();

                Some(view! {
                    <div class="library-tabs">
                        {has_multiple.then(|| {
                            let is_all = active == ALL_LIBRARIES_KEY;
                            view! {
                                <div
                                    class="lib-chip"
                                    class:active=is_all
                                    on:click=move |_| on_switch_library.run(ALL_LIBRARIES_KEY.to_string())
                                    title="Show media from all libraries"
                                >
                                    <span class="lib-chip-name">"All"</span>
                                </div>
                            }
                        })}
                        {lib.folders.into_iter().map(|folder| {
                            let path_select = folder.path.clone();
                            let path_remove = folder.path.clone();
                            let path_title = folder.path.clone();
                            let is_active = active.eq_ignore_ascii_case(&folder.path);
                            view! {
                                <div
                                    class="lib-chip"
                                    class:active=is_active
                                    title=path_title
                                    on:click=move |_| on_switch_library.run(path_select.clone())
                                >
                                    <span class="lib-chip-name">{folder.name}</span>
                                    <button
                                        class="lib-chip-remove"
                                        title="Remove library"
                                        on:click=move |ev| {
                                            ev.stop_propagation();
                                            on_remove_library.run(path_remove.clone());
                                        }
                                    >
                                        "×"
                                    </button>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                    <div class="folder-path" title=active_label_title>{active_label}</div>
                })
            }}

            <Show when=has_items>
                <div class="play-controls-box">
                    <div class="play-actions-row">
                        <button
                            class="play-all-btn"
                            on:click=move |_| on_play_all.run(())
                            title="Play all songs inside this folder from the beginning"
                        >
                            "▶ Play All"
                        </button>
                        <button
                            class="play-selected-btn"
                            disabled=move || {
                                selected_path.with(Option::is_none)
                                    && checked_paths.with(HashSet::is_empty)
                            }
                            on:click=move |_| on_play_selected.run(())
                            title="Play the selected song(s)"
                        >
                            {move || {
                                let count = checked_paths.with(HashSet::len);
                                if count > 0 {
                                    format!("▶ Play Selected ({count})")
                                } else {
                                    "▶ Play Selected".to_string()
                                }
                            }}
                        </button>
                    </div>
                    <div class="play-mode-row">
                        <label class="mode-label">"Mode:"</label>
                        <select
                            class="mode-select"
                            prop:value=move || status.with(|s| s.play_mode.clone())
                            on:change=move |ev| on_change_mode.run(event_target_value(&ev))
                        >
                            <option
                                value=MODE_CONTINUE_NEXT
                                selected=move || status.with(|s| s.play_mode == MODE_CONTINUE_NEXT)
                            >
                                "Continue play next song in the list"
                            </option>
                            <option
                                value=MODE_PLAY_ALL
                                selected=move || status.with(|s| s.play_mode == MODE_PLAY_ALL)
                            >
                                "Play all"
                            </option>
                            <option
                                value=MODE_SELECTED
                                selected=move || status.with(|s| s.play_mode == MODE_SELECTED)
                            >
                                "Selected"
                            </option>
                            <option
                                value=MODE_ONE
                                selected=move || status.with(|s| s.play_mode == MODE_ONE)
                            >
                                "Play only one song"
                            </option>
                        </select>
                    </div>
                </div>
            </Show>
        </div>
    }
}
