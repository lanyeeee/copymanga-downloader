use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum DownloadFormat {
    Webp,
    Jpeg,
}

impl DownloadFormat {
    pub fn extension(self) -> &'static str {
        match self {
            DownloadFormat::Webp => "webp",
            DownloadFormat::Jpeg => "jpg",
        }
    }
}
