//! Media folder scanning and audio/video thumbnail extraction.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Serialize;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", "ts", "mpg", "mpeg",
];

pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "opus", "m4a", "aac", "wma", "alac",
];

pub const DEFAULT_MEDIA_FOLDER: &str = r"E:\Songs";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub path: String,
    pub name: String,
    /// `"video"` or `"audio"`
    pub kind: &'static str,
    /// Data URL (`data:image/...;base64,...`)
    pub thumbnail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryResponse {
    pub folder: Option<String>,
    pub items: Vec<LibraryItem>,
}

pub fn media_kind(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some("video")
    } else if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some("audio")
    } else {
        None
    }
}

/// Config file storing the user's chosen library folder.
fn config_file_path() -> Option<PathBuf> {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(local).join("Dotara");
        let _ = fs::create_dir_all(&dir);
        return Some(dir.join("library_folder.txt"));
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("library_folder.txt")))
}

/// Load the saved library folder, or default to `E:\Songs` if it exists.
pub fn initial_folder() -> Option<PathBuf> {
    if let Some(cfg) = config_file_path() {
        if let Ok(saved) = fs::read_to_string(cfg) {
            let trimmed = saved.trim();
            if !trimmed.is_empty() {
                let p = PathBuf::from(trimmed);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
    }
    let default = PathBuf::from(DEFAULT_MEDIA_FOLDER);
    if default.is_dir() {
        Some(default)
    } else {
        None
    }
}

pub fn save_folder(path: &Path) {
    if let Some(cfg) = config_file_path() {
        let _ = fs::write(cfg, path.to_string_lossy().as_bytes());
    }
}

/// Scan all audio and video files in `folder` and extract thumbnails.
pub fn scan_folder(folder: &Path) -> Vec<LibraryItem> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut media_paths: Vec<(PathBuf, &'static str)> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .filter_map(|p| media_kind(&p).map(|k| (p, k)))
        .collect();

    media_paths.sort_by(|(a, _), (b, _)| {
        let na = a.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let nb = b.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        na.cmp(&nb)
    });

    media_paths
        .into_iter()
        .map(|(path, kind)| {
            let name = path
                .file_stem()
                .or_else(|| path.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let thumbnail = extract_thumbnail(&path, kind);
            LibraryItem {
                path: path.to_string_lossy().into_owned(),
                name,
                kind,
                thumbnail,
            }
        })
        .collect()
}

/// Extract a thumbnail data URL for a media file.
pub fn extract_thumbnail(path: &Path, kind: &str) -> String {
    // 1. OS Shell thumbnail (extracts video frames for .webm/.mp4/.mkv and cover art for .mp3/.m4a).
    #[cfg(windows)]
    if let Some(data_url) = win_shell::shell_thumbnail_png(path, 128, 128) {
        return data_url;
    }

    // 2. Sidecar image next to the media file (<stem>.jpg / .png / .webp).
    if let Some(data_url) = sidecar_thumbnail(path) {
        return data_url;
    }

    // 3. Embedded JPEG or PNG inside the media file's tag header.
    if let Some(data_url) = embedded_tag_thumbnail(path) {
        return data_url;
    }

    // 4. Clean SVG fallback icon for Audio vs Video.
    fallback_svg_data_url(kind)
}

fn sidecar_thumbnail(media_path: &Path) -> Option<String> {
    let parent = media_path.parent()?;
    let stem = media_path.file_stem()?.to_str()?;

    for ext in ["jpg", "jpeg", "png", "webp"] {
        let candidate = parent.join(format!("{stem}.{ext}"));
        if candidate.is_file() {
            return file_to_data_url(&candidate, ext);
        }
    }

    // Also check if another image in the folder shares the same prefix (e.g. cover variants).
    let stem_lower = stem.trim().to_lowercase();
    if stem_lower.len() >= 6 {
        if let Ok(entries) = fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                let Some(ext) = p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()) else {
                    continue;
                };
                if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
                    continue;
                }
                if let Some(img_stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let img_lower = img_stem.trim().to_lowercase();
                    if img_lower.starts_with(&stem_lower) || stem_lower.starts_with(&img_lower) {
                        if let Some(url) = file_to_data_url(&p, &ext) {
                            return Some(url);
                        }
                    }
                }
            }
        }
    }
    None
}

fn file_to_data_url(path: &Path, ext: &str) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    let mime = match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    Some(format!("data:{mime};base64,{}", BASE64.encode(&bytes)))
}

/// Scan the first 1.5 MB of a media file for an embedded JPEG or PNG cover image.
fn embedded_tag_thumbnail(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; 1536 * 1024];
    let mut n = 0;
    while n < buf.len() {
        match file.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(read) => n += read,
            Err(_) => break,
        }
    }
    let data = &buf[..n];

    // Look for JPEG SOI (\xFF\xD8\xFF\xE0 or \xFF\xD8\xFF\xE1 or \xFF\xD8\xFF\xDB)
    if let Some(start) = (0..data.len().saturating_sub(16)).find(|&i| {
        data[i] == 0xFF
            && data[i + 1] == 0xD8
            && data[i + 2] == 0xFF
            && matches!(data[i + 3], 0xE0 | 0xE1 | 0xDB | 0xEE)
    }) {
        // Find JPEG EOI (\xFF\xD9) after at least 512 bytes
        let search_from = (start + 512).min(data.len().saturating_sub(2));
        if let Some(rel_end) = data[search_from..]
            .windows(2)
            .position(|w| w == [0xFF, 0xD9])
        {
            let end = search_from + rel_end + 2;
            return Some(format!(
                "data:image/jpeg;base64,{}",
                BASE64.encode(&data[start..end])
            ));
        }
    }

    // Look for PNG signature (\x89PNG\r\n\x1A\n) .. IEND
    const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    const PNG_END: &[u8] = b"IEND\xae\x42\x60\x82";
    if let Some(start) = data.windows(PNG_SIG.len()).position(|w| w == PNG_SIG) {
        if let Some(rel_end) = data[start..].windows(PNG_END.len()).position(|w| w == PNG_END) {
            let end = start + rel_end + PNG_END.len();
            return Some(format!(
                "data:image/png;base64,{}",
                BASE64.encode(&data[start..end])
            ));
        }
    }

    None
}

fn fallback_svg_data_url(kind: &str) -> String {
    let svg = if kind == "video" {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120"><rect width="120" height="120" rx="14" fill="#1e2230"/><rect x="14" y="26" width="92" height="68" rx="8" fill="#2b3146" stroke="#ff8a1f" stroke-width="3"/><polygon points="50,44 50,76 78,60" fill="#ff8a1f"/></svg>"##
    } else {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120"><rect width="120" height="120" rx="14" fill="#1e2230"/><circle cx="60" cy="60" r="38" fill="#2b3146" stroke="#ff8a1f" stroke-width="2.5"/><path d="M52 74V42l26-6v32" fill="none" stroke="#ff8a1f" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/><circle cx="46" cy="74" r="7" fill="#ff8a1f"/><circle cx="72" cy="68" r="7" fill="#ff8a1f"/></svg>"##
    };
    format!("data:image/svg+xml;base64,{}", BASE64.encode(svg.as_bytes()))
}

#[cfg(windows)]
mod win_shell {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::path::Path;
    use std::ptr::null_mut;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };

    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    const IID_ISHELL_ITEM_IMAGE_FACTORY: Guid = Guid {
        data1: 0xbcc18b79,
        data2: 0xba16,
        data3: 0x442f,
        data4: [0x80, 0xc4, 0x8a, 0x59, 0xc3, 0x0c, 0x46, 0x3b],
    };

    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const SIIGBF_THUMBNAILONLY: i32 = 0x08;

    #[repr(C)]
    struct IShellItemImageFactoryVtbl {
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        // On x86_64 Windows ABI, SIZE { cx: i32, cy: i32 } is passed in a single 64-bit register.
        get_image: unsafe extern "system" fn(*mut c_void, u64, i32, *mut HBITMAP) -> i32,
    }

    #[repr(C)]
    struct IShellItemImageFactory {
        vtbl: *const IShellItemImageFactoryVtbl,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHCreateItemFromParsingName(
            psz_path: *const u16,
            pbc: *mut c_void,
            riid: *const Guid,
            ppv: *mut *mut c_void,
        ) -> i32;
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(pv_reserved: *mut c_void, dw_co_init: u32) -> i32;
    }

    pub fn shell_thumbnail_png(path: &Path, width: i32, height: i32) -> Option<String> {
        unsafe {
            let _ = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED);

            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut factory_ptr: *mut c_void = null_mut();
            let hr = SHCreateItemFromParsingName(
                wide.as_ptr(),
                null_mut(),
                &IID_ISHELL_ITEM_ITEM_FACTORY_REF,
                &mut factory_ptr,
            );
            if hr != 0 || factory_ptr.is_null() {
                return None;
            }

            let factory = factory_ptr as *mut IShellItemImageFactory;
            let size_packed = (width as u32 as u64) | ((height as u32 as u64) << 32);
            let mut hbm: HBITMAP = null_mut();
            let hr = ((*(*factory).vtbl).get_image)(
                factory_ptr,
                size_packed,
                SIIGBF_THUMBNAILONLY,
                &mut hbm,
            );
            ((*(*factory).vtbl).release)(factory_ptr);

            if hr != 0 || hbm.is_null() {
                return None;
            }

            let png_bytes = hbitmap_to_png(hbm);
            DeleteObject(hbm);
            let bytes = png_bytes?;
            Some(format!("data:image/png;base64,{}", BASE64.encode(&bytes)))
        }
    }

    const IID_ISHELL_ITEM_ITEM_FACTORY_REF: Guid = IID_ISHELL_ITEM_IMAGE_FACTORY;

    use std::os::windows::ffi::OsStrExt;

    unsafe fn hbitmap_to_png(hbm: HBITMAP) -> Option<Vec<u8>> {
        let mut bm: BITMAP = zeroed();
        if GetObjectW(hbm, size_of::<BITMAP>() as i32, &mut bm as *mut _ as *mut c_void) == 0 {
            return None;
        }
        let w = bm.bmWidth;
        let h = bm.bmHeight.abs();
        if w <= 0 || h <= 0 {
            return None;
        }

        let hdc = CreateCompatibleDC(null_mut());
        if hdc.is_null() {
            return None;
        }

        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h; // top-down DIB
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
        let lines = GetDIBits(
            hdc,
            hbm,
            0,
            h as u32,
            pixels.as_mut_ptr() as *mut c_void,
            &mut bmi,
            DIB_RGB_COLORS,
        );
        DeleteDC(hdc);
        if lines == 0 {
            return None;
        }

        // Convert BGRA -> RGBA (opaque)
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }

        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&pixels).ok()?;
        }
        Some(out)
    }
}
