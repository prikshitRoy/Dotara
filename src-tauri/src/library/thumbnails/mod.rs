//! Multi-source thumbnail extraction with an in-memory cache.
//!
//! Lookup order: OS Shell thumbnail -> sidecar image -> embedded tag art -> SVG icon.

mod embedded;
mod sidecar;
#[cfg(windows)]
mod win_shell;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Requested square dimension (in pixels) for OS shell thumbnails.
#[cfg(windows)]
const SHELL_THUMBNAIL_SIZE_PX: i32 = 128;

fn thumb_cache() -> &'static Mutex<HashMap<PathBuf, String>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Extract a thumbnail data URL (`data:image/...;base64,...`) for `path`, cached in memory.
pub fn extract_thumbnail(path: &Path, kind: &str) -> String {
    if let Some(cached) = thumb_cache().lock().unwrap().get(path).cloned() {
        return cached;
    }
    let thumbnail = extract_thumbnail_uncached(path, kind);
    thumb_cache()
        .lock()
        .unwrap()
        .insert(path.to_path_buf(), thumbnail.clone());
    thumbnail
}

fn extract_thumbnail_uncached(path: &Path, kind: &str) -> String {
    // 1. OS Shell thumbnail (video frames for .webm/.mp4/.mkv and cover art for .mp3/.m4a).
    #[cfg(windows)]
    if let Some(data_url) =
        win_shell::shell_thumbnail_png(path, SHELL_THUMBNAIL_SIZE_PX, SHELL_THUMBNAIL_SIZE_PX)
    {
        return data_url;
    }

    // 2. Sidecar image file next to the media file (<stem>.jpg/.png/.webp).
    if let Some(data_url) = sidecar::sidecar_thumbnail(path) {
        return data_url;
    }

    // 3. Embedded JPEG or PNG bytes inside the media file header.
    if let Some(data_url) = embedded::embedded_tag_thumbnail(path) {
        return data_url;
    }

    // 4. Crisp SVG icon when no artwork exists.
    fallback_svg_data_url(kind)
}

fn fallback_svg_data_url(kind: &str) -> String {
    let svg = if kind == "video" {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120"><rect width="120" height="120" rx="14" fill="#1e2230"/><rect x="14" y="26" width="92" height="68" rx="8" fill="#2b3146" stroke="#ff8a1f" stroke-width="3"/><polygon points="50,44 50,76 78,60" fill="#ff8a1f"/></svg>"##
    } else {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120"><rect width="120" height="120" rx="14" fill="#1e2230"/><circle cx="60" cy="60" r="38" fill="#2b3146" stroke="#ff8a1f" stroke-width="2.5"/><path d="M52 74V42l26-6v32" fill="none" stroke="#ff8a1f" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/><circle cx="46" cy="74" r="7" fill="#ff8a1f"/><circle cx="72" cy="68" r="7" fill="#ff8a1f"/></svg>"##
    };
    format!("data:image/svg+xml;base64,{}", BASE64.encode(svg.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_svg_differs_between_video_and_audio() {
        let video = fallback_svg_data_url("video");
        let audio = fallback_svg_data_url("audio");

        assert!(video.starts_with("data:image/svg+xml;base64,"));
        assert!(audio.starts_with("data:image/svg+xml;base64,"));
        assert_ne!(video, audio);
    }
}
