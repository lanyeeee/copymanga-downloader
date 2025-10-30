mod comic;
mod comic_info;
mod download_format;

pub use comic::*;
pub use comic_info::*;
pub use download_format::*;

pub type AsyncRwLock<T> = tokio::sync::RwLock<T>;
