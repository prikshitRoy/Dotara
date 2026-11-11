//! Headless smoke test for the libVLC binding:
//! `cargo run -p dotara --example vlc_smoke -- path/to/media`
#![allow(dead_code)]

#[path = "../src/vlc.rs"]
mod vlc;

use std::{thread::sleep, time::Duration};

fn main() {
    let path = std::env::args().nth(1).expect("usage: vlc_smoke <media file>");
    let player = vlc::Player::new().expect("VLC init");
    player.load(std::path::Path::new(&path)).expect("load");
    player.play().expect("play");
    sleep(Duration::from_secs(3));
    println!("after play : state={:?} time={}ms length={}ms vout={} vol={}",
        player.state(), player.time(), player.length(), player.has_video(), player.volume());
    player.pause();
    sleep(Duration::from_millis(500));
    let t = player.time();
    sleep(Duration::from_secs(1));
    println!("after pause: state={:?} time={}ms (still {}ms after 1s => frozen={})",
        player.state(), t, player.time(), t == player.time());
    player.set_time(10_000);
    player.play().expect("resume");
    sleep(Duration::from_secs(1));
    println!("after seek+resume: state={:?} time={}ms", player.state(), player.time());
    player.stop();
    println!("after stop : state={:?}", player.state());
}
