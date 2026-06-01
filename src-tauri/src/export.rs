mod cbz;
mod pdf;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

pub use cbz::{cbz, cbz_chapters};
use eyre::{eyre, OptionExt, WrapErr};
use parking_lot::Mutex;
pub use pdf::{pdf, pdf_chapters};
use serde::Serialize;
use tauri::AppHandle;
use tracing::instrument;

use crate::{
    extensions::{AppHandleExt, PathIsImg},
    types::{ChapterInfo, Comic},
    utils,
};

/// 导出互斥锁管理器，确保同一漫画的导出操作串行执行
#[derive(Debug, Clone, Default)]
pub struct ComicExportLock {
    /// 正在导出的漫画`path_word`集合
    locked_comic_path_words: Arc<Mutex<HashSet<String>>>,
}

impl ComicExportLock {
    pub fn new() -> Self {
        Self {
            locked_comic_path_words: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// 尝试获取漫画导出锁，返回是否成功（如果该漫画正在导出则返回 false）
    pub fn try_acquire(&self, comic_path_word: &str) -> bool {
        let mut locked = self.locked_comic_path_words.lock();
        if locked.contains(comic_path_word) {
            return false;
        }
        locked.insert(comic_path_word.to_string());
        true
    }

    /// 释放漫画导出锁
    pub fn release(&self, comic_path_word: &str) {
        self.locked_comic_path_words.lock().remove(comic_path_word);
    }
}

struct ComicExportLockGuard {
    lock: ComicExportLock,
    path_word: String,
}

impl Drop for ComicExportLockGuard {
    fn drop(&mut self) {
        self.lock.release(&self.path_word);
    }
}

/// 导出格式
#[derive(Debug, Copy, Clone)]
enum ExportFormat {
    Pdf,
    Cbz,
}

impl ExportFormat {
    fn extension(&self) -> &str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Cbz => "cbz",
        }
    }
}

/// 获取已下载的章节
fn get_downloaded_chapters(groups: &HashMap<String, Vec<ChapterInfo>>) -> Vec<ChapterInfo> {
    groups
        .values()
        .flatten()
        .filter(|chapter| chapter.is_downloaded.unwrap_or(false))
        .cloned()
        .collect()
}

/// 根据UUID列表获取章节
fn get_downloaded_chapters_by_uuids(
    groups: &HashMap<String, Vec<ChapterInfo>>,
    chapter_uuids: &[String],
) -> Vec<ChapterInfo> {
    let uuid_set: HashSet<_> = chapter_uuids.iter().collect();
    groups
        .values()
        .flatten()
        .filter(|chapter| {
            chapter.is_downloaded.unwrap_or(false) && uuid_set.contains(&chapter.chapter_uuid)
        })
        .cloned()
        .collect()
}

#[instrument(level = "error", skip_all, fields(images_dir = %images_dir.display()))]
fn get_image_paths(images_dir: &Path) -> eyre::Result<Vec<PathBuf>> {
    let mut image_paths: Vec<PathBuf> = std::fs::read_dir(images_dir)
        .wrap_err(format!("读取目录`{}`失败", images_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_img())
        .collect();
    image_paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(image_paths)
}

#[derive(Debug, Clone)]
struct ExportTarget {
    chapter_info: ChapterInfo,
    export_path: PathBuf,
}

const EXPORT_FMT_COMMON_FIELDS: [&str; 5] = [
    "comic_uuid",
    "comic_path_word",
    "comic_title",
    "author",
    "export_format",
];
const EXPORT_FMT_GROUP_FIELDS: [&str; 2] = ["group_path_word", "group_title"];
const EXPORT_FMT_CHAPTER_FIELDS: [&str; 3] = ["chapter_uuid", "chapter_title", "order"];

#[derive(Debug, Clone, Serialize)]
struct ChapterExportFmtParams {
    comic_uuid: String,
    comic_path_word: String,
    comic_title: String,
    author: String,
    export_format: String,
    group_path_word: String,
    group_title: String,
    chapter_uuid: String,
    chapter_title: String,
    order: f64,
}

impl ChapterExportFmtParams {
    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_uuid = self.comic_uuid,
            comic_path_word = self.comic_path_word,
            comic_title = self.comic_title,
            group_path_word = self.group_path_word,
            group_title = self.group_title,
            chapter_uuid = self.chapter_uuid,
            chapter_title = self.chapter_title,
            order = self.order,
            export_format = self.export_format
        )
    )]
    fn to_export_path(
        &self,
        export_dir: &Path,
        export_dir_fmt: &str,
        format: ExportFormat,
    ) -> eyre::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(self)
            .wrap_err("将ChapterExportFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .ok_or_eyre("ChapterExportFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                (key.clone(), value)
            })
            .collect();

        let mut export_dir_fmt = export_dir_fmt.to_string();
        utils::preprocess_order_placeholder(&mut export_dir_fmt, &vars)?;

        let dir_fmt_parts: Vec<&str> = export_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt_part in dir_fmt_parts {
            let dir_name = strfmt(fmt_part, &vars).wrap_err("格式化导出目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }

        let Some(filename) = dir_names.pop() else {
            let err_msg =
            "配置中的导出目录格式至少要有1个层级，例如这个例子是4个层级：{comic_title}/{export_format}/{group_title}/{order} {chapter_title}";
            return Err(eyre!(err_msg));
        };

        let mut export_path = export_dir.to_path_buf();
        for dir_name in dir_names {
            export_path = export_path.join(dir_name);
        }

        let ext = format.extension();

        Ok(export_path.join(format!("{filename}.{ext}")))
    }
}

#[instrument(
    level = "error",
    skip_all,
    fields(comic_uuid = comic.comic.uuid, comic_title = comic.comic.name, export_format = ?export_format)
)]
pub fn build_grouped_export_targets(
    app: &AppHandle,
    comic: &Comic,
    chapter_infos: Vec<ChapterInfo>,
    export_format: ExportFormat,
) -> eyre::Result<HashMap<String, Vec<ExportTarget>>> {
    let (export_dir, export_dir_fmt) = {
        let config = app.get_config();
        let config = config.read();
        (config.export_dir.clone(), config.export_dir_fmt.clone())
    };

    validate_export_dir_fmt(&export_dir_fmt)?;

    let author = comic
        .comic
        .author
        .iter()
        .map(|author| author.name.clone())
        .collect::<Vec<_>>()
        .join(", ");

    let mut grouped_export_targets: HashMap<String, Vec<ExportTarget>> = HashMap::new();

    for chapter_info in chapter_infos {
        let fmt_params = ChapterExportFmtParams {
            comic_uuid: comic.comic.uuid.clone(),
            comic_path_word: comic.comic.path_word.clone(),
            comic_title: comic.comic.name.clone(),
            author: author.clone(),
            export_format: export_format.extension().to_string(),
            group_path_word: chapter_info.group_path_word.clone(),
            group_title: chapter_info.group_name.clone(),
            chapter_uuid: chapter_info.chapter_uuid.clone(),
            chapter_title: chapter_info.chapter_title.clone(),
            order: chapter_info.order,
        };

        let export_path = fmt_params.to_export_path(&export_dir, &export_dir_fmt, export_format)?;

        grouped_export_targets
            .entry(chapter_info.group_path_word.clone())
            .or_default()
            .push(ExportTarget {
                chapter_info,
                export_path,
            });
    }

    Ok(grouped_export_targets)
}

fn validate_export_dir_fmt(export_dir_fmt: &str) -> eyre::Result<()> {
    let mut fmt_parts: Vec<&str> = export_dir_fmt.split('/').collect();

    let Some(last_fmt_part) = fmt_parts.pop() else {
        return Err(eyre!("`导出目录格式`不能为空"));
    };

    if !contains_any_field(last_fmt_part, &EXPORT_FMT_CHAPTER_FIELDS) {
        return Err(eyre!(
            "`导出目录格式`不合法，最后一层必须至少包含一个章节字段 {:?}",
            EXPORT_FMT_CHAPTER_FIELDS
        ));
    }

    for fmt_part in fmt_parts {
        if let Some(field) = find_used_field(fmt_part, &EXPORT_FMT_CHAPTER_FIELDS) {
            return Err(eyre!(
                "`导出目录格式`不合法，章节字段只能出现在最后一层，你当前在非最后一层使用了`{field}`。非最后一层只允许使用 {:?} 和 {:?}",
                EXPORT_FMT_COMMON_FIELDS,
                EXPORT_FMT_GROUP_FIELDS,
            ));
        }
    }

    Ok(())
}

fn contains_any_field(fmt_part: &str, fields: &[&str]) -> bool {
    find_used_field(fmt_part, fields).is_some()
}

fn find_used_field<'a>(fmt_part: &str, fields: &'a [&'a str]) -> Option<&'a str> {
    fields.iter().copied().find(|field| {
        let placeholder = format!("{{{field}}}");
        let format_prefix = format!("{{{field}:");
        fmt_part.contains(&placeholder) || fmt_part.contains(&format_prefix)
    })
}
