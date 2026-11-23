//! Root `App` component composing the library sidebar, video stage, and transport controls.

use crate::api;
use crate::components::{ControlBar, LibraryHeader, MediaList, VideoStage};
use crate::controller::AppController;
use crate::formatting::placeholder_for;
use crate::models::{BoundsArgs, BOUNDS_SETTLE_DELAY_MS, POLL_INTERVAL_MS};
use crate::shortcuts::bind_keyboard_shortcuts;
use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::time::Duration;

/// Top-level Dotara application shell.
#[component]
pub fn App() -> impl IntoView {
    let ctrl = AppController::new();
    let video_ref = NodeRef::<leptos::html::Div>::new();

    if let Ok(handle) = set_interval_with_handle(
        move || ctrl.refresh_status(),
        Duration::from_millis(POLL_INTERVAL_MS),
    ) {
        on_cleanup(move || handle.clear());
    }

    sync_video_bounds(video_ref, ctrl.is_menu_open);
    bind_keyboard_shortcuts(ctrl);
    ctrl.load_initial_library();

    let placeholder = Memo::new(move |_| ctrl.status.with(placeholder_for));
    let has_library_items = Signal::derive(move || ctrl.library.with(|l| !l.items.is_empty()));
    let has_media_or_selection = Signal::derive(move || {
        ctrl.status.with(|s| s.media.is_some()) || ctrl.selected_path.with(Option::is_some)
    });

    view! {
        <div class="app">
            <div class="workspace">
                <Show when=move || ctrl.is_menu_open.get()>
                    <aside class="side-menu">
                        <LibraryHeader
                            library=ctrl.library
                            status=ctrl.status
                            selected_path=ctrl.selected_path
                            checked_paths=ctrl.checked_paths
                            on_add_library=Callback::new(move |()| ctrl.add_library())
                            on_switch_library=Callback::new(move |f| ctrl.switch_library(f))
                            on_remove_library=Callback::new(move |f| ctrl.remove_library(f))
                            on_play_all=Callback::new(move |()| ctrl.play_all())
                            on_play_selected=Callback::new(move |()| ctrl.play_selected())
                            on_change_mode=Callback::new(move |m| ctrl.change_mode(m))
                        />
                        <MediaList
                            library=ctrl.library
                            status=ctrl.status
                            selected_path=ctrl.selected_path
                            checked_paths=ctrl.checked_paths
                            on_add_library=Callback::new(move |()| ctrl.add_library())
                            on_select=Callback::new(move |p| ctrl.select_row(p))
                            on_toggle_check=Callback::new(move |p| ctrl.toggle_check(p))
                            on_play_item=Callback::new(move |p| ctrl.play_item(p))
                        />
                    </aside>
                </Show>

                <VideoStage video_ref=video_ref placeholder=placeholder />
            </div>

            <ControlBar
                status=ctrl.status
                error=ctrl.error
                is_menu_open=ctrl.is_menu_open
                has_library_items=has_library_items
                has_media_or_selection=has_media_or_selection
                on_toggle_menu=Callback::new(move |()| ctrl.toggle_menu())
                on_open_file=Callback::new(move |()| ctrl.open_file())
                on_prev=Callback::new(move |()| ctrl.step_track(false))
                on_play_or_toggle=Callback::new(move |()| ctrl.play_or_toggle())
                on_next=Callback::new(move |()| ctrl.step_track(true))
                on_stop=Callback::new(move |()| ctrl.stop())
                on_seek=Callback::new(move |ms| ctrl.seek(ms))
                on_volume=Callback::new(move |v| ctrl.set_volume(v))
            />
        </div>
    }
}

/// Keep the Win32 video child window aligned with `.video-area` on mount, resize, and sidebar toggle.
fn sync_video_bounds(
    video_ref: NodeRef<leptos::html::Div>,
    is_menu_open: ReadSignal<bool>,
) {
    let report_bounds = move || {
        let Some(el) = video_ref.get_untracked() else {
            return;
        };
        let rect = el.get_bounding_client_rect();
        let dpr = window().device_pixel_ratio();
        let bounds = BoundsArgs {
            x: rect.left() * dpr,
            y: rect.top() * dpr,
            width: rect.width() * dpr,
            height: rect.height() * dpr,
        };
        spawn_local(async move {
            let _ = api::set_video_bounds(bounds).await;
        });
    };

    Effect::new(move |_| {
        let _ = is_menu_open.get();
        if video_ref.get().is_some() {
            report_bounds();
            set_timeout(report_bounds, Duration::from_millis(BOUNDS_SETTLE_DELAY_MS));
        }
    });

    let resize = window_event_listener(ev::resize, move |_| report_bounds());
    on_cleanup(move || resize.remove());
}
