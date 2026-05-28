use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use eyre::{OptionExt, WrapErr};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tracing::instrument;

use crate::{extensions::AppHandleExt, types::Comic, utils};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ChapterInfo {
    pub chapter_uuid: String,
    pub chapter_title: String,
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
    /// 是否曾导出过PDF
    pub is_pdf_exported: bool,
    /// 是否曾导出过CBZ
    pub is_cbz_exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_download_dir: Option<PathBuf>,
}

impl ChapterInfo {
    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_uuid = self.comic_uuid,
            comic_title = self.comic_title,
            group_path_word = self.group_path_word,
            group_name = self.group_name,
            chapter_uuid = self.chapter_uuid,
            order = self.order
        )
    )]
    pub fn save_metadata(&self) -> eyre::Result<()> {
        let mut chapter_info = self.clone();
        // 将is_downloaded和chapter_download_dir字段设置为None
        // 这样能使这些字段在序列化时被忽略
        chapter_info.is_downloaded = None;
        chapter_info.chapter_download_dir = None;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;
        let metadata_path = chapter_download_dir.join("章节元数据.json");

        std::fs::create_dir_all(chapter_download_dir)
            .wrap_err(format!("创建目录`{}`失败", chapter_download_dir.display()))?;

        let chapter_json = serde_json::to_string_pretty(&chapter_info)
            .wrap_err("将ChapterInfo序列化为json失败")?;

        std::fs::write(&metadata_path, chapter_json)
            .wrap_err(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_uuid = self.comic_uuid,
            comic_title = self.comic_title,
            group_path_word = self.group_path_word,
            group_name = self.group_name,
            chapter_uuid = self.chapter_uuid,
            order = self.order
        )
    )]
    pub fn get_temp_download_dir(&self) -> eyre::Result<PathBuf> {
        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        let chapter_download_dir_name = chapter_download_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_eyre(format!(
                "获取`{}`的目录名失败",
                chapter_download_dir.display()
            ))?;

        let parent = chapter_download_dir.parent().ok_or_eyre(format!(
            "`{}`的父目录不存在",
            chapter_download_dir.display()
        ))?;

        let temp_download_dir = parent.join(format!(".下载中-{chapter_download_dir_name}"));
        Ok(temp_download_dir)
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_uuid = comic.comic.uuid,
            comic_title = comic.comic.name,
            group_path_word = self.group_path_word,
            group_name = self.group_name,
            chapter_uuid = self.chapter_uuid,
            order = self.order
        )
    )]
    pub fn get_chapter_relative_dir(&self, comic: &Comic) -> eyre::Result<PathBuf> {
        let comic_download_dir = comic
            .comic_download_dir
            .as_ref()
            .ok_or_eyre("`comic_download_dir`字段为`None`")?;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        let relative_dir = chapter_download_dir
            .strip_prefix(comic_download_dir)
            .wrap_err(format!(
                "无法从路径`{}`中移除前缀`{}`",
                chapter_download_dir.display(),
                comic_download_dir.display()
            ))?;

        Ok(relative_dir.to_path_buf())
    }

    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_uuid = fmt_params.comic_uuid,
            comic_path_word = fmt_params.comic_path_word,
            comic_title = fmt_params.comic_title,
            author = fmt_params.author,
            group_path_word = fmt_params.group_path_word,
            group_title = fmt_params.group_title,
            chapter_uuid = fmt_params.chapter_uuid,
            chapter_title = fmt_params.chapter_title,
            order = fmt_params.order,
        )
    )]
    pub fn get_chapter_download_dir_by_fmt(
        app: &AppHandle,
        comic_download_dir: &Path,
        fmt_params: &ChapterDirFmtParams,
    ) -> eyre::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .wrap_err("将ChapterDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .ok_or_eyre("ChapterDirFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .into_iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();
        let mut chapter_dir_fmt = app.get_config().read().chapter_dir_fmt.clone();
        utils::preprocess_order_placeholder(&mut chapter_dir_fmt, &vars)?;

        let dir_fmt_parts: Vec<&str> = chapter_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).wrap_err("格式化目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }
        // 将格式化后的目录名拼接成完整的目录路径
        let mut chapter_download_dir = comic_download_dir.to_path_buf();
        for dir_name in dir_names {
            chapter_download_dir = chapter_download_dir.join(dir_name);
        }

        Ok(chapter_download_dir)
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

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ChapterDirFmtParams {
    pub comic_uuid: String,
    pub comic_path_word: String,
    pub comic_title: String,
    pub author: String,
    pub group_path_word: String,
    pub group_title: String,
    pub chapter_uuid: String,
    pub chapter_title: String,
    pub order: f64,
}
