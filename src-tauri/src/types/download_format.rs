use image::ImageFormat;
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

    pub fn to_image_format(self) -> ImageFormat {
        match self {
            DownloadFormat::Webp => ImageFormat::WebP,
            DownloadFormat::Jpeg => ImageFormat::Jpeg,
        }
    }
}
