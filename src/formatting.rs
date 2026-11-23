//! Pure UI formatting, label derivation, and selection ordering helpers.

use crate::models::{LibraryItem, Placeholder, Status, ALL_LIBRARIES_KEY};
use std::collections::HashSet;

const MS_PER_SECOND: i64 = 1000;
const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 3600;

/// Format a duration in milliseconds as `m:ss` or `h:mm:ss`.
pub fn format_duration(ms: i64) -> String {
    let total_secs = ms.max(0) / MS_PER_SECOND;
    let hours = total_secs / SECONDS_PER_HOUR;
    let minutes = (total_secs % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let seconds = total_secs % SECONDS_PER_MINUTE;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Decide what placeholder to show inside `.video-area` when no native video output is active.
pub fn placeholder_for(status: &Status) -> Placeholder {
    let Some(media) = &status.media else {
        return Placeholder::Empty;
    };
    let is_active_playback = matches!(status.state.as_str(), "playing" | "paused" | "buffering");
    if !status.has_video && is_active_playback {
        return Placeholder::Audio {
            name: media.name.clone(),
            thumbnail: media.thumbnail.clone(),
        };
    }
    Placeholder::Ready(media.name.clone())
}

/// Build the folder-path label shown under the library chips.
pub fn active_library_label(active_folder: &str, folder_count: usize) -> String {
    if active_folder == ALL_LIBRARIES_KEY {
        format!("All Libraries ({folder_count} folders)")
    } else {
        active_folder.to_string()
    }
}

/// Return checked tracks in library display order, falling back to the single highlighted row.
pub fn resolve_selected_paths(
    items: &[LibraryItem],
    checked: &HashSet<String>,
    selected: Option<&str>,
) -> Vec<String> {
    if !checked.is_empty() {
        let ordered: Vec<String> = items
            .iter()
            .filter(|item| checked.contains(&item.path))
            .map(|item| item.path.clone())
            .collect();
        if !ordered.is_empty() {
            return ordered;
        }
    }
    selected.map(str::to_string).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MediaInfo;

    #[test]
    fn formats_negative_zero_minute_and_hour_durations() {
        assert_eq!(format_duration(-500), "0:00");
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(65_000), "1:05");
        assert_eq!(format_duration(3_661_000), "1:01:01");
    }

    #[test]
    fn derives_audio_placeholder_only_during_active_audio_playback() {
        let media = MediaInfo {
            path: "song.mp3".into(),
            name: "Song".into(),
            kind: "audio".into(),
            thumbnail: "data:image/png;base64,abc".into(),
        };
        let playing_audio = Status {
            state: "playing".into(),
            has_video: false,
            media: Some(media.clone()),
            ..Status::default()
        };
        let stopped_audio = Status {
            state: "stopped".into(),
            has_video: false,
            media: Some(media),
            ..Status::default()
        };

        assert_eq!(
            placeholder_for(&playing_audio),
            Placeholder::Audio {
                name: "Song".into(),
                thumbnail: "data:image/png;base64,abc".into(),
            }
        );
        assert_eq!(
            placeholder_for(&stopped_audio),
            Placeholder::Ready("Song".into())
        );
        assert_eq!(placeholder_for(&Status::default()), Placeholder::Empty);
    }

    #[test]
    fn formats_active_library_label_for_all_and_single_folder() {
        assert_eq!(
            active_library_label(ALL_LIBRARIES_KEY, 3),
            "All Libraries (3 folders)"
        );
        assert_eq!(active_library_label(r"E:\Songs", 1), r"E:\Songs");
    }

    #[test]
    fn resolves_checked_items_in_library_order_over_single_selection() {
        let items = vec![
            LibraryItem {
                path: "1.mp3".into(),
                ..LibraryItem::default()
            },
            LibraryItem {
                path: "2.mp3".into(),
                ..LibraryItem::default()
            },
            LibraryItem {
                path: "3.mp3".into(),
                ..LibraryItem::default()
            },
        ];
        let checked: HashSet<String> = ["3.mp3".into(), "1.mp3".into()].into_iter().collect();

        assert_eq!(
            resolve_selected_paths(&items, &checked, Some("2.mp3")),
            vec!["1.mp3".to_string(), "3.mp3".to_string()]
        );
        assert_eq!(
            resolve_selected_paths(&items, &HashSet::new(), Some("2.mp3")),
            vec!["2.mp3".to_string()]
        );
    }
}
