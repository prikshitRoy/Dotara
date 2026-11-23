//! Dotara WebAssembly UI entry point.

mod api;
mod app;
mod components;
mod controller;
mod formatting;
mod models;
mod shortcuts;
mod tauri;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
