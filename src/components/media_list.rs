//! Scrollable media list and individual track row with thumbnails and multi-select checkboxes.

use crate::models::{LibraryItem, LibraryResponse, Status, ALL_LIBRARIES_KEY};
use leptos::prelude::*;
use std::collections::HashSet;

/// Scrollable list of audio and video tracks for the active library view.
#[component]
pub fn MediaList(
    library: ReadSignal<LibraryResponse>,
    status: ReadSignal<Status>,
    selected_path: ReadSignal<Option<String>>,
    checked_paths: ReadSignal<HashSet<String>>,
    on_add_library: Callback<()>,
    on_select: Callback<String>,
    on_toggle_check: Callback<String>,
    on_play_item: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="media-list">
            {move || {
                let lib = library.get();
                if lib.folders.is_empty() {
                    return view! {
                        <div class="empty-library">
                            <div>"No media libraries added yet."</div>
                            <button class="primary" on:click=move |_| on_add_library.run(())>
                                "+ Add Media Library…"
                            </button>
                        </div>
                    }
                    .into_any();
                }
                if lib.items.is_empty() {
                    return view! {
                        <div class="empty-library">
                            <div>"No audio or video files found in this library."</div>
                            <button on:click=move |_| on_add_library.run(())>
                                "+ Add Another Library…"
                            </button>
                        </div>
                    }
                    .into_any();
                }

                let should_show_folder = lib.active_folder.as_deref() == Some(ALL_LIBRARIES_KEY);
                view! {
                    <div class="media-items">
                        {lib.items.into_iter().map(|item| {
                            view! {
                                <MediaRow
                                    item=item
                                    should_show_folder=should_show_folder
                                    status=status
                                    selected_path=selected_path
                                    checked_paths=checked_paths
                                    on_select=on_select
                                    on_toggle_check=on_toggle_check
                                    on_play_item=on_play_item
                                />
                            }
                        }).collect_view()}
                    </div>
                }
                .into_any()
            }}
        </div>
    }
}

#[component]
fn MediaRow(
    item: LibraryItem,
    should_show_folder: bool,
    status: ReadSignal<Status>,
    selected_path: ReadSignal<Option<String>>,
    checked_paths: ReadSignal<HashSet<String>>,
    on_select: Callback<String>,
    on_toggle_check: Callback<String>,
    on_play_item: Callback<String>,
) -> impl IntoView {
    let path_selected = item.path.clone();
    let path_playing = item.path.clone();
    let path_playing_btn = item.path.clone();
    let path_checked = item.path.clone();
    let path_click = item.path.clone();
    let path_dbl = item.path.clone();
    let path_toggle = item.path.clone();
    let path_play_btn = item.path.clone();

    let name_title = item.name.clone();
    let name_alt = item.name.clone();
    let is_video = item.kind == "video";

    view! {
        <div
            class="media-item"
            class:selected=move || {
                selected_path.with(|s| s.as_deref() == Some(path_selected.as_str()))
            }
            class:playing=move || {
                status.with(|s| s.media.as_ref().map(|m| m.path.as_str()) == Some(path_playing.as_str()))
            }
            on:click=move |_| on_select.run(path_click.clone())
            on:dblclick=move |_| on_play_item.run(path_dbl.clone())
        >
            <input
                type="checkbox"
                class="item-check"
                title="Select for playlist"
                prop:checked=move || checked_paths.with(|c| c.contains(&path_checked))
                on:click=move |ev| ev.stop_propagation()
                on:change=move |_| on_toggle_check.run(path_toggle.clone())
            />
            <div class="thumb-wrap">
                <img class="thumb" src=item.thumbnail alt=name_alt loading="lazy" />
                <span class="kind-badge" class:video=is_video>
                    {if is_video { "VIDEO" } else { "AUDIO" }}
                </span>
            </div>
            <div class="item-meta">
                <div class="item-name" title=name_title>{item.name}</div>
                {should_show_folder.then(|| view! {
                    <div class="item-folder">{item.folder}</div>
                })}
            </div>
            <button
                class="item-play-btn"
                title="Play / Pause"
                on:click=move |ev| {
                    ev.stop_propagation();
                    on_play_item.run(path_play_btn.clone());
                }
            >
                {move || {
                    let is_this_playing = status.with(|s| {
                        s.state == "playing"
                            && s.media.as_ref().map(|m| m.path.as_str())
                                == Some(path_playing_btn.as_str())
                    });
                    if is_this_playing { "⏸" } else { "▶" }
                }}
            </button>
        </div>
    }
}
