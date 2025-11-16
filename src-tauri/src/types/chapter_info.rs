use std::path::Path;

use anyhow::Context;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::config::Config;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    pub chapter_uuid: String,
    pub chapter_title: String,
    /// 以order为前缀的章节标题
    pub prefixed_chapter_title: String,
    /// 此章节有多少页
    pub chapter_size: i64,
    pub comic_uuid: String,
    pub comic_title: String,
    pub comic_path_word: String,
    pub group_path_word: String,
    pub group_name: String,
    /// 此章节对应的group有多少章节
    pub group_size: i64,
    /// 此章节在group中的顺序
    pub order: f64,
    /// 漫画的连载状态
    pub comic_status: ComicStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
}

impl ChapterInfo {
    pub fn save_metadata(&self, chapter_download_dir: &Path) -> anyhow::Result<()> {
        let mut chapter_info = self.clone();
        // 将is_downloaded字段设置为None，这样能使它在序列化时被跳过
        chapter_info.is_downloaded = None;

        let metadata_path = chapter_download_dir.join("章节元数据.json");

        std::fs::create_dir_all(chapter_download_dir)
            .context(format!("创建目录`{}`失败", chapter_download_dir.display()))?;

        let chapter_json =
            serde_json::to_string_pretty(&chapter_info).context("将ChapterInfo序列化为json失败")?;

        std::fs::write(&metadata_path, chapter_json)
            .context(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    pub fn get_is_downloaded(
        app: &AppHandle,
        comic_title: &str,
        group_name: &str,
        prefixed_chapter_title: &str,
    ) -> bool {
        app.state::<RwLock<Config>>()
            .read()
            .download_dir
            .join(comic_title)
            .join(group_name)
            .join(prefixed_chapter_title)
            .exists()
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub enum ComicStatus {
    #[default]
    Ongoing,
    Completed,
}
