//! Main playback stage where the native Win32 video surface docks or audio cover art is shown.

use crate::models::Placeholder;
use leptos::prelude::*;

/// Video surface anchor and audio/empty placeholder view.
#[component]
pub fn VideoStage(
    video_ref: NodeRef<leptos::html::Div>,
    placeholder: Memo<Placeholder>,
) -> impl IntoView {
    view! {
        <div class="video-area" node_ref=video_ref>
            {move || match placeholder.get() {
                Placeholder::Empty => view! {
                    <div class="empty-stage">
                        <div>"Select a track from the side menu or open a file to start"</div>
                    </div>
                }
                .into_any(),
                Placeholder::Audio { name, thumbnail } => {
                    let alt = name.clone();
                    view! {
                        <div class="audio-placeholder">
                            <img class="audio-cover" src=thumbnail alt=alt />
                            <div class="audio-title">{name}</div>
                        </div>
                    }
                    .into_any()
                }
                Placeholder::Ready(name) => view! {
                    <div>{format!("{name} — press Play")}</div>
                }
                .into_any(),
            }}
        </div>
    }
}
