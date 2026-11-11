use crate::tauri::{invoke, invoke_unit, NoArgs};
use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: String,
    pub time_ms: i64,
    pub length_ms: i64,
    pub volume: i32,
    pub has_video: bool,
    pub media: Option<MediaInfo>,
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

/// What to show in the video area when the native VLC surface is hidden.
#[derive(Clone, PartialEq)]
enum Placeholder {
    Empty,
    Audio(String),
    Ready(String),
}

fn placeholder_for(s: &Status) -> Placeholder {
    match &s.media {
        None => Placeholder::Empty,
        Some(m) if !s.has_video && matches!(s.state.as_str(), "playing" | "paused" | "buffering") => {
            Placeholder::Audio(m.name.clone())
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
    let video_ref = NodeRef::<leptos::html::Div>::new();

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
        if video_ref.get().is_some() {
            report_bounds();
        }
    });
    let resize = window_event_listener(ev::resize, move |_| report_bounds());
    on_cleanup(move || resize.remove());

    // Load testvideo.webm (if present) on startup.
    spawn_local(async move {
        if let Err(e) = invoke::<_, Option<MediaInfo>>("load_default", &NoArgs {}).await {
            set_error.set(Some(e));
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
    let open = move |_| {
        spawn_local(async move {
            match invoke::<_, Option<MediaInfo>>("open_file", &NoArgs {}).await {
                Ok(_) => set_error.set(None),
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
    let has_media = move || status.with(|s| s.media.is_some());
    let is_playing = move || status.with(|s| s.state == "playing");
    let placeholder = Memo::new(move |_| status.with(placeholder_for));
    let position = move || scrub.get().unwrap_or_else(|| status.with(|s| s.time_ms));
    let length = move || status.with(|s| s.length_ms);

    view! {
        <div class="app">
            <div class="video-area" node_ref=video_ref>
                {move || match placeholder.get() {
                    Placeholder::Empty => view! { <div>"Open a video or audio file to start"</div> }.into_any(),
                    Placeholder::Audio(name) => view! {
                        <div class="audio-placeholder">
                            <div class="note">"♪"</div>
                            <div>{name}</div>
                        </div>
                    }.into_any(),
                    Placeholder::Ready(name) => view! { <div>{format!("{name} — press Play")}</div> }.into_any(),
                }}
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
                    <button on:click=open>"Open…"</button>
                    <button class="primary" disabled=move || !has_media() on:click=move |_| run("toggle_pause")>
                        {move || if is_playing() { "⏸ Pause" } else { "▶ Play" }}
                    </button>
                    <button disabled=move || !has_media() on:click=move |_| run("stop")>"⏹ Stop"</button>
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
