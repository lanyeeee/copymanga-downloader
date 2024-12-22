mod comic;
mod comic_info;

pub use comic::*;
pub use comic_info::*;

pub type AsyncRwLock<T> = tokio::sync::RwLock<T>;
