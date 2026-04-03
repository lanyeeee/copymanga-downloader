mod cbz;
mod pdf;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

pub use cbz::{cbz, cbz_chapters};
use eyre::WrapErr;
use parking_lot::Mutex;
pub use pdf::{pdf, pdf_chapters};

use crate::{extensions::PathIsImg, types::ChapterInfo};

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
