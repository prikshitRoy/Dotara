//! Dynamic FFI loader and raw symbol table for `libvlc.dll`.

use libloading::Library;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::path::{Path, PathBuf};

/// Opaque pointer returned by libVLC C APIs.
pub(crate) type RawPtr = *mut c_void;

/// `libvlc_state_t` numeric codes from the libVLC 3.x C ABI.
pub(crate) const LIBVLC_STATE_OPENING: c_int = 1;
pub(crate) const LIBVLC_STATE_BUFFERING: c_int = 2;
pub(crate) const LIBVLC_STATE_PLAYING: c_int = 3;
pub(crate) const LIBVLC_STATE_PAUSED: c_int = 4;
pub(crate) const LIBVLC_STATE_STOPPED: c_int = 5;
pub(crate) const LIBVLC_STATE_ENDED: c_int = 6;
pub(crate) const LIBVLC_STATE_ERROR: c_int = 7;

/// CLI flags passed to `libvlc_new` to keep overlays and console noise off.
pub(crate) const VLC_INIT_ARGS: &[&str] = &[
    "--no-video-title-show",
    "--no-snapshot-preview",
    "--quiet",
];

/// Function table resolved at runtime from the bundled libVLC shared library.
pub(crate) struct Api {
    pub(crate) new: unsafe extern "C" fn(c_int, *const *const c_char) -> RawPtr,
    pub(crate) release: unsafe extern "C" fn(RawPtr),
    pub(crate) errmsg: unsafe extern "C" fn() -> *const c_char,
    pub(crate) media_new_path: unsafe extern "C" fn(RawPtr, *const c_char) -> RawPtr,
    pub(crate) media_release: unsafe extern "C" fn(RawPtr),
    pub(crate) mp_new: unsafe extern "C" fn(RawPtr) -> RawPtr,
    pub(crate) mp_release: unsafe extern "C" fn(RawPtr),
    pub(crate) mp_set_media: unsafe extern "C" fn(RawPtr, RawPtr),
    pub(crate) mp_play: unsafe extern "C" fn(RawPtr) -> c_int,
    pub(crate) mp_set_pause: unsafe extern "C" fn(RawPtr, c_int),
    pub(crate) mp_stop: unsafe extern "C" fn(RawPtr),
    pub(crate) mp_get_state: unsafe extern "C" fn(RawPtr) -> c_int,
    pub(crate) mp_get_time: unsafe extern "C" fn(RawPtr) -> i64,
    pub(crate) mp_set_time: unsafe extern "C" fn(RawPtr, i64),
    pub(crate) mp_get_length: unsafe extern "C" fn(RawPtr) -> i64,
    pub(crate) mp_has_vout: unsafe extern "C" fn(RawPtr) -> c_uint,
    #[cfg(windows)]
    pub(crate) mp_set_hwnd: unsafe extern "C" fn(RawPtr, RawPtr),
    pub(crate) video_set_mouse_input: unsafe extern "C" fn(RawPtr, c_uint),
    pub(crate) video_set_key_input: unsafe extern "C" fn(RawPtr, c_uint),
    pub(crate) audio_get_volume: unsafe extern "C" fn(RawPtr) -> c_int,
    pub(crate) audio_set_volume: unsafe extern "C" fn(RawPtr, c_int) -> c_int,
    // Must remain the last field so the DLL unloads only after all calls finish.
    _lib: Library,
}

impl Api {
    /// Load `libvlc` from `dll_path` and resolve all symbols required by `Player`.
    pub(crate) fn load(dll_path: &Path) -> Result<Self, String> {
        let lib = load_shared_library(dll_path)?;
        // SAFETY: Every symbol signature matches the stable libVLC 3.x C ABI,
        // and `_lib` outlives every copied function pointer stored in `Self`.
        unsafe {
            Ok(Self {
                new: resolve_symbol(&lib, "libvlc_new")?,
                release: resolve_symbol(&lib, "libvlc_release")?,
                errmsg: resolve_symbol(&lib, "libvlc_errmsg")?,
                media_new_path: resolve_symbol(&lib, "libvlc_media_new_path")?,
                media_release: resolve_symbol(&lib, "libvlc_media_release")?,
                mp_new: resolve_symbol(&lib, "libvlc_media_player_new")?,
                mp_release: resolve_symbol(&lib, "libvlc_media_player_release")?,
                mp_set_media: resolve_symbol(&lib, "libvlc_media_player_set_media")?,
                mp_play: resolve_symbol(&lib, "libvlc_media_player_play")?,
                mp_set_pause: resolve_symbol(&lib, "libvlc_media_player_set_pause")?,
                mp_stop: resolve_symbol(&lib, "libvlc_media_player_stop")?,
                mp_get_state: resolve_symbol(&lib, "libvlc_media_player_get_state")?,
                mp_get_time: resolve_symbol(&lib, "libvlc_media_player_get_time")?,
                mp_set_time: resolve_symbol(&lib, "libvlc_media_player_set_time")?,
                mp_get_length: resolve_symbol(&lib, "libvlc_media_player_get_length")?,
                mp_has_vout: resolve_symbol(&lib, "libvlc_media_player_has_vout")?,
                #[cfg(windows)]
                mp_set_hwnd: resolve_symbol(&lib, "libvlc_media_player_set_hwnd")?,
                video_set_mouse_input: resolve_symbol(&lib, "libvlc_video_set_mouse_input")?,
                video_set_key_input: resolve_symbol(&lib, "libvlc_video_set_key_input")?,
                audio_get_volume: resolve_symbol(&lib, "libvlc_audio_get_volume")?,
                audio_set_volume: resolve_symbol(&lib, "libvlc_audio_set_volume")?,
                _lib: lib,
            })
        }
    }

    /// Return libVLC's most recent thread-local error message, if any.
    pub(crate) fn last_error(&self) -> String {
        // SAFETY: `libvlc_errmsg` returns either null or a valid NUL-terminated string.
        let ptr = unsafe { (self.errmsg)() };
        if ptr.is_null() {
            return "unknown libvlc error".to_string();
        }
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
    }
}

/// Locate the bundled `vlc` directory containing `libvlc` and `plugins/`.
pub(crate) fn find_vlc_dir() -> Option<PathBuf> {
    let dll = libvlc_file_name();
    let mut candidates = Vec::new();
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        candidates.push(exe_dir.join("vlc"));
        candidates.push(exe_dir);
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("vlc"));
    candidates.into_iter().find(|dir| dir.join(dll).is_file())
}

/// Platform-specific shared library filename for libVLC 3.x.
pub(crate) fn libvlc_file_name() -> &'static str {
    if cfg!(windows) {
        "libvlc.dll"
    } else if cfg!(target_os = "macos") {
        "libvlc.dylib"
    } else {
        "libvlc.so.5"
    }
}

fn load_shared_library(dll_path: &Path) -> Result<Library, String> {
    #[cfg(windows)]
    {
        // Altered search path ensures `libvlc.dll` resolves `libvlccore.dll` from the same folder.
        unsafe {
            libloading::os::windows::Library::load_with_flags(
                dll_path,
                libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        }
        .map(Into::into)
        .map_err(|e| format!("failed to load {}: {e}", dll_path.display()))
    }
    #[cfg(not(windows))]
    {
        unsafe { Library::new(dll_path) }
            .map_err(|e| format!("failed to load {}: {e}", dll_path.display()))
    }
}

unsafe fn resolve_symbol<T: Copy>(lib: &Library, name: &str) -> Result<T, String> {
    lib.get::<T>(name.as_bytes())
        .map(|symbol| *symbol)
        .map_err(|e| format!("libvlc is missing `{name}`: {e}"))
}
