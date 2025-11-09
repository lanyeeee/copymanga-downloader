use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::{config::Config, responses::ChapterInGetChaptersRespData, utils};

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
    #[allow(clippy::too_many_arguments)] // TODO: 参数太多了，得想办法减少
    #[allow(clippy::cast_precision_loss)]
    pub fn from(
        app: &AppHandle,
        chapter: ChapterInGetChaptersRespData,
        comic_title: String,
        group_name: String,
        comic_uuid: String,
        comic_path_word: String,
        group_path_word: String,
        comic_status: ComicStatus,
    ) -> ChapterInfo {
        let order = chapter.ordered as f64 / 10.0;
        let chapter_title = utils::filename_filter(&chapter.name);
        let prefixed_chapter_title = format!("{order} {chapter_title}");
        let is_downloaded =
            ChapterInfo::get_is_downloaded(app, &comic_title, &group_name, &prefixed_chapter_title);

        ChapterInfo {
            chapter_uuid: chapter.uuid,
            chapter_title,
            chapter_size: chapter.size,
            prefixed_chapter_title,
            comic_uuid,
            comic_title,
            comic_path_word,
            group_path_word,
            group_name,
            group_size: chapter.count,
            order: chapter.ordered as f64 / 10.0,
            comic_status,
            is_downloaded: Some(is_downloaded),
        }
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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub enum ComicStatus {
    #[default]
    Ongoing,
    Completed,
}
