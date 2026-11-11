//! Headless smoke test for the libVLC binding and library scanner:
//! `cargo run -p dotara --example vlc_smoke`
#![allow(dead_code)]

#[path = "../src/library.rs"]
mod library;
#[path = "../src/vlc.rs"]
mod vlc;

use std::{path::Path, thread::sleep, time::Duration};

fn main() {
    let (folders, _) = library::initial_libraries();
    let folder = folders
        .into_iter()
        .next()
        .unwrap_or_else(|| Path::new(r"E:\Songs").to_path_buf());
    println!("Scanning folder: {}", folder.display());
    let items = library::scan_folder(&folder);
    println!("Found {} media items:", items.len());
    for item in &items {
        let thumb_kind = item.thumbnail.split(';').next().unwrap_or("unknown");
        println!(
            "  [{:<5}] {:<24} ({:>6} B) {}",
            item.kind,
            thumb_kind,
            item.thumbnail.len(),
            item.name
        );
    }

    if let Some(path) = std::env::args()
        .nth(1)
        .or_else(|| items.first().map(|i| i.path.clone()))
    {
        let player = vlc::Player::new().expect("VLC init");
        player.load(Path::new(&path)).expect("load");
        player.play().expect("play");
        sleep(Duration::from_secs(2));
        println!(
            "Playback check ({}): state={:?} time={}ms length={}ms vout={}",
            path,
            player.state(),
            player.time(),
            player.length(),
            player.has_video()
        );
        player.stop();
    }
}
