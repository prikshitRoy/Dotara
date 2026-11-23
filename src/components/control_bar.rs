//! Bottom transport bar: seek slider, sidebar toggle, playback buttons, title, volume, and errors.

use crate::formatting::format_duration;
use crate::models::{DEFAULT_VOLUME, Status};
use leptos::prelude::*;

/// Bottom media transport controls docked beneath the workspace.
#[component]
pub fn ControlBar(
    status: ReadSignal<Status>,
    error: ReadSignal<Option<String>>,
    is_menu_open: ReadSignal<bool>,
    has_library_items: Signal<bool>,
    has_media_or_selection: Signal<bool>,
    on_toggle_menu: Callback<()>,
    on_open_file: Callback<()>,
    on_prev: Callback<()>,
    on_play_or_toggle: Callback<()>,
    on_next: Callback<()>,
    on_stop: Callback<()>,
    on_seek: Callback<i64>,
    on_volume: Callback<i32>,
) -> impl IntoView {
    // Holds the user's in-flight slider position while dragging so status polls don't fight it.
    let (scrub_ms, set_scrub_ms) = signal(None::<i64>);

    let position = move || scrub_ms.get().unwrap_or_else(|| status.with(|s| s.time_ms));
    let length = move || status.with(|s| s.length_ms);
    let is_playing = move || status.with(|s| s.state == "playing");
    let has_loaded_media = move || status.with(|s| s.media.is_some());

    view! {
        <div class="controls">
            <div class="seek-row">
                <span>{move || format_duration(position())}</span>
                <input
                    type="range"
                    min="0"
                    step="100"
                    max=move || length().max(1).to_string()
                    prop:value=move || position().to_string()
                    disabled=move || length() <= 0
                    on:input=move |ev| set_scrub_ms.set(event_target_value(&ev).parse().ok())
                    on:change=move |ev| {
                        set_scrub_ms.set(None);
                        let target = event_target_value(&ev).parse().unwrap_or(0);
                        on_seek.run(target);
                    }
                />
                <span>{move || format_duration(length())}</span>
            </div>

            <div class="button-row">
                <button
                    class:active-toggle=move || is_menu_open.get()
                    on:click=move |_| on_toggle_menu.run(())
                    title="Toggle Library Side Menu"
                >
                    "☰ Library"
                </button>
                <button on:click=move |_| on_open_file.run(())>"Open File…"</button>
                <button
                    disabled=move || !has_library_items.get()
                    on:click=move |_| on_prev.run(())
                    title="Previous track in list"
                >
                    "⏮"
                </button>
                <button
                    class="primary"
                    disabled=move || !has_media_or_selection.get()
                    on:click=move |_| on_play_or_toggle.run(())
                >
                    {move || if is_playing() { "⏸ Pause" } else { "▶ Play" }}
                </button>
                <button
                    disabled=move || !has_library_items.get()
                    on:click=move |_| on_next.run(())
                    title="Next track in list"
                >
                    "⏭"
                </button>
                <button
                    disabled=move || !has_loaded_media()
                    on:click=move |_| on_stop.run(())
                >
                    "⏹ Stop"
                </button>
                <div class="title">
                    {move || {
                        status.with(|s| s.media.as_ref().map(|m| m.name.clone()).unwrap_or_default())
                    }}
                </div>
                <div class="volume">
                    <span>"Vol"</span>
                    <input
                        type="range"
                        min="0"
                        max="100"
                        prop:value=move || status.with(|s| s.volume).to_string()
                        on:input=move |ev| {
                            let vol = event_target_value(&ev).parse().unwrap_or(DEFAULT_VOLUME);
                            on_volume.run(vol);
                        }
                    />
                </div>
            </div>

            {move || error.get().map(|msg| view! { <div class="error">{msg}</div> })}
        </div>
    }
}
