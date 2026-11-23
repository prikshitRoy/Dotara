//! Embedded JPEG/PNG artwork scanner for audio tag headers (ID3v2, Vorbis, MP4).

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Maximum leading bytes scanned for embedded tag artwork (1.5 MiB).
const EMBEDDED_ART_SCAN_BYTES: usize = 1536 * 1024;
/// Minimum plausible JPEG payload length before searching for the EOI marker.
const MIN_JPEG_BODY_BYTES: usize = 512;

const JPEG_SOI: [u8; 3] = [0xFF, 0xD8, 0xFF];
const JPEG_EOI: [u8; 2] = [0xFF, 0xD9];
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const PNG_IEND_CHUNK: &[u8] = b"IEND\xae\x42\x60\x82";

/// Scan the leading header of `path` for an embedded JPEG or PNG cover image.
pub(super) fn embedded_tag_thumbnail(path: &Path) -> Option<String> {
    let data = read_header_prefix(path, EMBEDDED_ART_SCAN_BYTES)?;
    extract_embedded_image_data_url(&data)
}

fn read_header_prefix(path: &Path, max_bytes: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; max_bytes];
    let mut total_read = 0;
    while total_read < buf.len() {
        match file.read(&mut buf[total_read..]) {
            Ok(0) => break,
            Ok(n) => total_read += n,
            Err(_) => break,
        }
    }
    buf.truncate(total_read);
    Some(buf)
}

/// Pure byte scanner that locates an embedded JPEG or PNG inside raw tag bytes.
fn extract_embedded_image_data_url(data: &[u8]) -> Option<String> {
    if let Some(jpeg) = find_jpeg_slice(data) {
        return Some(format!("data:image/jpeg;base64,{}", BASE64.encode(jpeg)));
    }
    if let Some(png) = find_png_slice(data) {
        return Some(format!("data:image/png;base64,{}", BASE64.encode(png)));
    }
    None
}

fn find_jpeg_slice(data: &[u8]) -> Option<&[u8]> {
    let start = (0..data.len().saturating_sub(16)).find(|&i| {
        data[i..i + 3] == JPEG_SOI && matches!(data[i + 3], 0xE0 | 0xE1 | 0xDB | 0xEE)
    })?;
    let search_from = (start + MIN_JPEG_BODY_BYTES).min(data.len().saturating_sub(2));
    let rel_end = data[search_from..]
        .windows(JPEG_EOI.len())
        .position(|w| w == JPEG_EOI)?;
    let end = search_from + rel_end + JPEG_EOI.len();
    Some(&data[start..end])
}

fn find_png_slice(data: &[u8]) -> Option<&[u8]> {
    let start = data
        .windows(PNG_SIGNATURE.len())
        .position(|w| w == PNG_SIGNATURE)?;
    let rel_end = data[start..]
        .windows(PNG_IEND_CHUNK.len())
        .position(|w| w == PNG_IEND_CHUNK)?;
    let end = start + rel_end + PNG_IEND_CHUNK.len();
    Some(&data[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_embedded_jpeg_from_tag_bytes() {
        let mut raw = b"ID3...APIC\0image/jpeg\0".to_vec();
        raw.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0]);
        raw.extend_from_slice(&vec![0x42; MIN_JPEG_BODY_BYTES + 16]);
        raw.extend_from_slice(&JPEG_EOI);
        raw.extend_from_slice(b"trailing audio frames");

        let url = extract_embedded_image_data_url(&raw);

        assert!(url
            .as_deref()
            .is_some_and(|u| u.starts_with("data:image/jpeg;base64,")));
    }

    #[test]
    fn extracts_embedded_png_from_tag_bytes() {
        let mut raw = b"header".to_vec();
        raw.extend_from_slice(PNG_SIGNATURE);
        raw.extend_from_slice(b"IHDR....");
        raw.extend_from_slice(PNG_IEND_CHUNK);

        let url = extract_embedded_image_data_url(&raw);

        assert!(url
            .as_deref()
            .is_some_and(|u| u.starts_with("data:image/png;base64,")));
    }

    #[test]
    fn returns_none_when_no_image_markers_exist() {
        assert_eq!(extract_embedded_image_data_url(b"plain audio bytes"), None);
    }
}
