//! Sidecar cover-art lookup next to a media file.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::fs;
use std::path::Path;

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
/// Ignore unusually large image files so IPC payloads stay responsive.
const MAX_SIDECAR_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// Minimum stem length before attempting prefix-based sidecar matching.
const MIN_PREFIX_MATCH_LEN: usize = 6;

/// Find a matching sidecar image in the same folder as `media_path`.
pub(super) fn sidecar_thumbnail(media_path: &Path) -> Option<String> {
    let parent = media_path.parent()?;
    let stem = media_path.file_stem()?.to_str()?;

    if let Some(exact) = find_exact_sidecar(parent, stem) {
        return Some(exact);
    }
    find_prefix_sidecar(parent, stem)
}

fn find_exact_sidecar(parent: &Path, stem: &str) -> Option<String> {
    for ext in IMAGE_EXTENSIONS {
        let candidate = parent.join(format!("{stem}.{ext}"));
        if candidate.is_file() {
            return file_to_data_url(&candidate, ext);
        }
    }
    None
}

fn find_prefix_sidecar(parent: &Path, stem: &str) -> Option<String> {
    let stem_lower = stem.trim().to_lowercase();
    if stem_lower.len() < MIN_PREFIX_MATCH_LEN {
        return None;
    }

    let entries = fs::read_dir(parent).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
        else {
            continue;
        };
        if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let Some(img_stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let img_lower = img_stem.trim().to_lowercase();
        if img_lower.starts_with(&stem_lower) || stem_lower.starts_with(&img_lower) {
            if let Some(url) = file_to_data_url(&path, &ext) {
                return Some(url);
            }
        }
    }
    None
}

fn file_to_data_url(path: &Path, ext: &str) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() || bytes.len() > MAX_SIDECAR_IMAGE_BYTES {
        return None;
    }
    let mime = match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    Some(format!("data:{mime};base64,{}", BASE64.encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("dotara_sidecar_{label}_{nonce}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_exact_matching_sidecar_image() {
        let dir = make_temp_dir("exact");
        let audio = dir.join("Song Title.flac");
        fs::write(&audio, b"audio").unwrap();
        fs::write(dir.join("Song Title.png"), b"\x89PNGdata").unwrap();

        let thumb = sidecar_thumbnail(&audio);

        assert!(thumb
            .as_deref()
            .is_some_and(|u| u.starts_with("data:image/png;base64,")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_empty_sidecar_files() {
        let dir = make_temp_dir("empty");
        let audio = dir.join("EmptyArt.mp3");
        fs::write(&audio, b"audio").unwrap();
        fs::write(dir.join("EmptyArt.jpg"), b"").unwrap();

        assert_eq!(sidecar_thumbnail(&audio), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
