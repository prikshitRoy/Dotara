# CONTEXT.md — Dotara project handoff

> Machine-readable summary for another AI/dev picking up this project. Last updated: 2026-11-24.

## 1. What this is
**Dotara**: Windows desktop media player (audio + video). Playback engine = **libVLC 3.x**. App shell = **Tauri 2**. UI = **Leptos (Rust → WebAssembly)**, *no JavaScript framework* (user requirement).

- Repo root: `E:\Software Projects\Music Player\Dotara`
- Branch: `main`
- OS/shell: Windows, PowerShell. Toolchain: Rust 1.99 (MSVC), `wasm32-unknown-unknown`, Trunk 0.22, tauri-cli 2.12.
- User's media folder: `E:\Songs` (mixed audio + video; default library on first run).

## 2. Architecture (decisions + rationale)
| Area | Decision | Why |
|---|---|---|
| UI | Leptos 0.8 CSR, built by Trunk (`index.html`, `styles/`, `src/`) | User wants Rust UI, no JS framework |
| Shell | Tauri 2 (`src-tauri/`), uses WebView2 on Windows | Tauri has no non-webview renderer; WebView2 hosts the WASM + CSS. `msedgewebview2.exe` processes are expected |
| VLC binding | Hand-written, runtime-loaded via `libloading` (`src-tauri/src/vlc/`) | No VLC SDK/import libs needed at build time |
| Video output | Win32 child window (`surface.rs`) that VLC renders into; UI reports `.video-area` bounds (physical px) via `set_video_bounds` | WebView2 can't show libVLC frames. Consequence: HTML cannot overlay video; controls sit below it |
| Native surface visibility | Shown only when VLC has a vout (`status` polling) | Audio files show cover art placeholder instead |
| Top title bar | Static `"title": ""` in `tauri.conf.json`; backend `status` calls `window.set_title` with active media name (or `""` when idle) | Shows only the logo when idle and active media name at the top while playing |
| VLC distribution | `scripts/prepare-vlc.ps1` copies libvlc/libvlccore + trimmed `plugins/` + regenerated `plugins.dat` into `src-tauri/vlc/` (gitignored); bundled via `tauri.conf.json` `bundle.resources` | Self-contained installer; app only looks next to the exe (`<exe>/vlc`, `<exe>`, then `src-tauri/vlc` for dev). **No** system-VLC or env-var lookup |
| Installer | NSIS only (`targets: ["nsis"]`) → `target/release/bundle/nsis/Dotara_0.1.0_x64-setup.exe` (~30 MB) | User asked for a single .exe installer |
| IPC | Tauri commands (async) + UI polls `status` every 250 ms; optimistic seek/volume guards avoid stale poll flicker | Simple; drives auto-advance and smooth arrow-key shortcuts |
| Thumbnails | `library/thumbnails/`: (1) Windows Shell `IShellItemImageFactory` via raw COM vtable, (2) sidecar image w/ same stem, (3) embedded JPEG/PNG scan of first 1.5 MB, (4) SVG fallback. Returned as data URLs, cached in memory | Works for video frames + mp3/m4a cover art |
| Persistence | `%LOCALAPPDATA%\Dotara\libraries.json` (`folders[]`, `activeFolder`); migrates legacy `library_folder.txt`; defaults to `E:\Songs` | Multiple libraries survive restarts |

### Key files
- `src/app.rs` — root Leptos shell & video-bounds sync. `src/controller.rs` — reactive signals & UI actions. `src/api.rs` — typed Tauri IPC client. `src/models.rs` — UI structs/constants. `src/formatting.rs` — pure formatting/selection/active-track helpers (+ unit tests). `src/shortcuts.rs` — global keyboard shortcuts (`←`/`→` seek ±5s, `↑`/`↓` volume ±5%) (+ unit tests). `src/components/` — `LibraryHeader`, `MediaList`, `VideoStage`, `ControlBar`. `styles/` — `base.css`, `sidebar.css`, `media-list.css`, `player.css`.
- `src-tauri/src/lib.rs` & `main.rs` — Tauri bootstrap and handler wiring.
- `src-tauri/src/state.rs` — shared `AppState`, track loading, and end-of-track auto-advance.
- `src-tauri/src/commands/` — Tauri IPC handlers (`library.rs`, `playback.rs`, `window.rs`).
- `src-tauri/src/playback/` — pure `PlayMode` (`mode.rs`) and `PlaybackQueue` (`queue.rs`) (+ unit tests).
- `src-tauri/src/library/` — `models.rs`, `config.rs`, `scanner.rs`, and `thumbnails/` (`mod.rs`, `sidecar.rs`, `embedded.rs`, `win_shell.rs`) (+ unit tests).
- `src-tauri/src/vlc/` — runtime FFI loader (`api.rs`) and safe `Player` (`player.rs`). `src-tauri/src/surface.rs` — Win32 child window.
- `src-tauri/examples/vlc_smoke.rs` — headless test: scans library + plays first file.
- `scripts/prepare-vlc.ps1` — VLC staging (runs in `beforeBuild/DevCommand`).

## 3. Features completed
1. Basic play/pause/stop/seek/volume with libVLC; working video + audio.
2. Docked (non-popup) **Library side menu**, toggled by `☰ Library`.
3. **Multiple media libraries**: add (`+ Add Library…`), switch via chips, `All` combined view, remove (`×`).
4. Scrollable list with audio/video thumbnails + AUDIO/VIDEO badge; focusable rows (`tabindex="0"`, Enter to play) with border-only selection focus and accent-colored text on the playing track (no bg color change); bottom transport controls always act on the playing media and clear row focus; per-row checkbox for multi-select.
5. **Play All** and **Play Selected** buttons; **play-mode dropdown**: `continue_next` (DEFAULT), `play_all` (loops), `selected`, `one`. Prev/Next buttons. Auto-advance when VLC state = Ended (handled in `status` command).
6. **Keyboard shortcuts**: `ArrowLeft`/`ArrowRight` seek ±5s, `ArrowUp`/`ArrowDown` adjust volume ±5% with optimistic poll guards.
7. **Top window title bar**: displays only the app logo when idle and shows the active media name when playing; `"Vol"` text label removed from the bottom volume slider.
8. Self-contained NSIS installer; `testvideo.webm` removed from project/bundle.
9. Modular codebase split into focused modules with **38 unit tests** (`cargo test --workspace`).

## 4. Known limitations / caveats
- Windows-only video embedding (`surface.rs` is a stub on other OSes).
- `Selected` mode with a single item selected plays only that item; `play_file` rebuilds queue from folder unless mode is `selected`.
- Library scan + thumbnail extraction runs synchronously inside the command on first load (can be slow for big folders; thumbnails cached in-memory only, not on disk).
- Whole-library data URLs are sent to the UI in one payload (heavy for large libraries) — consider lazy/paged thumbnails.
- `.smi` and unsupported files ignored; some mp3/flac/opus have no art → SVG fallback.
- Build quirk: PowerShell reports exit code 1 for cargo/trunk when stderr has progress output even on success; check for "Finished".
- Tool sandbox in this environment sometimes needs commands run outside sandbox.

## 5. Immediate next steps (suggested)
1. Persist volume and play mode across restarts; remember last played track.
2. Performance: async/background thumbnail loading + on-disk cache; virtualize long lists.
3. Optional playback controls: Spacebar play/pause shortcut, mute (`M`), shuffle/repeat-one.
4. User was exploring **removing WebView2**: conclusion — Tauri cannot drop its webview; options are Tauri with Verso/CEF runtime (still a web engine) or migrating UI to a native Rust GUI (egui/Iced/Slint) while reusing `vlc/`, `library/`, `playback/`, `surface.rs`. No decision made yet; ask the user before starting.
5. Rebuild release `.exe` installer (`cargo tauri build`) once current UI refinements are verified.

## 6. How to run / build
```powershell
cargo tauri dev        # dev (runs prepare-vlc.ps1 + trunk serve)
cargo tauri build      # release + NSIS installer
cargo test --workspace # run all backend + frontend unit tests
cargo run -p dotara --example vlc_smoke   # headless library/VLC check
```
