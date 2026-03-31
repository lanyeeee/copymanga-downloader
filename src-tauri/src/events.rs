use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::{
    downloader::download_task_state::DownloadTaskState,
    types::{ChapterInfo, Comic, LogLevel},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Speed { speed: String },

    #[serde(rename_all = "camelCase")]
    RiskControl {
        chapter_uuid: String,
        retry_after: u32,
    },

    #[serde(rename_all = "camelCase")]
    Sleeping {
        chapter_uuid: String,
        remaining_sec: u64,
    },

    #[serde(rename_all = "camelCase")]
    TaskCreate {
        state: DownloadTaskState,
        comic: Box<Comic>,
        chapter_info: Box<ChapterInfo>,
        downloaded_img_count: u32,
        total_img_count: u32,
    },

    #[serde(rename_all = "camelCase")]
    TaskDelete { chapter_uuid: String },

    #[serde(rename_all = "camelCase")]
    TaskUpdate {
        chapter_uuid: String,
        state: DownloadTaskState,
        downloaded_img_count: u32,
        total_img_count: u32,
    },
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
    End {
        uuid: String,
        comic_path_word: String,
        chapter_export_dir: PathBuf,
    },
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
    CreateEnd {
        uuid: String,
        comic_path_word: String,
        chapter_export_dir: PathBuf,
    },

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
    MergeEnd {
        uuid: String,
        comic_path_word: String,
        chapter_export_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(tag = "event", content = "data")]
pub enum UpdateDownloadedComicsEvent {
    #[serde(rename_all = "camelCase")]
    GetComicStart { total: i64 },

    #[serde(rename_all = "camelCase")]
    GetComicProgress { current: i64, total: i64 },

    #[serde(rename_all = "camelCase")]
    CreateDownloadTasksStart {
        comic_path_word: String,
        comic_title: String,
    },

    #[serde(rename_all = "camelCase")]
    CreateDownloadTasksEnd { comic_path_word: String },

    #[serde(rename_all = "camelCase")]
    GetComicEnd,
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
