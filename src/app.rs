use crate::tauri::{invoke, invoke_unit, NoArgs};
use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

const ALL_LIBRARIES_KEY: &str = "*";
const MODE_CONTINUE_NEXT: &str = "continue_next";
const MODE_PLAY_ALL: &str = "play_all";
const MODE_SELECTED: &str = "selected";
const MODE_ONE: &str = "one";

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub thumbnail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub path: String,
    pub name: String,
    pub folder: String,
    pub kind: String,
    pub thumbnail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryResponse {
    pub folders: Vec<LibraryFolder>,
    pub active_folder: Option<String>,
    pub items: Vec<LibraryItem>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: String,
    pub time_ms: i64,
    pub length_ms: i64,
    pub volume: i32,
    pub has_video: bool,
    pub play_mode: String,
    pub media: Option<MediaInfo>,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            state: String::new(),
            time_ms: 0,
            length_ms: 0,
            volume: 80,
            has_video: false,
            play_mode: MODE_CONTINUE_NEXT.to_string(),
            media: None,
        }
    }
}

#[derive(Serialize)]
struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Serialize)]
struct SeekArgs {
    ms: i64,
}

#[derive(Serialize)]
struct VolumeArgs {
    volume: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayFileArgs {
    path: String,
    selected_paths: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaySelectedArgs {
    selected_paths: Vec<String>,
}

#[derive(Serialize)]
struct ModeArgs {
    mode: String,
}

#[derive(Serialize)]
struct FolderArgs {
    folder: String,
}

/// What to show in the video area when the native VLC surface is hidden.
#[derive(Clone, PartialEq)]
enum Placeholder {
    Empty,
    Audio { name: String, thumbnail: String },
    Ready(String),
}

fn placeholder_for(s: &Status) -> Placeholder {
    match &s.media {
        None => Placeholder::Empty,
        Some(m)
            if !s.has_video && matches!(s.state.as_str(), "playing" | "paused" | "buffering") =>
        {
            Placeholder::Audio {
                name: m.name.clone(),
                thumbnail: m.thumbnail.clone(),
            }
        }
        Some(m) => Placeholder::Ready(m.name.clone()),
    }
}

fn fmt_time(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (status, set_status) = signal(Status::default());
    let (error, set_error) = signal(None::<String>);
    // Position (ms) while the user is dragging the seek bar.
    let (scrub, set_scrub) = signal(None::<i64>);
    // Side menu open/closed state (docked panel, not a popup).
    let (menu_open, set_menu_open) = signal(true);
    let (library, set_library) = signal(LibraryResponse::default());
    let (selected_path, set_selected_path) = signal(None::<String>);
    // Optional multi-selection set (checkboxes) for "Selected" playback mode.
    let (checked_paths, set_checked_paths) = signal(HashSet::<String>::new());

    let video_ref = NodeRef::<leptos::html::Div>::new();

    let apply_library = move |lib: LibraryResponse| {
        let valid_paths: HashSet<String> = lib.items.iter().map(|i| i.path.clone()).collect();
        set_checked_paths.update(|set| set.retain(|p| valid_paths.contains(p)));
        let keep_current = selected_path
            .get_untracked()
            .filter(|sel| valid_paths.contains(sel));
        let next_sel = keep_current.or_else(|| lib.items.first().map(|i| i.path.clone()));
        set_selected_path.set(next_sel);
        set_library.set(lib);
    };

    // Ordered list of checked paths (following folder order), or the single `selected_path`.
    let resolve_selected_list = move || -> Vec<String> {
        let checked = checked_paths.get_untracked();
        if !checked.is_empty() {
            let ordered: Vec<String> = library
                .with_untracked(|l| {
                    l.items
                        .iter()
                        .filter(|i| checked.contains(&i.path))
                        .map(|i| i.path.clone())
                        .collect()
                });
            if !ordered.is_empty() {
                return ordered;
            }
        }
        selected_path.get_untracked().into_iter().collect()
    };

    // --- status polling -----------------------------------------------------
    let refresh = move || {
        spawn_local(async move {
            match invoke::<_, Status>("status", &NoArgs {}).await {
                Ok(s) => set_status.set(s),
                Err(e) => set_error.set(Some(e)),
            }
        })
    };
    if let Ok(handle) = set_interval_with_handle(refresh, Duration::from_millis(250)) {
        on_cleanup(move || handle.clear());
    }

    // --- keep the native video surface glued to the .video-area element ---
    let report_bounds = move || {
        if let Some(el) = video_ref.get_untracked() {
            let r = el.get_bounding_client_rect();
            let dpr = window().device_pixel_ratio();
            let bounds = Bounds {
                x: r.left() * dpr,
                y: r.top() * dpr,
                width: r.width() * dpr,
                height: r.height() * dpr,
            };
            spawn_local(async move {
                let _ = invoke_unit("set_video_bounds", &bounds).await;
            });
        }
    };
    Effect::new(move |_| {
        // Re-measure whenever the video ref mounts or the side menu opens/closes.
        let _ = menu_open.get();
        if video_ref.get().is_some() {
            report_bounds();
            set_timeout(report_bounds, Duration::from_millis(30));
        }
    });
    let resize = window_event_listener(ev::resize, move |_| report_bounds());
    on_cleanup(move || resize.remove());

    // Load initial libraries (defaults to E:\Songs if present).
    spawn_local(async move {
        match invoke::<_, LibraryResponse>("get_library", &NoArgs {}).await {
            Ok(lib) => apply_library(lib),
            Err(e) => set_error.set(Some(e)),
        }
        refresh();
    });

    // --- actions ------------------------------------------------------------
    let run = move |cmd: &'static str| {
        spawn_local(async move {
            match invoke_unit(cmd, &NoArgs {}).await {
                Ok(()) => set_error.set(None),
                Err(e) => set_error.set(Some(e)),
            }
            refresh();
        })
    };

    let play_item = move |path: String| {
        set_selected_path.set(Some(path.clone()));
        let sel_list = resolve_selected_list();
        spawn_local(async move {
            let current_playing_path = status.with_untracked(|s| {
                if s.state == "playing" || s.state == "paused" {
                    s.media.as_ref().map(|m| m.path.clone())
                } else {
                    None
                }
            });
            if current_playing_path.as_deref() == Some(path.as_str()) {
                match invoke_unit("toggle_pause", &NoArgs {}).await {
                    Ok(()) => set_error.set(None),
                    Err(e) => set_error.set(Some(e)),
                }
            } else {
                let args = PlayFileArgs {
                    path,
                    selected_paths: Some(sel_list),
                };
                match invoke::<_, MediaInfo>("play_file", &args).await {
                    Ok(_) => set_error.set(None),
                    Err(e) => set_error.set(Some(e)),
                }
            }
            refresh();
        });
    };

    let start_play_all = move |_| {
        spawn_local(async move {
            match invoke::<_, Option<MediaInfo>>("play_all", &NoArgs {}).await {
                Ok(Some(info)) => {
                    set_selected_path.set(Some(info.path));
                    set_error.set(None);
                }
                Ok(None) => {}
                Err(e) => set_error.set(Some(e)),
            }
            refresh();
        });
    };

    let start_play_selected = move |_| {
        let selected_paths = resolve_selected_list();
        if selected_paths.is_empty() {
            return;
        }
        spawn_local(async move {
            match invoke::<_, Option<MediaInfo>>(
                "play_selected",
                &PlaySelectedArgs { selected_paths },
            )
            .await
            {
                Ok(Some(info)) => {
                    set_selected_path.set(Some(info.path));
                    set_error.set(None);
                }
                Ok(None) => {}
                Err(e) => set_error.set(Some(e)),
            }
            refresh();
        });
    };

    let change_play_mode = move |mode: String| {
        set_status.update(|s| s.play_mode = mode.clone());
        spawn_local(async move {
            match invoke_unit("set_play_mode", &ModeArgs { mode }).await {
                Ok(()) => set_error.set(None),
                Err(e) => set_error.set(Some(e)),
            }
            refresh();
        });
    };

    let skip_next = move |_| {
        spawn_local(async move {
            match invoke::<_, Option<MediaInfo>>("next_track", &NoArgs {}).await {
                Ok(Some(info)) => {
                    set_selected_path.set(Some(info.path));
                    set_error.set(None);
                }
                Ok(None) => {}
                Err(e) => set_error.set(Some(e)),
            }
            refresh();
        });
    };

    let skip_prev = move |_| {
        spawn_local(async move {
            match invoke::<_, Option<MediaInfo>>("prev_track", &NoArgs {}).await {
                Ok(Some(info)) => {
                    set_selected_path.set(Some(info.path));
                    set_error.set(None);
                }
                Ok(None) => {}
                Err(e) => set_error.set(Some(e)),
            }
            refresh();
        });
    };

    let play_or_toggle = move |_| {
        let loaded_path = status.with_untracked(|s| s.media.as_ref().map(|m| m.path.clone()));
        let sel = selected_path.get_untracked();
        if let Some(sel_path) = sel {
            if loaded_path.as_deref() != Some(sel_path.as_str()) {
                play_item(sel_path);
                return;
            }
        }
        run("toggle_pause");
    };

    let add_library = move |_| {
        spawn_local(async move {
            match invoke::<_, Option<LibraryResponse>>("add_folder", &NoArgs {}).await {
                Ok(Some(lib)) => {
                    apply_library(lib);
                    set_error.set(None);
                }
                Ok(None) => {}
                Err(e) => set_error.set(Some(e)),
            }
        })
    };

    let switch_library = move |folder: String| {
        spawn_local(async move {
            match invoke::<_, LibraryResponse>("select_folder", &FolderArgs { folder }).await {
                Ok(lib) => {
                    apply_library(lib);
                    set_error.set(None);
                }
                Err(e) => set_error.set(Some(e)),
            }
        })
    };

    let delete_library = move |folder: String| {
        spawn_local(async move {
            match invoke::<_, LibraryResponse>("remove_folder", &FolderArgs { folder }).await {
                Ok(lib) => {
                    apply_library(lib);
                    set_error.set(None);
                }
                Err(e) => set_error.set(Some(e)),
            }
        })
    };

    let open = move |_| {
        spawn_local(async move {
            match invoke::<_, Option<MediaInfo>>("open_file", &NoArgs {}).await {
                Ok(Some(info)) => {
                    set_selected_path.set(Some(info.path));
                    set_error.set(None);
                }
                Ok(None) => {}
                Err(e) => set_error.set(Some(e)),
            }
            refresh();
        })
    };

    let seek_to = move |ms: i64| {
        set_scrub.set(None);
        spawn_local(async move {
            let _ = invoke_unit("seek", &SeekArgs { ms }).await;
            refresh();
        })
    };

    let change_volume = move |volume: i32| {
        set_status.update(|s| s.volume = volume);
        spawn_local(async move {
            let _ = invoke_unit("set_volume", &VolumeArgs { volume }).await;
        })
    };

    // --- derived state ------------------------------------------------------
    let has_media_or_selection =
        move || status.with(|s| s.media.is_some()) || selected_path.with(|p| p.is_some());
    let has_loaded_media = move || status.with(|s| s.media.is_some());
    let has_library_items = move || library.with(|l| !l.items.is_empty());
    let is_playing = move || status.with(|s| s.state == "playing");
    let placeholder = Memo::new(move |_| status.with(placeholder_for));
    let position = move || scrub.get().unwrap_or_else(|| status.with(|s| s.time_ms));
    let length = move || status.with(|s| s.length_ms);

    view! {
        <div class="app">
            <div class="workspace">
                <Show when=move || menu_open.get()>
                    <aside class="side-menu">
                        <div class="side-header">
                            <div class="side-title-row">
                                <span class="side-title">"Media Libraries"</span>
                                <div class="side-header-actions">
                                    <button class="small-btn" on:click=add_library>
                                        "+ Add Library…"
                                    </button>
                                </div>
                            </div>

                            {move || {
                                let lib = library.get();
                                if lib.folders.is_empty() {
                                    None
                                } else {
                                    let active = lib.active_folder.clone().unwrap_or_default();
                                    let multi = lib.folders.len() > 1;
                                    let active_for_all = active.clone();
                                    let active_label = if active == ALL_LIBRARIES_KEY {
                                        format!("All Libraries ({} folders)", lib.folders.len())
                                    } else {
                                        active.clone()
                                    };
                                    let active_label_title = active_label.clone();

                                    Some(view! {
                                        <div class="library-tabs">
                                            {multi.then(|| {
                                                let is_all = active_for_all == ALL_LIBRARIES_KEY;
                                                view! {
                                                    <div
                                                        class="lib-chip"
                                                        class:active=is_all
                                                        on:click=move |_| switch_library(ALL_LIBRARIES_KEY.to_string())
                                                        title="Show media from all libraries"
                                                    >
                                                        <span class="lib-chip-name">"All"</span>
                                                    </div>
                                                }
                                            })}
                                            {lib.folders.into_iter().map(|f| {
                                                let path_select = f.path.clone();
                                                let path_remove = f.path.clone();
                                                let path_title = f.path.clone();
                                                let is_active = active.eq_ignore_ascii_case(&f.path);
                                                view! {
                                                    <div
                                                        class="lib-chip"
                                                        class:active=is_active
                                                        title=path_title
                                                        on:click=move |_| switch_library(path_select.clone())
                                                    >
                                                        <span class="lib-chip-name">{f.name}</span>
                                                        <button
                                                            class="lib-chip-remove"
                                                            title="Remove library"
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                delete_library(path_remove.clone());
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
                                }
                            }}

                            <Show when=has_library_items>
                                <div class="play-controls-box">
                                    <div class="play-actions-row">
                                        <button
                                            class="play-all-btn"
                                            on:click=start_play_all
                                            title="Play all songs inside this folder from the beginning"
                                        >
                                            "▶ Play All"
                                        </button>
                                        <button
                                            class="play-selected-btn"
                                            disabled=move || selected_path.with(|s| s.is_none()) && checked_paths.with(|c| c.is_empty())
                                            on:click=start_play_selected
                                            title="Play the selected song(s)"
                                        >
                                            {move || {
                                                let count = checked_paths.with(|c| c.len());
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
                                            on:change=move |ev| change_play_mode(event_target_value(&ev))
                                        >
                                            <option value=MODE_CONTINUE_NEXT selected=move || status.with(|s| s.play_mode == MODE_CONTINUE_NEXT)>
                                                "Continue play next song in the list"
                                            </option>
                                            <option value=MODE_PLAY_ALL selected=move || status.with(|s| s.play_mode == MODE_PLAY_ALL)>
                                                "Play all"
                                            </option>
                                            <option value=MODE_SELECTED selected=move || status.with(|s| s.play_mode == MODE_SELECTED)>
                                                "Selected"
                                            </option>
                                            <option value=MODE_ONE selected=move || status.with(|s| s.play_mode == MODE_ONE)>
                                                "Play only one song"
                                            </option>
                                        </select>
                                    </div>
                                </div>
                            </Show>
                        </div>

                        <div class="media-list">
                            {move || {
                                let lib = library.get();
                                let show_folder_badge = lib.active_folder.as_deref() == Some(ALL_LIBRARIES_KEY);
                                if lib.folders.is_empty() {
                                    view! {
                                        <div class="empty-library">
                                            <div>"No media libraries added yet."</div>
                                            <button class="primary" on:click=add_library>
                                                "+ Add Media Library…"
                                            </button>
                                        </div>
                                    }.into_any()
                                } else if lib.items.is_empty() {
                                    view! {
                                        <div class="empty-library">
                                            <div>"No audio or video files found in this library."</div>
                                            <button on:click=add_library>"+ Add Another Library…"</button>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="media-items">
                                            {lib.items.into_iter().map(|item| {
                                                let item_path = item.path.clone();
                                                let item_path_click = item.path.clone();
                                                let item_path_dbl = item.path.clone();
                                                let item_path_btn = item.path.clone();
                                                let item_path_active = item.path.clone();
                                                let item_path_playing = item.path.clone();
                                                let item_path_chk_read = item.path.clone();
                                                let item_path_chk_toggle = item.path.clone();
                                                let item_name = item.name.clone();
                                                let item_alt = item.name.clone();
                                                let item_title = item.name.clone();
                                                let item_folder = item.folder.clone();
                                                let is_video = item.kind == "video";

                                                view! {
                                                    <div
                                                        class="media-item"
                                                        class:selected=move || selected_path.with(|s| s.as_deref() == Some(item_path.as_str()))
                                                        class:playing=move || status.with(|s| s.media.as_ref().map(|m| m.path.as_str()) == Some(item_path_active.as_str()))
                                                        on:click=move |_| set_selected_path.set(Some(item_path_click.clone()))
                                                        on:dblclick=move |_| play_item(item_path_dbl.clone())
                                                    >
                                                        <input
                                                            type="checkbox"
                                                            class="item-check"
                                                            title="Select for playlist"
                                                            prop:checked=move || checked_paths.with(|c| c.contains(&item_path_chk_read))
                                                            on:click=move |ev| ev.stop_propagation()
                                                            on:change=move |_| {
                                                                let p = item_path_chk_toggle.clone();
                                                                set_selected_path.set(Some(p.clone()));
                                                                set_checked_paths.update(|set| {
                                                                    if !set.remove(&p) {
                                                                        set.insert(p);
                                                                    }
                                                                });
                                                            }
                                                        />
                                                        <div class="thumb-wrap">
                                                            <img class="thumb" src=item.thumbnail alt=item_alt loading="lazy" />
                                                            <span class="kind-badge" class:video=is_video>
                                                                {if is_video { "VIDEO" } else { "AUDIO" }}
                                                            </span>
                                                        </div>
                                                        <div class="item-meta">
                                                            <div class="item-name" title=item_title>{item_name}</div>
                                                            {show_folder_badge.then(|| view! {
                                                                <div class="item-folder">{item_folder}</div>
                                                            })}
                                                        </div>
                                                        <button
                                                            class="item-play-btn"
                                                            title="Play / Pause"
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                play_item(item_path_btn.clone());
                                                            }
                                                        >
                                                            {move || {
                                                                let this_playing = status.with(|s| {
                                                                    s.state == "playing"
                                                                        && s.media.as_ref().map(|m| m.path.as_str()) == Some(item_path_playing.as_str())
                                                                });
                                                                if this_playing { "⏸" } else { "▶" }
                                                            }}
                                                        </button>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </div>
                    </aside>
                </Show>

                <div class="video-area" node_ref=video_ref>
                    {move || match placeholder.get() {
                        Placeholder::Empty => view! {
                            <div class="empty-stage">
                                <div>"Select a track from the side menu or open a file to start"</div>
                            </div>
                        }.into_any(),
                        Placeholder::Audio { name, thumbnail } => view! {
                            <div class="audio-placeholder">
                                <img class="audio-cover" src=thumbnail alt=name.clone() />
                                <div class="audio-title">{name}</div>
                            </div>
                        }.into_any(),
                        Placeholder::Ready(name) => view! {
                            <div>{format!("{name} — press Play")}</div>
                        }.into_any(),
                    }}
                </div>
            </div>

            <div class="controls">
                <div class="seek-row">
                    <span>{move || fmt_time(position())}</span>
                    <input
                        type="range"
                        min="0"
                        step="100"
                        max=move || length().max(1).to_string()
                        prop:value=move || position().to_string()
                        disabled=move || length() <= 0
                        on:input=move |ev| set_scrub.set(event_target_value(&ev).parse().ok())
                        on:change=move |ev| seek_to(event_target_value(&ev).parse().unwrap_or(0))
                    />
                    <span>{move || fmt_time(length())}</span>
                </div>

                <div class="button-row">
                    <button
                        class:active-toggle=move || menu_open.get()
                        on:click=move |_| set_menu_open.update(|v| *v = !*v)
                        title="Toggle Library Side Menu"
                    >
                        "☰ Library"
                    </button>
                    <button on:click=open>"Open File…"</button>
                    <button
                        disabled=move || !has_library_items()
                        on:click=skip_prev
                        title="Previous track in list"
                    >
                        "⏮"
                    </button>
                    <button
                        class="primary"
                        disabled=move || !has_media_or_selection()
                        on:click=play_or_toggle
                    >
                        {move || if is_playing() { "⏸ Pause" } else { "▶ Play" }}
                    </button>
                    <button
                        disabled=move || !has_library_items()
                        on:click=skip_next
                        title="Next track in list"
                    >
                        "⏭"
                    </button>
                    <button disabled=move || !has_loaded_media() on:click=move |_| run("stop")>
                        "⏹ Stop"
                    </button>
                    <div class="title">
                        {move || status.with(|s| s.media.as_ref().map(|m| m.name.clone()).unwrap_or_default())}
                    </div>
                    <div class="volume">
                        <span>"Vol"</span>
                        <input
                            type="range"
                            min="0"
                            max="100"
                            prop:value=move || status.with(|s| s.volume).to_string()
                            on:input=move |ev| change_volume(event_target_value(&ev).parse().unwrap_or(80))
                        />
                    </div>
                </div>

                {move || error.get().map(|e| view! { <div class="error">{e}</div> })}
            </div>
        </div>
    }
}
