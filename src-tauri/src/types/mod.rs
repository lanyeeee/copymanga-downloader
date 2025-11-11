mod chapter_info;
mod comic;
mod comic_info;
mod download_format;
mod get_favorite_result;
mod log_level;
mod search_result;

pub use chapter_info::*;
pub use comic::*;
pub use comic_info::*;
pub use download_format::*;
pub use get_favorite_result::*;
pub use log_level::*;
pub use search_result::*;

pub type AsyncRwLock<T> = tokio::sync::RwLock<T>;
