//! Global keyboard shortcut mapping and listener registration.

use crate::controller::AppController;
use crate::models::{MAX_VOLUME, MIN_VOLUME, SEEK_STEP_MS, VOLUME_STEP};
use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

const KEY_ARROW_LEFT: &str = "ArrowLeft";
const KEY_ARROW_RIGHT: &str = "ArrowRight";
const KEY_ARROW_UP: &str = "ArrowUp";
const KEY_ARROW_DOWN: &str = "ArrowDown";

/// Player action triggered by a global keyboard shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    SeekBy(i64),
    VolumeBy(i32),
}

/// Map a DOM `KeyboardEvent.key` value to a `ShortcutAction`, or `None` if unhandled.
pub fn action_for_key(key: &str) -> Option<ShortcutAction> {
    match key {
        KEY_ARROW_LEFT => Some(ShortcutAction::SeekBy(-SEEK_STEP_MS)),
        KEY_ARROW_RIGHT => Some(ShortcutAction::SeekBy(SEEK_STEP_MS)),
        KEY_ARROW_UP => Some(ShortcutAction::VolumeBy(VOLUME_STEP)),
        KEY_ARROW_DOWN => Some(ShortcutAction::VolumeBy(-VOLUME_STEP)),
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

/// Compute a clamped audio volume in `0..=100`.
pub fn compute_volume_target(current_volume: i32, delta: i32) -> i32 {
    (current_volume + delta).clamp(MIN_VOLUME, MAX_VOLUME)
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
        let Some(action) = action_for_key(&ev.key()) else {
            return;
        };

        // Prevent range sliders, dropdowns, or scroll containers from hijacking arrow shortcuts.
        ev.prevent_default();
        match action {
            ShortcutAction::SeekBy(delta_ms) => ctrl.seek_by(delta_ms),
            ShortcutAction::VolumeBy(delta) => ctrl.adjust_volume(delta),
        }
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
    fn maps_arrow_keys_to_seek_and_volume_actions() {
        assert_eq!(
            action_for_key("ArrowLeft"),
            Some(ShortcutAction::SeekBy(-SEEK_STEP_MS))
        );
        assert_eq!(
            action_for_key("ArrowRight"),
            Some(ShortcutAction::SeekBy(SEEK_STEP_MS))
        );
        assert_eq!(
            action_for_key("ArrowUp"),
            Some(ShortcutAction::VolumeBy(VOLUME_STEP))
        );
        assert_eq!(
            action_for_key("ArrowDown"),
            Some(ShortcutAction::VolumeBy(-VOLUME_STEP))
        );
        assert_eq!(action_for_key("Space"), None);
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

    #[test]
    fn clamps_volume_target_within_zero_to_hundred() {
        assert_eq!(compute_volume_target(50, 5), 55);
        assert_eq!(compute_volume_target(2, -5), MIN_VOLUME);
        assert_eq!(compute_volume_target(98, 5), MAX_VOLUME);
    }
}
