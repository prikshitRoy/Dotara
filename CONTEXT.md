# CONTEXT.md — Dotara project handoff

> Machine-readable summary for another AI/dev picking up this project. Last updated: 2026-11-12.

## 1. What this is
**Dotara**: Windows desktop media player (audio + video). Playback engine = **libVLC 3.x**. App shell = **Tauri 2**. UI = **Leptos (Rust → WebAssembly)**, *no JavaScript framework* (user requirement).

- Repo root: `E:\Software Projects\Music Player\Dotara`
- Branch: `main`
- OS/shell: Windows, PowerShell. Toolchain: Rust 1.99 (MSVC), `wasm32-unknown-unknown`, Trunk 0.22, tauri-cli 2.12.
- User's media folder: `E:\Songs` (mixed audio + video; default library on first run).

## 2. Architecture (decisions + rationale)
| Area | Decision | Why |
|---|---|---|
| UI | Leptos 0.8 CSR, built by Trunk (`index.html`, `styles.css`, `src/`) | User wants Rust UI, no JS framework |
| Shell | Tauri 2 (`src-tauri/`), uses WebView2 on Windows | Tauri has no non-webview renderer; WebView2 hosts the WASM + CSS. `msedgewebview2.exe` processes are expected |
| VLC binding | Hand-written, runtime-loaded via `libloading` (`src-tauri/src/vlc.rs`) | No VLC SDK/import libs needed at build time |
| Video output | Win32 child window (`surface.rs`) that VLC renders into; UI reports `.video-area` bounds (physical px) via `set_video_bounds` | WebView2 can't show libVLC frames. Consequence: HTML cannot overlay video; controls sit below it |
| Native surface visibility | Shown only when VLC has a vout (`status` polling) | Audio files show cover art placeholder instead |
| VLC distribution | `scripts/prepare-vlc.ps1` copies libvlc/libvlccore + trimmed `plugins/` + regenerated `plugins.dat` into `src-tauri/vlc/` (gitignored); bundled via `tauri.conf.json` `bundle.resources` | Self-contained installer; app only looks next to the exe (`<exe>/vlc`, `<exe>`, then `src-tauri/vlc` for dev). **No** system-VLC or env-var lookup |
| Installer | NSIS only (`targets: ["nsis"]`) → `target/release/bundle/nsis/Dotara_0.1.0_x64-setup.exe` (~30 MB) | User asked for a single .exe installer |
| IPC | Tauri commands (async) + UI polls `status` every 250 ms | Simple; also drives auto-advance |
| Thumbnails | `library.rs`: (1) Windows Shell `IShellItemImageFactory` via raw COM vtable, (2) sidecar image w/ same stem, (3) embedded JPEG/PNG scan of first 1.5 MB, (4) SVG fallback. Returned as data URLs, cached in memory | Works for video frames + mp3/m4a cover art |
| Persistence | `%LOCALAPPDATA%\Dotara\libraries.json` (`folders[]`, `activeFolder`); migrates legacy `library_folder.txt`; defaults to `E:\Songs` | Multiple libraries survive restarts |

### Key files
- `src/app.rs` — entire Leptos UI (side menu, library chips, list, controls). `src/tauri.rs` — typed `invoke` wrapper.
- `src-tauri/src/main.rs` — AppState, all `#[tauri::command]`s, play-mode/queue logic, auto-advance inside `status`.
- `src-tauri/src/library.rs` — scanning, persistence, thumbnails. `vlc.rs` — libVLC FFI. `surface.rs` — Win32 child window.
- `src-tauri/examples/vlc_smoke.rs` — headless test: scans library + plays first file.
- `scripts/prepare-vlc.ps1` — VLC staging (runs in `beforeBuild/DevCommand`).

## 3. Features completed (all committed)
1. Basic play/pause/stop/seek/volume with libVLC; working video + audio.
2. Docked (non-popup) **Library side menu**, toggled by `☰ Library`.
3. **Multiple media libraries**: add (`+ Add Library…`), switch via chips, `All` combined view, remove (`×`).
4. Scrollable list with audio/video thumbnails + AUDIO/VIDEO badge; select + play; per-row checkbox for multi-select.
5. **Play All** and **Play Selected** buttons; **play-mode dropdown**: `continue_next` (DEFAULT), `play_all` (loops), `selected`, `one`. Prev/Next buttons. Auto-advance when VLC state = Ended (handled in `status` command).
6. Self-contained NSIS installer; `testvideo.webm` removed from project/bundle.

Commit history (newest first): `f3310b4` playback modes/Play All · `abef6c0` multiple libraries · `39c392a` side menu + thumbnails · `9287b25` bundle VLC/installer · `5a0de20` README · UI/core commits before that.

## 4. Known limitations / caveats
- Windows-only video embedding (`surface.rs` is a stub on other OSes).
- **Not yet verified by a human run**: Play All/modes/prev-next/checkbox behavior (compiled + built, but only library/thumbnail scan and basic playback were exercised). First thing to do next: launch `cargo tauri dev` and test them.
- `Selected` mode with a single item selected plays only that item; `play_file` rebuilds queue from folder unless mode is `selected`.
- Library scan + thumbnail extraction runs synchronously inside the command on first load (can be slow for big folders; thumbnails cached in-memory only, not on disk).
- Whole-library data URLs are sent to the UI in one payload (heavy for large libraries) — consider lazy/paged thumbnails.
- `.smi` and unsupported files ignored; some mp3/flac/opus have no art → SVG fallback.
- Build quirk: PowerShell reports exit code 1 for cargo/trunk when stderr has progress output even on success; check for "Finished".
- Tool sandbox in this environment sometimes needs commands run outside sandbox.

## 5. Immediate next steps (suggested)
1. Manually test the new play modes, prev/next, multi-select and auto-advance with `E:\Songs`; fix issues.
2. Show a "now playing" highlight/queue indicator and shuffle/repeat-one if desired.
3. Persist volume and play mode; remember last played track.
4. Performance: async/background thumbnail loading + on-disk cache; virtualize long lists.
5. User was exploring **removing WebView2**: conclusion — Tauri cannot drop its webview; options are Tauri with Verso/CEF runtime (still a web engine) or migrating UI to a native Rust GUI (egui/Iced/Slint) while reusing `vlc.rs`, `library.rs`, `surface.rs`. No decision made yet; ask the user before starting.
6. Optional: code signing, app icon polish, auto-update.

## 6. How to run / build
```powershell
cargo tauri dev        # dev (runs prepare-vlc.ps1 + trunk serve)
cargo tauri build      # release + NSIS installer
cargo run -p dotara --example vlc_smoke   # headless library/VLC check
```
