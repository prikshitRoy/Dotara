//! Ordered playback queue and track-advancement rules.

use super::PlayMode;

/// Holds the ordered list of track paths currently queued for playback.
#[derive(Debug, Default, Clone)]
pub struct PlaybackQueue {
    paths: Vec<String>,
}

impl PlaybackQueue {
    /// Create a queue pre-populated with track paths.
    pub fn new(paths: Vec<String>) -> Self {
        Self { paths }
    }

    /// Return `true` when the queue contains no tracks.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Replace the queued tracks with `paths`.
    pub fn set_paths(&mut self, paths: Vec<String>) {
        self.paths = paths;
    }

    /// Choose the queue for a "Play Selected" action based on selection count and mode.
    pub fn build_for_selected(
        selected_paths: Vec<String>,
        folder_paths: Vec<String>,
        mode: PlayMode,
    ) -> Vec<String> {
        if selected_paths.len() > 1 || mode == PlayMode::Selected {
            selected_paths
        } else {
            folder_paths
        }
    }

    /// Choose the queue when starting a single file from the library list.
    pub fn build_for_file(
        path: &str,
        selected_paths: Option<Vec<String>>,
        folder_paths: Vec<String>,
        mode: PlayMode,
    ) -> Vec<String> {
        if mode == PlayMode::Selected {
            let matching_selection = selected_paths
                .filter(|paths| !paths.is_empty() && paths.iter().any(|p| p == path));
            return matching_selection.unwrap_or_else(|| vec![path.to_string()]);
        }

        if folder_paths.iter().any(|fp| fp == path) {
            folder_paths
        } else {
            vec![path.to_string()]
        }
    }

    /// Decide which track to auto-play after `current_path` reaches `Ended`.
    pub fn next_after_end(&self, current_path: Option<&str>, mode: PlayMode) -> Option<String> {
        if mode == PlayMode::One {
            return None;
        }
        let idx = self.index_of(current_path)?;
        if idx + 1 < self.paths.len() {
            return Some(self.paths[idx + 1].clone());
        }
        if mode == PlayMode::PlayAll {
            return self.paths.first().cloned();
        }
        None
    }

    /// Decide which track to play when the user clicks "Next".
    pub fn next_manual(&self, current_path: Option<&str>, mode: PlayMode) -> Option<String> {
        let Some(idx) = self.index_of(current_path) else {
            return self.paths.first().cloned();
        };
        if idx + 1 < self.paths.len() {
            return Some(self.paths[idx + 1].clone());
        }
        if mode == PlayMode::PlayAll {
            return self.paths.first().cloned();
        }
        None
    }

    /// Decide which track to play when the user clicks "Previous".
    pub fn prev_manual(&self, current_path: Option<&str>, mode: PlayMode) -> Option<String> {
        let idx = self.index_of(current_path)?;
        if idx > 0 {
            return Some(self.paths[idx - 1].clone());
        }
        if mode == PlayMode::PlayAll {
            return self.paths.last().cloned();
        }
        None
    }

    fn index_of(&self, current_path: Option<&str>) -> Option<usize> {
        let target = current_path?;
        self.paths.iter().position(|p| p == target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_queue() -> PlaybackQueue {
        PlaybackQueue::new(vec!["a.mp3".into(), "b.mp3".into(), "c.mp3".into()])
    }

    #[test]
    fn continues_to_next_track_and_stops_at_end_in_continue_next_mode() {
        let queue = sample_queue();

        assert_eq!(
            queue.next_after_end(Some("a.mp3"), PlayMode::ContinueNext),
            Some("b.mp3".into())
        );
        assert_eq!(
            queue.next_after_end(Some("c.mp3"), PlayMode::ContinueNext),
            None
        );
    }

    #[test]
    fn wraps_to_first_track_at_end_in_play_all_mode() {
        let queue = sample_queue();

        assert_eq!(
            queue.next_after_end(Some("c.mp3"), PlayMode::PlayAll),
            Some("a.mp3".into())
        );
    }

    #[test]
    fn never_auto_advances_in_one_mode() {
        let queue = sample_queue();

        assert_eq!(queue.next_after_end(Some("a.mp3"), PlayMode::One), None);
    }

    #[test]
    fn manual_next_starts_from_first_track_when_nothing_is_loaded() {
        let queue = sample_queue();

        assert_eq!(
            queue.next_manual(None, PlayMode::ContinueNext),
            Some("a.mp3".into())
        );
    }

    #[test]
    fn manual_prev_wraps_only_in_play_all_mode() {
        let queue = sample_queue();

        assert_eq!(queue.prev_manual(Some("a.mp3"), PlayMode::ContinueNext), None);
        assert_eq!(
            queue.prev_manual(Some("a.mp3"), PlayMode::PlayAll),
            Some("c.mp3".into())
        );
        assert_eq!(
            queue.prev_manual(Some("b.mp3"), PlayMode::ContinueNext),
            Some("a.mp3".into())
        );
    }

    #[test]
    fn returns_none_for_empty_queue() {
        let queue = PlaybackQueue::default();

        assert_eq!(queue.next_after_end(Some("a.mp3"), PlayMode::PlayAll), None);
        assert_eq!(queue.next_manual(None, PlayMode::PlayAll), None);
        assert_eq!(queue.prev_manual(Some("a.mp3"), PlayMode::PlayAll), None);
    }

    #[test]
    fn build_for_selected_uses_folder_for_single_track_unless_in_selected_mode() {
        let folder = vec!["a.mp3".into(), "b.mp3".into()];
        let single = vec!["a.mp3".into()];

        assert_eq!(
            PlaybackQueue::build_for_selected(single.clone(), folder.clone(), PlayMode::ContinueNext),
            folder
        );
        assert_eq!(
            PlaybackQueue::build_for_selected(single.clone(), folder, PlayMode::Selected),
            single
        );
    }

    #[test]
    fn build_for_file_keeps_multi_selection_only_when_clicked_file_is_included() {
        let folder = vec!["a.mp3".into(), "b.mp3".into(), "c.mp3".into()];
        let checked = vec!["a.mp3".into(), "c.mp3".into()];

        assert_eq!(
            PlaybackQueue::build_for_file(
                "a.mp3",
                Some(checked.clone()),
                folder.clone(),
                PlayMode::Selected
            ),
            checked
        );
        assert_eq!(
            PlaybackQueue::build_for_file("b.mp3", Some(checked), folder, PlayMode::Selected),
            vec!["b.mp3".to_string()]
        );
    }
}
