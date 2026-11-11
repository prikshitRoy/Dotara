//! Minimal libVLC 3.x binding.
//!
//! `libvlc.dll` is loaded at runtime with `libloading`, so no VLC SDK, headers or
//! import libraries are needed at build time. Only the handful of functions the
//! player uses are resolved.

use libloading::Library;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::path::{Path, PathBuf};

type Ptr = *mut c_void;

/// Function table resolved from `libvlc.dll`.
struct Api {
    new: unsafe extern "C" fn(c_int, *const *const c_char) -> Ptr,
    release: unsafe extern "C" fn(Ptr),
    errmsg: unsafe extern "C" fn() -> *const c_char,
    media_new_path: unsafe extern "C" fn(Ptr, *const c_char) -> Ptr,
    media_release: unsafe extern "C" fn(Ptr),
    mp_new: unsafe extern "C" fn(Ptr) -> Ptr,
    mp_release: unsafe extern "C" fn(Ptr),
    mp_set_media: unsafe extern "C" fn(Ptr, Ptr),
    mp_play: unsafe extern "C" fn(Ptr) -> c_int,
    mp_set_pause: unsafe extern "C" fn(Ptr, c_int),
    mp_stop: unsafe extern "C" fn(Ptr),
    mp_get_state: unsafe extern "C" fn(Ptr) -> c_int,
    mp_get_time: unsafe extern "C" fn(Ptr) -> i64,
    mp_set_time: unsafe extern "C" fn(Ptr, i64),
    mp_get_length: unsafe extern "C" fn(Ptr) -> i64,
    mp_has_vout: unsafe extern "C" fn(Ptr) -> c_uint,
    #[cfg(windows)]
    mp_set_hwnd: unsafe extern "C" fn(Ptr, Ptr),
    video_set_mouse_input: unsafe extern "C" fn(Ptr, c_uint),
    video_set_key_input: unsafe extern "C" fn(Ptr, c_uint),
    audio_get_volume: unsafe extern "C" fn(Ptr) -> c_int,
    audio_set_volume: unsafe extern "C" fn(Ptr, c_int) -> c_int,
    // Must be the last field: dropped after every function pointer above.
    _lib: Library,
}

unsafe fn sym<T: Copy>(lib: &Library, name: &str) -> Result<T, String> {
    lib.get::<T>(name.as_bytes())
        .map(|s| *s)
        .map_err(|e| format!("libvlc is missing `{name}`: {e}"))
}

impl Api {
    fn load(dll: &Path) -> Result<Self, String> {
        #[cfg(windows)]
        let lib: Library = unsafe {
            // Altered search path lets libvlc.dll find libvlccore.dll next to it.
            libloading::os::windows::Library::load_with_flags(
                dll,
                libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        }
        .map_err(|e| format!("failed to load {}: {e}", dll.display()))?
        .into();
        #[cfg(not(windows))]
        let lib = unsafe { Library::new(dll) }
            .map_err(|e| format!("failed to load {}: {e}", dll.display()))?;

        unsafe {
            Ok(Self {
                new: sym(&lib, "libvlc_new")?,
                release: sym(&lib, "libvlc_release")?,
                errmsg: sym(&lib, "libvlc_errmsg")?,
                media_new_path: sym(&lib, "libvlc_media_new_path")?,
                media_release: sym(&lib, "libvlc_media_release")?,
                mp_new: sym(&lib, "libvlc_media_player_new")?,
                mp_release: sym(&lib, "libvlc_media_player_release")?,
                mp_set_media: sym(&lib, "libvlc_media_player_set_media")?,
                mp_play: sym(&lib, "libvlc_media_player_play")?,
                mp_set_pause: sym(&lib, "libvlc_media_player_set_pause")?,
                mp_stop: sym(&lib, "libvlc_media_player_stop")?,
                mp_get_state: sym(&lib, "libvlc_media_player_get_state")?,
                mp_get_time: sym(&lib, "libvlc_media_player_get_time")?,
                mp_set_time: sym(&lib, "libvlc_media_player_set_time")?,
                mp_get_length: sym(&lib, "libvlc_media_player_get_length")?,
                mp_has_vout: sym(&lib, "libvlc_media_player_has_vout")?,
                #[cfg(windows)]
                mp_set_hwnd: sym(&lib, "libvlc_media_player_set_hwnd")?,
                video_set_mouse_input: sym(&lib, "libvlc_video_set_mouse_input")?,
                video_set_key_input: sym(&lib, "libvlc_video_set_key_input")?,
                audio_get_volume: sym(&lib, "libvlc_audio_get_volume")?,
                audio_set_volume: sym(&lib, "libvlc_audio_set_volume")?,
                _lib: lib,
            })
        }
    }

    fn last_error(&self) -> String {
        let p = unsafe { (self.errmsg)() };
        if p.is_null() {
            "unknown libvlc error".into()
        } else {
            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
        }
    }
}

/// Mirrors `libvlc_state_t`.
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
    pub fn as_str(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Opening => "opening",
            State::Buffering => "buffering",
            State::Playing => "playing",
            State::Paused => "paused",
            State::Stopped => "stopped",
            State::Ended => "ended",
            State::Error => "error",
        }
    }
}

/// Locate the directory holding `libvlc.dll`.
///
/// Search order: `DOTARA_VLC_DIR` env var, a `vlc` folder next to the executable
/// (for bundled builds), the executable folder itself, then standard VLC installs.
fn find_vlc_dir() -> Option<PathBuf> {
    let dll = libvlc_file_name();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("DOTARA_VLC_DIR") {
        candidates.push(dir.into());
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        candidates.push(exe_dir.join("vlc"));
        candidates.push(exe_dir);
    }
    #[cfg(windows)]
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(pf) = std::env::var_os(var) {
            candidates.push(PathBuf::from(pf).join("VideoLAN").join("VLC"));
        }
    }
    candidates.into_iter().find(|d| d.join(dll).is_file())
}

fn libvlc_file_name() -> &'static str {
    if cfg!(windows) {
        "libvlc.dll"
    } else if cfg!(target_os = "macos") {
        "libvlc.dylib"
    } else {
        "libvlc.so.5"
    }
}

/// A libVLC instance plus a single media player.
pub struct Player {
    instance: Ptr,
    mp: Ptr,
    api: Api,
}

// libVLC's media player API is thread-safe; the raw pointers are only handles.
unsafe impl Send for Player {}
unsafe impl Sync for Player {}

impl Player {
    pub fn new() -> Result<Self, String> {
        let dir = find_vlc_dir().ok_or_else(|| {
            "Could not find VLC. Install 64-bit VLC 3.x or set DOTARA_VLC_DIR to the folder containing libvlc.dll.".to_string()
        })?;

        let plugins = dir.join("plugins");
        if plugins.is_dir() && std::env::var_os("VLC_PLUGIN_PATH").is_none() {
            std::env::set_var("VLC_PLUGIN_PATH", &plugins);
        }

        let api = Api::load(&dir.join(libvlc_file_name()))?;

        let args: Vec<CString> = ["--no-video-title-show", "--no-snapshot-preview", "--quiet"]
            .iter()
            .map(|a| CString::new(*a).unwrap())
            .collect();
        let argv: Vec<*const c_char> = args.iter().map(|a| a.as_ptr()).collect();

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
        // Let mouse/keyboard events reach our window instead of VLC's.
        unsafe {
            (api.video_set_mouse_input)(mp, 0);
            (api.video_set_key_input)(mp, 0);
        }
        Ok(Self { instance, mp, api })
    }

    /// Tell VLC to render video into the given native window.
    #[cfg(windows)]
    pub fn set_hwnd(&self, hwnd: Ptr) {
        unsafe { (self.api.mp_set_hwnd)(self.mp, hwnd) }
    }

    pub fn load(&self, path: &Path) -> Result<(), String> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| "path contains a NUL byte".to_string())?;
        unsafe {
            (self.api.mp_stop)(self.mp);
            let media = (self.api.media_new_path)(self.instance, c_path.as_ptr());
            if media.is_null() {
                return Err(format!("cannot open {}: {}", path.display(), self.api.last_error()));
            }
            (self.api.mp_set_media)(self.mp, media);
            // The player keeps its own reference.
            (self.api.media_release)(media);
        }
        Ok(())
    }

    pub fn play(&self) -> Result<(), String> {
        if unsafe { (self.api.mp_play)(self.mp) } == 0 {
            Ok(())
        } else {
            Err(format!("play failed: {}", self.api.last_error()))
        }
    }

    pub fn pause(&self) {
        unsafe { (self.api.mp_set_pause)(self.mp, 1) }
    }

    pub fn stop(&self) {
        unsafe { (self.api.mp_stop)(self.mp) }
    }

    pub fn state(&self) -> State {
        match unsafe { (self.api.mp_get_state)(self.mp) } {
            1 => State::Opening,
            2 => State::Buffering,
            3 => State::Playing,
            4 => State::Paused,
            5 => State::Stopped,
            6 => State::Ended,
            7 => State::Error,
            _ => State::Idle,
        }
    }

    /// Current position in milliseconds (-1 if unknown).
    pub fn time(&self) -> i64 {
        unsafe { (self.api.mp_get_time)(self.mp) }
    }

    pub fn set_time(&self, ms: i64) {
        unsafe { (self.api.mp_set_time)(self.mp, ms) }
    }

    /// Media length in milliseconds (-1 if unknown).
    pub fn length(&self) -> i64 {
        unsafe { (self.api.mp_get_length)(self.mp) }
    }

    pub fn has_video(&self) -> bool {
        unsafe { (self.api.mp_has_vout)(self.mp) > 0 }
    }

    /// Volume 0..=100 (-1 if no audio output yet).
    pub fn volume(&self) -> i32 {
        unsafe { (self.api.audio_get_volume)(self.mp) }
    }

    pub fn set_volume(&self, v: i32) -> bool {
        unsafe { (self.api.audio_set_volume)(self.mp, v) == 0 }
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
