use std::{
    io::Write,
    sync::{atomic::AtomicU32, Arc},
};

use eyre::{eyre, OptionExt, WrapErr};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tauri::AppHandle;
use tauri_specta::Event;
use tracing::instrument;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    config::ExportSkipMode,
    events::ExportCbzEvent,
    export::{
        get_downloaded_chapters, get_downloaded_chapters_by_uuids, get_image_paths,
        ComicExportLockGuard, ExportFormat,
    },
    extensions::AppHandleExt,
    types::{ChapterInfo, Comic, ComicInfo},
};

struct CbzErrorEventGuard {
    uuid: String,
    app: AppHandle,
    success: bool,
}

impl Drop for CbzErrorEventGuard {
    fn drop(&mut self) {
        if self.success {
            return;
        }

        let uuid = self.uuid.clone();
        let _ = ExportCbzEvent::Error { uuid }.emit(&self.app);
    }
}

/// 公开接口：导出全部已下载章节为CBZ
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[instrument(level = "error", skip_all, fields(comic_uuid = comic.comic.uuid, comic_title = comic.comic.name))]
pub fn cbz(app: &AppHandle, comic: &Comic) -> eyre::Result<()> {
    let comic_path_word = &comic.comic.path_word;
    let comic_title = &comic.comic.name;
    let export_lock = app.get_export_lock().inner().clone();

    // 检查导出锁
    if !export_lock.try_acquire(comic_path_word) {
        return Err(eyre!("漫画`{comic_title}`正在导出，请稍后再试"));
    }

    let _guard = ComicExportLockGuard {
        lock: export_lock.clone(),
        path_word: comic_path_word.clone(),
    };

    // 获取配置
    let skip_mode = app.get_config().read().export_skip_mode;

    // 获取已下载章节
    let downloaded_chapters = get_downloaded_chapters(&comic.comic.groups);

    // 调用内部实现
    cbz_internal(
        app,
        comic,
        downloaded_chapters,
        skip_mode,
        comic_path_word,
        comic_title,
    )
}

/// 公开接口：导出指定已下载章节为CBZ
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
#[instrument(level = "error", skip_all, fields(comic_uuid = comic.comic.uuid, comic_title = comic.comic.name))]
pub fn cbz_chapters(
    app: &AppHandle,
    comic: &Comic,
    chapter_uuids: Vec<String>,
) -> eyre::Result<()> {
    let comic_path_word = &comic.comic.path_word;
    let comic_title = &comic.comic.name;
    let export_lock = app.get_export_lock().inner().clone();

    // 检查导出锁
    if !export_lock.try_acquire(comic_path_word) {
        return Err(eyre!("漫画`{comic_title}`正在导出，请稍后再试"));
    }

    let _guard = ComicExportLockGuard {
        lock: export_lock.clone(),
        path_word: comic_path_word.clone(),
    };

    // 获取指定章节（用户主动选择，不跳过）
    let downloaded_chapters = get_downloaded_chapters_by_uuids(&comic.comic.groups, &chapter_uuids);

    // 调用内部实现，skip_mode = None（用户主动选择，不跳过）
    cbz_internal(
        app,
        comic,
        downloaded_chapters,
        ExportSkipMode::None,
        comic_path_word,
        comic_title,
    )
}

/// 内部实现：导出CBZ
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
#[instrument(level = "error", skip_all, fields(skip_mode = ?skip_mode))]
fn cbz_internal(
    app: &AppHandle,
    comic: &Comic,
    downloaded_chapters: Vec<ChapterInfo>,
    skip_mode: ExportSkipMode,
    // TODO: 删掉`comic_path_word` 和 `comic_title`，因为它们根本不是必要的参数，完全可以从 `comic` 中取到
    comic_path_word: &str,
    comic_title: &str,
) -> eyre::Result<()> {
    // 用于生成格式化的xml
    let xml_cfg = yaserde::ser::Config {
        perform_indent: true,
        ..Default::default()
    };
    let event_uuid = uuid::Uuid::new_v4().to_string();

    // 发送开始导出cbz事件
    let _ = ExportCbzEvent::Start {
        uuid: event_uuid.clone(),
        comic_title: comic_title.to_string(),
        total: downloaded_chapters.len() as u32,
    }
    .emit(app);

    // 如果success为false，drop时发送Error事件
    let mut error_event_guard = CbzErrorEventGuard {
        uuid: event_uuid.clone(),
        app: app.clone(),
        success: false,
    };

    // 用来记录导出进度
    let current = Arc::new(AtomicU32::new(0));

    let extension = ExportFormat::Cbz.extension();
    let comic_export_dir = comic
        .get_comic_export_dir(app)
        .wrap_err(format!("`{comic_title}` 获取导出目录失败"))?;
    let cbz_export_dir = comic_export_dir.join(extension);

    // 并发处理
    let current_span = tracing::Span::current();
    let download_chapters = downloaded_chapters.into_par_iter();
    download_chapters.try_for_each(|mut chapter_info| -> eyre::Result<()> {
        let _enter = current_span.enter();

        let span = tracing::error_span!(
            "export_cbz_rayon",
            group_name = chapter_info.group_name,
            chapter_title = chapter_info.chapter_title
        );
        let _enter = span.enter();

        // 获取导出路径
        let chapter_download_dir = chapter_info
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;
        let chapter_download_dir_name = chapter_download_dir
            .file_name()
            .and_then(|name: &std::ffi::OsStr| name.to_str())
            .ok_or_eyre(format!(
                "获取`{}`的目录名失败",
                chapter_download_dir.display()
            ))?;
        let chapter_relative_dir = chapter_info
            .get_chapter_relative_dir(comic)
            .wrap_err("获取章节相对目录失败")?;
        let chapter_relative_dir_parent = chapter_relative_dir
            .parent()
            .ok_or_eyre(format!("`{}`没有父目录", chapter_relative_dir.display()))?;
        let chapter_export_dir = cbz_export_dir.join(chapter_relative_dir_parent);

        // 保证导出目录存在
        std::fs::create_dir_all(&chapter_export_dir)
            .wrap_err(format!("创建目录`{}`失败", chapter_export_dir.display()))?;

        let zip_path = chapter_export_dir.join(format!("{chapter_download_dir_name}.{extension}"));

        // 跳过逻辑
        let should_skip = match skip_mode {
            ExportSkipMode::SkipExported if chapter_info.is_cbz_exported => true,
            ExportSkipMode::SkipExisting if zip_path.exists() => true,
            _ => false,
        };

        if should_skip {
            // 更新进度
            let current = current.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = ExportCbzEvent::Progress {
                uuid: event_uuid.clone(),
                current,
            }
            .emit(app);
            return Ok(());
        }

        // 生成ComicInfo
        let comic_info = ComicInfo::from(comic, &chapter_info);
        // 序列化ComicInfo为xml
        let comic_info_xml = yaserde::ser::to_string_with_config(&comic_info, &xml_cfg)
            .map_err(|err_msg| eyre!("序列化`ComicInfo.xml`失败: {err_msg}"))?;

        // 创建cbz文件
        let zip_file = std::fs::File::create(&zip_path)
            .wrap_err(format!("创建文件`{}`失败", zip_path.display()))?;
        let mut zip_writer = ZipWriter::new(zip_file);

        // 把ComicInfo.xml写入cbz
        zip_writer
            .start_file("ComicInfo.xml", SimpleFileOptions::default())
            .wrap_err(format!("在`{}`创建`ComicInfo.xml`失败", zip_path.display()))?;
        zip_writer
            .write_all(comic_info_xml.as_bytes())
            .wrap_err("写入`ComicInfo.xml`失败")?;

        let image_paths = get_image_paths(chapter_download_dir).wrap_err(format!(
            "获取`{}`中的图片失败",
            chapter_download_dir.display()
        ))?;

        for image_path in image_paths {
            let filename = image_path
                .file_name()
                .and_then(|name: &std::ffi::OsStr| name.to_str())
                .ok_or_eyre(format!("获取`{}`的文件名失败", image_path.display()))?;
            // 将文件写入cbz
            zip_writer
                .start_file(filename, SimpleFileOptions::default())
                .wrap_err(format!("在`{}`创建`{filename:?}`失败", zip_path.display()))?;
            let mut file = std::fs::File::open(&image_path)
                .wrap_err(format!("打开`{}`失败", image_path.display()))?;
            std::io::copy(&mut file, &mut zip_writer).wrap_err(format!(
                "将`{}`写入`{}`失败",
                image_path.display(),
                zip_path.display()
            ))?;
        }

        zip_writer
            .finish()
            .wrap_err(format!("关闭`{}`失败", zip_path.display()))?;

        // 更新章节导出状态
        chapter_info.is_cbz_exported = true;
        chapter_info.save_metadata()?;

        // 更新导出cbz的进度
        let current = current.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        // 发送导出cbz进度事件
        let _ = ExportCbzEvent::Progress {
            uuid: event_uuid.clone(),
            current,
        }
        .emit(app);

        Ok(())
    })?;

    // 标记为成功，后面drop时就不会发送Error事件
    error_event_guard.success = true;
    // 发送导出cbz完成事件
    let _ = ExportCbzEvent::End {
        uuid: event_uuid,
        comic_path_word: comic_path_word.to_string(),
        chapter_export_dir: cbz_export_dir,
    }
    .emit(app);

    Ok(())
}
