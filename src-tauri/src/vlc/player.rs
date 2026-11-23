//! Safe `Player` wrapper and `State` enum over the libVLC C API.

use super::api::{
    find_vlc_dir, libvlc_file_name, Api, RawPtr, LIBVLC_STATE_BUFFERING, LIBVLC_STATE_ENDED,
    LIBVLC_STATE_ERROR, LIBVLC_STATE_OPENING, LIBVLC_STATE_PAUSED, LIBVLC_STATE_PLAYING,
    LIBVLC_STATE_STOPPED, VLC_INIT_ARGS,
};
use std::ffi::{c_char, c_int, CString};
use std::path::Path;

/// Playback states mirrored from `libvlc_state_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Opening,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Ended,
    Error,
}

impl State {
    /// Convert a raw `libvlc_state_t` integer into a typed `State`.
    pub(crate) fn from_raw(code: c_int) -> Self {
        match code {
            LIBVLC_STATE_OPENING => Self::Opening,
            LIBVLC_STATE_BUFFERING => Self::Buffering,
            LIBVLC_STATE_PLAYING => Self::Playing,
            LIBVLC_STATE_PAUSED => Self::Paused,
            LIBVLC_STATE_STOPPED => Self::Stopped,
            LIBVLC_STATE_ENDED => Self::Ended,
            LIBVLC_STATE_ERROR => Self::Error,
            _ => Self::Idle,
        }
    }

    /// Lowercase string representation serialized across Tauri IPC.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Opening => "opening",
            Self::Buffering => "buffering",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
            Self::Ended => "ended",
            Self::Error => "error",
        }
    }
}

/// Owns a libVLC instance and a single media player handle.
pub struct Player {
    instance: RawPtr,
    mp: RawPtr,
    api: Api,
}

// SAFETY: libVLC media player functions are internally synchronized; raw pointers are opaque handles.
unsafe impl Send for Player {}
unsafe impl Sync for Player {}

impl Player {
    /// Initialize libVLC from the bundled `vlc/` directory next to the executable.
    pub fn new() -> Result<Self, String> {
        let dir = find_vlc_dir().ok_or_else(|| {
            "Bundled VLC runtime not found next to the executable (expected a `vlc` folder with libvlc.dll).".to_string()
        })?;

        let plugins = dir.join("plugins");
        if plugins.is_dir() {
            std::env::set_var("VLC_PLUGIN_PATH", &plugins);
        }

        let api = Api::load(&dir.join(libvlc_file_name()))?;
        let (instance, mp) = create_vlc_handles(&api)?;

        Ok(Self { instance, mp, api })
    }

    /// Bind VLC's video output to a native Win32 window handle (`HWND`).
    #[cfg(windows)]
    pub fn set_hwnd(&self, hwnd: RawPtr) {
        unsafe { (self.api.mp_set_hwnd)(self.mp, hwnd) }
    }

    /// Stop current playback and load a local media file into the player.
    pub fn load(&self, path: &Path) -> Result<(), String> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| format!("path contains a NUL byte: {}", path.display()))?;
        unsafe {
            (self.api.mp_stop)(self.mp);
            let media = (self.api.media_new_path)(self.instance, c_path.as_ptr());
            if media.is_null() {
                return Err(format!(
                    "cannot open {}: {}",
                    path.display(),
                    self.api.last_error()
                ));
            }
            (self.api.mp_set_media)(self.mp, media);
            // Release our reference; the media player retains its own.
            (self.api.media_release)(media);
        }
        Ok(())
    }

    /// Start or resume playback of the loaded media.
    pub fn play(&self) -> Result<(), String> {
        if unsafe { (self.api.mp_play)(self.mp) } == 0 {
            Ok(())
        } else {
            Err(format!("play failed: {}", self.api.last_error()))
        }
    }

    /// Pause playback without unloading the media.
    pub fn pause(&self) {
        unsafe { (self.api.mp_set_pause)(self.mp, 1) }
    }

    /// Stop playback and reset position.
    pub fn stop(&self) {
        unsafe { (self.api.mp_stop)(self.mp) }
    }

    /// Return the current playback state.
    pub fn state(&self) -> State {
        State::from_raw(unsafe { (self.api.mp_get_state)(self.mp) })
    }

    /// Current playback position in milliseconds (`-1` if unavailable).
    pub fn time(&self) -> i64 {
        unsafe { (self.api.mp_get_time)(self.mp) }
    }

    /// Seek to the given position in milliseconds.
    pub fn set_time(&self, ms: i64) {
        unsafe { (self.api.mp_set_time)(self.mp, ms) }
    }

    /// Total duration of the current media in milliseconds (`-1` if unavailable).
    pub fn length(&self) -> i64 {
        unsafe { (self.api.mp_get_length)(self.mp) }
    }

    /// Return `true` while VLC has an active video output surface.
    pub fn has_video(&self) -> bool {
        unsafe { (self.api.mp_has_vout)(self.mp) > 0 }
    }

    /// Current audio volume in `0..=100` (`-1` if no audio output is active).
    pub fn volume(&self) -> i32 {
        unsafe { (self.api.audio_get_volume)(self.mp) }
    }

    /// Set audio volume (`0..=100`); returns `true` on success.
    pub fn set_volume(&self, volume: i32) -> bool {
        unsafe { (self.api.audio_set_volume)(self.mp, volume) == 0 }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            (self.api.mp_stop)(self.mp);
            (self.api.mp_release)(self.mp);
            (self.api.release)(self.instance);
        }
    }
}

fn create_vlc_handles(api: &Api) -> Result<(RawPtr, RawPtr), String> {
    let args: Vec<CString> = VLC_INIT_ARGS
        .iter()
        .map(|arg| CString::new(*arg).expect("static VLC arg contains NUL"))
        .collect();
    let argv: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();

    let instance = unsafe { (api.new)(argv.len() as c_int, argv.as_ptr()) };
    if instance.is_null() {
        return Err(format!("libvlc_new failed: {}", api.last_error()));
    }

    let mp = unsafe { (api.mp_new)(instance) };
    if mp.is_null() {
        let err = api.last_error();
        unsafe { (api.release)(instance) };
        return Err(format!("libvlc_media_player_new failed: {err}"));
    }

    // Keep mouse and keyboard events flowing to the Tauri window instead of VLC.
    unsafe {
        (api.video_set_mouse_input)(mp, 0);
        (api.video_set_key_input)(mp, 0);
    }
    Ok((instance, mp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_raw_libvlc_state_codes_to_expected_variants() {
        assert_eq!(State::from_raw(0), State::Idle);
        assert_eq!(State::from_raw(LIBVLC_STATE_OPENING), State::Opening);
        assert_eq!(State::from_raw(LIBVLC_STATE_BUFFERING), State::Buffering);
        assert_eq!(State::from_raw(LIBVLC_STATE_PLAYING), State::Playing);
        assert_eq!(State::from_raw(LIBVLC_STATE_PAUSED), State::Paused);
        assert_eq!(State::from_raw(LIBVLC_STATE_STOPPED), State::Stopped);
        assert_eq!(State::from_raw(LIBVLC_STATE_ENDED), State::Ended);
        assert_eq!(State::from_raw(LIBVLC_STATE_ERROR), State::Error);
        assert_eq!(State::from_raw(99), State::Idle);
    }

    #[test]
    fn serializes_state_names_as_lowercase_strings() {
        assert_eq!(State::Playing.as_str(), "playing");
        assert_eq!(State::Paused.as_str(), "paused");
        assert_eq!(State::Ended.as_str(), "ended");
    }
}
