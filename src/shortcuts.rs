//! Global keyboard shortcut mapping and listener registration.

use crate::controller::AppController;
use crate::models::SEEK_STEP_MS;
use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

const KEY_ARROW_LEFT: &str = "ArrowLeft";
const KEY_ARROW_RIGHT: &str = "ArrowRight";

/// Return the seek offset in milliseconds for a keyboard `key`, or `None` if unhandled.
pub fn seek_delta_for_key(key: &str) -> Option<i64> {
    match key {
        KEY_ARROW_LEFT => Some(-SEEK_STEP_MS),
        KEY_ARROW_RIGHT => Some(SEEK_STEP_MS),
        _ => None,
    }
}

/// Compute a clamped seek destination in milliseconds, or `None` when media length is unknown.
pub fn compute_seek_target(current_ms: i64, length_ms: i64, delta_ms: i64) -> Option<i64> {
    if length_ms <= 0 {
        return None;
    }
    Some((current_ms + delta_ms).clamp(0, length_ms))
}

/// Register global window keyboard shortcuts (automatically cleaned up on unmount).
pub fn bind_keyboard_shortcuts(ctrl: AppController) {
    let listener = window_event_listener(ev::keydown, move |ev: web_sys::KeyboardEvent| {
        if ev.ctrl_key() || ev.alt_key() || ev.meta_key() {
            return;
        }
        if is_text_input_target(&ev) {
            return;
        }
        let Some(delta_ms) = seek_delta_for_key(&ev.key()) else {
            return;
        };

        // Prevent range sliders or select menus from hijacking Left/Right arrow keys.
        ev.prevent_default();
        ctrl.seek_by(delta_ms);
    });
    on_cleanup(move || listener.remove());
}

fn is_text_input_target(ev: &web_sys::KeyboardEvent) -> bool {
    let Some(target) = ev.target() else {
        return false;
    };
    let Ok(el) = target.dyn_into::<web_sys::Element>() else {
        return false;
    };
    let tag = el.tag_name();
    if tag.eq_ignore_ascii_case("TEXTAREA") {
        return true;
    }
    if tag.eq_ignore_ascii_case("INPUT") {
        let input_type = el.get_attribute("type").unwrap_or_default();
        return matches!(
            input_type.to_ascii_lowercase().as_str(),
            "" | "text" | "search" | "url" | "email" | "password"
        );
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_left_and_right_arrow_keys_to_seek_deltas() {
        assert_eq!(seek_delta_for_key("ArrowLeft"), Some(-SEEK_STEP_MS));
        assert_eq!(seek_delta_for_key("ArrowRight"), Some(SEEK_STEP_MS));
        assert_eq!(seek_delta_for_key("Space"), None);
    }

    #[test]
    fn clamps_seek_target_within_media_bounds() {
        assert_eq!(compute_seek_target(10_000, 60_000, 5_000), Some(15_000));
        assert_eq!(compute_seek_target(2_000, 60_000, -5_000), Some(0));
        assert_eq!(compute_seek_target(58_000, 60_000, 5_000), Some(60_000));
    }

    #[test]
    fn returns_none_when_media_has_no_known_length() {
        assert_eq!(compute_seek_target(0, 0, 5_000), None);
        assert_eq!(compute_seek_target(0, -1, -5_000), None);
    }
}
