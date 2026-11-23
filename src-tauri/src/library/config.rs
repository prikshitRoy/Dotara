//! Persistent storage for the user's configured media library folders.

use super::models::{ALL_LIBRARIES_KEY, DEFAULT_MEDIA_FOLDER};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const APP_CONFIG_DIR_NAME: &str = "Dotara";
const LIBRARIES_CONFIG_FILE: &str = "libraries.json";
const LEGACY_FOLDER_CONFIG_FILE: &str = "library_folder.txt";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedLibraries {
    folders: Vec<String>,
    active_folder: Option<String>,
}

/// Load saved library folders and active selection, defaulting to `E:\Songs` if present.
pub fn initial_libraries() -> (Vec<PathBuf>, Option<String>) {
    let Some(dir) = config_dir() else {
        return fallback_default_libraries(Path::new(DEFAULT_MEDIA_FOLDER));
    };
    load_libraries_from_dir(&dir, Path::new(DEFAULT_MEDIA_FOLDER))
}

/// Persist the current library folders and active selection to `libraries.json`.
pub fn save_libraries(folders: &[PathBuf], active_folder: Option<&str>) {
    if let Some(dir) = config_dir() {
        save_libraries_to_dir(&dir, folders, active_folder);
    }
}

fn config_dir() -> Option<PathBuf> {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(local).join(APP_CONFIG_DIR_NAME);
        let _ = fs::create_dir_all(&dir);
        return Some(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn load_libraries_from_dir(
    dir: &Path,
    default_folder: &Path,
) -> (Vec<PathBuf>, Option<String>) {
    let config_path = dir.join(LIBRARIES_CONFIG_FILE);
    if let Ok(raw) = fs::read_to_string(&config_path) {
        if let Ok(saved) = serde_json::from_str::<SavedLibraries>(&raw) {
            let folders: Vec<PathBuf> = saved
                .folders
                .into_iter()
                .map(PathBuf::from)
                .filter(|p| p.is_dir())
                .collect();
            if !folders.is_empty() {
                let active = resolve_active_folder(&folders, saved.active_folder);
                return (folders, active);
            }
        }
    }

    // Migrate the single-folder text config from earlier builds when present.
    let mut folders = read_legacy_folder(dir);
    if default_folder.is_dir() && !folders.iter().any(|p| p == default_folder) {
        folders.push(default_folder.to_path_buf());
    }

    let active = folders.first().map(|f| f.to_string_lossy().into_owned());
    if !folders.is_empty() {
        save_libraries_to_dir(dir, &folders, active.as_deref());
    }
    (folders, active)
}

fn save_libraries_to_dir(dir: &Path, folders: &[PathBuf], active_folder: Option<&str>) {
    let payload = SavedLibraries {
        folders: folders
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        active_folder: active_folder.map(str::to_owned),
    };
    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(dir.join(LIBRARIES_CONFIG_FILE), json);
    }
}

fn read_legacy_folder(dir: &Path) -> Vec<PathBuf> {
    let Ok(saved) = fs::read_to_string(dir.join(LEGACY_FOLDER_CONFIG_FILE)) else {
        return Vec::new();
    };
    let trimmed = saved.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let path = PathBuf::from(trimmed);
    if path.is_dir() {
        vec![path]
    } else {
        Vec::new()
    }
}

fn resolve_active_folder(folders: &[PathBuf], saved_active: Option<String>) -> Option<String> {
    saved_active
        .filter(|active| {
            active == ALL_LIBRARIES_KEY
                || folders.iter().any(|f| f.to_string_lossy() == active.as_str())
        })
        .or_else(|| folders.first().map(|f| f.to_string_lossy().into_owned()))
}

fn fallback_default_libraries(default_folder: &Path) -> (Vec<PathBuf>, Option<String>) {
    if default_folder.is_dir() {
        let active = Some(default_folder.to_string_lossy().into_owned());
        (vec![default_folder.to_path_buf()], active)
    } else {
        (Vec::new(), None)
    }
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
        let dir = std::env::temp_dir().join(format!("dotara_cfg_{label}_{nonce}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saves_and_reloads_multiple_libraries_with_active_selection() {
        let root = make_temp_dir("roundtrip");
        let cfg_dir = root.join("cfg");
        let lib_a = root.join("MusicA");
        let lib_b = root.join("MusicB");
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::create_dir_all(&lib_a).unwrap();
        fs::create_dir_all(&lib_b).unwrap();

        save_libraries_to_dir(&cfg_dir, &[lib_a.clone(), lib_b.clone()], Some(ALL_LIBRARIES_KEY));
        let (loaded, active) = load_libraries_from_dir(&cfg_dir, &root.join("missing"));

        assert_eq!(loaded, vec![lib_a, lib_b]);
        assert_eq!(active, Some(ALL_LIBRARIES_KEY.to_string()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn migrates_legacy_single_folder_config_file() {
        let root = make_temp_dir("legacy");
        let cfg_dir = root.join("cfg");
        let lib_a = root.join("OldSongs");
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::create_dir_all(&lib_a).unwrap();
        fs::write(
            cfg_dir.join(LEGACY_FOLDER_CONFIG_FILE),
            lib_a.to_string_lossy().as_bytes(),
        )
        .unwrap();

        let (loaded, active) = load_libraries_from_dir(&cfg_dir, &root.join("missing"));

        assert_eq!(loaded, vec![lib_a.clone()]);
        assert_eq!(active, Some(lib_a.to_string_lossy().into_owned()));
        assert!(cfg_dir.join(LIBRARIES_CONFIG_FILE).is_file());
        let _ = fs::remove_dir_all(&root);
    }
}
