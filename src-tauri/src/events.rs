use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    ChapterPending {
        chapter_uuid: String,
        comic_title: String,
        chapter_title: String,
    },

    #[serde(rename_all = "camelCase")]
    ChapterControlRisk {
        chapter_uuid: String,
        retry_after: u32,
    },

    #[serde(rename_all = "camelCase")]
    ChapterStart { chapter_uuid: String, total: u32 },

    #[serde(rename_all = "camelCase")]
    ChapterEnd {
        chapter_uuid: String,
        err_msg: Option<String>,
    },

    #[serde(rename_all = "camelCase")]
    ImageSuccess {
        chapter_uuid: String,
        url: String,
        current: u32,
    },

    #[serde(rename_all = "camelCase")]
    ImageError {
        chapter_uuid: String,
        url: String,
        err_msg: String,
    },

    #[serde(rename_all = "camelCase")]
    OverallUpdate {
        downloaded_image_count: u32,
        total_image_count: u32,
        percentage: f64,
    },

    #[serde(rename_all = "camelCase")]
    OverallSpeed { speed: String },
}