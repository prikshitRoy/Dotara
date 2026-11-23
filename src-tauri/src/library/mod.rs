//! Media library models, folder persistence, scanning, and thumbnail extraction.

mod config;
mod models;
mod scanner;
mod thumbnails;

pub use config::{initial_libraries, save_libraries};
pub use models::{
    display_name_for_path, folder_entry, media_kind, LibraryFolder, LibraryItem, LibraryResponse,
    ALL_LIBRARIES_KEY, MEDIA_EXTENSIONS,
};
pub use scanner::{scan_folder, scan_libraries};
pub use thumbnails::extract_thumbnail;
