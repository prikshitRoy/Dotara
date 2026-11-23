//! Directory scanning for audio and video media files.

use super::models::{display_name_for_path, media_kind, LibraryItem, ALL_LIBRARIES_KEY};
use super::thumbnails::extract_thumbnail;
use std::fs;
use std::path::{Path, PathBuf};

/// Scan one library folder, or all folders combined when `active_folder` is `"*"`.
pub fn scan_libraries(folders: &[PathBuf], active_folder: Option<&str>) -> Vec<LibraryItem> {
    match active_folder {
        Some(ALL_LIBRARIES_KEY) => folders
            .iter()
            .filter(|folder| folder.is_dir())
            .flat_map(|folder| scan_folder(folder))
            .collect(),
        Some(target) => folders
            .iter()
            .find(|folder| folder.to_string_lossy() == target && folder.is_dir())
            .map(|folder| scan_folder(folder))
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Scan a single directory for supported media files, sorted case-insensitively by filename.
pub fn scan_folder(folder: &Path) -> Vec<LibraryItem> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let folder_str = folder.to_string_lossy().into_owned();

    let mut media_paths: Vec<(PathBuf, &'static str)> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file())
        .filter_map(|path| media_kind(&path).map(|kind| (path, kind)))
        .collect();

    media_paths.sort_by(|(a, _), (b, _)| {
        let name_a = a.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let name_b = b.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        name_a.cmp(&name_b)
    });

    media_paths
        .into_iter()
        .map(|(path, kind)| {
            let name = display_name_for_path(&path);
            let thumbnail = extract_thumbnail(&path, kind);
            LibraryItem {
                path: path.to_string_lossy().into_owned(),
                name,
                folder: folder_str.clone(),
                kind,
                thumbnail,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("dotara_scan_{label}_{nonce}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scans_only_media_files_sorted_case_insensitively() {
        let dir = make_temp_dir("sort");
        fs::write(dir.join("zebra.mp3"), b"").unwrap();
        fs::write(dir.join("Alpha.webm"), b"").unwrap();
        fs::write(dir.join("notes.txt"), b"ignore me").unwrap();

        let items = scan_folder(&dir);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Alpha");
        assert_eq!(items[0].kind, "video");
        assert_eq!(items[1].name, "zebra");
        assert_eq!(items[1].kind, "audio");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_libraries_combines_all_folders_when_all_key_is_active() {
        let root = make_temp_dir("multi");
        let dir_a = root.join("A");
        let dir_b = root.join("B");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(dir_a.join("one.mp3"), b"").unwrap();
        fs::write(dir_b.join("two.mp4"), b"").unwrap();

        let items = scan_libraries(&[dir_a, dir_b], Some(ALL_LIBRARIES_KEY));

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "one");
        assert_eq!(items[1].name, "two");
        let _ = fs::remove_dir_all(&root);
    }
}
