//! Windows Shell thumbnail extraction via `IShellItemImageFactory`.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
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

/// COM interface ID for `IShellItemImageFactory` (`{bcc18b79-ba16-442f-80c4-8a59c30c463b}`).
const IID_ISHELL_ITEM_IMAGE_FACTORY: Guid = Guid {
    data1: 0xbcc18b79,
    data2: 0xba16,
    data3: 0x442f,
    data4: [0x80, 0xc4, 0x8a, 0x59, 0xc3, 0x0c, 0x46, 0x3b],
};

const COINIT_APARTMENTTHREADED: u32 = 0x2;
/// Request only a real thumbnail (video frame or album art), never a generic file-type icon.
const SIIGBF_THUMBNAILONLY: i32 = 0x08;
const BITS_PER_PIXEL_BGRA: u16 = 32;

#[repr(C)]
struct IShellItemImageFactoryVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // On x86_64 Windows ABI, `SIZE { cx: i32, cy: i32 }` is packed into one 64-bit register.
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

/// Ask the Windows Shell for a real thumbnail of `path` and return it as a PNG data URL.
pub(super) fn shell_thumbnail_png(path: &Path, width: i32, height: i32) -> Option<String> {
    // SAFETY: Standard Win32 COM calls; `factory_ptr` and `hbm` are released before returning.
    unsafe {
        let _ = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED);
        let hbm = fetch_shell_hbitmap(path, width, height)?;
        let png_bytes = hbitmap_to_png(hbm);
        DeleteObject(hbm);
        let bytes = png_bytes?;
        Some(format!("data:image/png;base64,{}", BASE64.encode(&bytes)))
    }
}

unsafe fn fetch_shell_hbitmap(path: &Path, width: i32, height: i32) -> Option<HBITMAP> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut factory_ptr: *mut c_void = null_mut();
    let hr = SHCreateItemFromParsingName(
        wide.as_ptr(),
        null_mut(),
        &IID_ISHELL_ITEM_IMAGE_FACTORY,
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
    Some(hbm)
}

unsafe fn hbitmap_to_png(hbm: HBITMAP) -> Option<Vec<u8>> {
    let mut bm: BITMAP = zeroed();
    if GetObjectW(hbm, size_of::<BITMAP>() as i32, &mut bm as *mut _ as *mut c_void) == 0 {
        return None;
    }
    let width = bm.bmWidth;
    let height = bm.bmHeight.abs();
    if width <= 0 || height <= 0 {
        return None;
    }

    let mut pixels = read_bgra_pixels(hbm, width, height)?;
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2); // BGRA -> RGBA
        px[3] = 255;
    }
    encode_rgba_png(&pixels, width as u32, height as u32)
}

unsafe fn read_bgra_pixels(hbm: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
    let hdc = CreateCompatibleDC(null_mut());
    if hdc.is_null() {
        return None;
    }

    let mut bmi: BITMAPINFO = zeroed();
    bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = width;
    bmi.bmiHeader.biHeight = -height; // Negative height selects a top-down DIB.
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = BITS_PER_PIXEL_BGRA;
    bmi.bmiHeader.biCompression = BI_RGB;

    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    let scan_lines = GetDIBits(
        hdc,
        hbm,
        0,
        height as u32,
        pixels.as_mut_ptr() as *mut c_void,
        &mut bmi,
        DIB_RGB_COLORS,
    );
    DeleteDC(hdc);

    if scan_lines == 0 {
        None
    } else {
        Some(pixels)
    }
}

fn encode_rgba_png(pixels: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(pixels).ok()?;
    drop(writer);
    Some(out)
}
