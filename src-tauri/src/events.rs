use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::types::LogLevel;

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

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum ExportCbzEvent {
    #[serde(rename_all = "camelCase")]
    Start {
        uuid: String,
        comic_title: String,
        total: u32,
    },

    #[serde(rename_all = "camelCase")]
    Progress { uuid: String, current: u32 },

    #[serde(rename_all = "camelCase")]
    Error { uuid: String },

    #[serde(rename_all = "camelCase")]
    End { uuid: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum ExportPdfEvent {
    #[serde(rename_all = "camelCase")]
    CreateStart {
        uuid: String,
        comic_title: String,
        total: u32,
    },
    #[serde(rename_all = "camelCase")]
    CreateProgress { uuid: String, current: u32 },
    #[serde(rename_all = "camelCase")]
    CreateError { uuid: String },
    #[serde(rename_all = "camelCase")]
    CreateEnd { uuid: String },

    #[serde(rename_all = "camelCase")]
    MergeStart {
        uuid: String,
        comic_title: String,
        total: u32,
    },
    #[serde(rename_all = "camelCase")]
    MergeProgress { uuid: String, current: u32 },
    #[serde(rename_all = "camelCase")]
    MergeError { uuid: String },
    #[serde(rename_all = "camelCase")]
    MergeEnd { uuid: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum UpdateDownloadedComicsEvent {
    #[serde(rename_all = "camelCase")]
    GettingComics { total: i64 },

    #[serde(rename_all = "camelCase")]
    GetComicError {
        comic_title: String,
        err_msg: String,
    },

    #[serde(rename_all = "camelCase")]
    ComicGot { current: i64, total: i64 },

    #[serde(rename_all = "camelCase")]
    DownloadTaskCreated,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub timestamp: String,
    pub level: LogLevel,
    pub fields: HashMap<String, serde_json::Value>,
    pub target: String,
    pub filename: String,
    #[serde(rename = "line_number")]
    pub line_number: i64,
}
