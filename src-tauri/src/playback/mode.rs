//! Playback modes controlling how Dotara advances when a track finishes.

/// Supported playback progression modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayMode {
    /// Advance to the next track in the list and stop after the last track.
    #[default]
    ContinueNext,
    /// Play through every track in the list and wrap back to the first track.
    PlayAll,
    /// Play only the user's selected track(s) in order and then stop.
    Selected,
    /// Stop after the current track finishes without advancing.
    One,
}

impl PlayMode {
    pub const CONTINUE_NEXT_STR: &'static str = "continue_next";
    pub const PLAY_ALL_STR: &'static str = "play_all";
    pub const SELECTED_STR: &'static str = "selected";
    pub const ONE_STR: &'static str = "one";

    /// Parse a wire-format mode string coming from the UI.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            Self::CONTINUE_NEXT_STR => Ok(Self::ContinueNext),
            Self::PLAY_ALL_STR => Ok(Self::PlayAll),
            Self::SELECTED_STR => Ok(Self::Selected),
            Self::ONE_STR => Ok(Self::One),
            other => Err(format!("Invalid play mode: {other}")),
        }
    }

    /// Wire-format string serialized to the UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContinueNext => Self::CONTINUE_NEXT_STR,
            Self::PlayAll => Self::PLAY_ALL_STR,
            Self::Selected => Self::SELECTED_STR,
            Self::One => Self::ONE_STR,
        }
    }

    /// Adjust mode when the user explicitly clicks "Play All" so the list actually continues.
    pub fn resolved_for_play_all(self) -> Self {
        match self {
            Self::One | Self::Selected => Self::ContinueNext,
            Self::ContinueNext | Self::PlayAll => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_continue_next() {
        assert_eq!(PlayMode::default(), PlayMode::ContinueNext);
    }

    #[test]
    fn parses_all_valid_mode_strings() {
        assert_eq!(PlayMode::parse("continue_next"), Ok(PlayMode::ContinueNext));
        assert_eq!(PlayMode::parse("play_all"), Ok(PlayMode::PlayAll));
        assert_eq!(PlayMode::parse("selected"), Ok(PlayMode::Selected));
        assert_eq!(PlayMode::parse("one"), Ok(PlayMode::One));
    }

    #[test]
    fn rejects_unknown_mode_string() {
        let result = PlayMode::parse("shuffle");

        assert!(result.is_err());
    }

    #[test]
    fn resolves_single_and_selected_modes_to_continue_next_on_play_all() {
        assert_eq!(PlayMode::One.resolved_for_play_all(), PlayMode::ContinueNext);
        assert_eq!(
            PlayMode::Selected.resolved_for_play_all(),
            PlayMode::ContinueNext
        );
        assert_eq!(PlayMode::PlayAll.resolved_for_play_all(), PlayMode::PlayAll);
        assert_eq!(
            PlayMode::ContinueNext.resolved_for_play_all(),
            PlayMode::ContinueNext
        );
    }
}
