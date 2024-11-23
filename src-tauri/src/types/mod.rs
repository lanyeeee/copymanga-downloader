mod comic;

pub use comic::*;

pub type AsyncRwLock<T> = tokio::sync::RwLock<T>;
