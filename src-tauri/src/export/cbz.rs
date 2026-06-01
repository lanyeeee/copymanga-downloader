use std::{
    io::Write,
    path::{Path, PathBuf},
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
        build_grouped_export_targets, get_downloaded_chapters, get_downloaded_chapters_by_uuids,
        get_image_paths, ComicExportLockGuard, ExportFormat, ExportTarget,
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
    export_cbz_internal(app, comic, downloaded_chapters, skip_mode)
}

/// 公开接口：导出指定已下载章节为CBZ
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

    // 调用内部实现
    export_cbz_internal(
        app,
        comic,
        downloaded_chapters,
        ExportSkipMode::None, // 用户主动选择，不跳过
    )
}

/// 内部实现：导出CBZ
#[instrument(level = "error", skip_all, fields(skip_mode = ?skip_mode))]
fn export_cbz_internal(
    app: &AppHandle,
    comic: &Comic,
    downloaded_chapters: Vec<ChapterInfo>,
    skip_mode: ExportSkipMode,
) -> eyre::Result<()> {
    let grouped_export_targets =
        build_grouped_export_targets(app, comic, downloaded_chapters, ExportFormat::Cbz)?;
    if grouped_export_targets.is_empty() {
        return Ok(());
    }

    for (_group_path_word, export_targets) in grouped_export_targets {
        if export_targets.is_empty() {
            continue;
        }

        create_group_cbz_files(app, comic, export_targets, skip_mode)?;
    }

    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
#[instrument(level = "error", skip_all)]
fn create_group_cbz_files(
    app: &AppHandle,
    comic: &Comic,
    export_targets: Vec<ExportTarget>,
    skip_mode: ExportSkipMode,
) -> eyre::Result<()> {
    let create_event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始创建cbz事件
    let _ = ExportCbzEvent::Start {
        uuid: create_event_uuid.clone(),
        comic_title: comic.comic.name.clone(),
        group_title: export_targets[0].chapter_info.group_name.clone(),
        total: export_targets.len() as u32,
    }
    .emit(app);
    // 如果success为false，drop时发送Error事件
    let mut create_error_event_guard = CbzErrorEventGuard {
        uuid: create_event_uuid.clone(),
        app: app.clone(),
        success: false,
    };

    let export_dir = {
        let first_export_path = &export_targets[0].export_path;
        first_export_path
            .parent()
            .ok_or_eyre(format!("获取`{}`的父目录失败", first_export_path.display()))?
            .to_path_buf()
    };
    // 保证导出目录存在
    std::fs::create_dir_all(&export_dir)
        .wrap_err(format!("创建目录`{}`失败", export_dir.display()))?;

    let cfg = yaserde::ser::Config {
        perform_indent: true,
        ..Default::default()
    };

    // 用来记录创建cbz的进度
    let created_count = Arc::new(AtomicU32::new(0));
    // 并发处理
    let current_span = tracing::Span::current();
    let export_targets = export_targets.into_par_iter();
    export_targets.try_for_each(|target| -> eyre::Result<()> {
        let mut chapter_info = target.chapter_info;
        let cbz_path = target.export_path;

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

        // 跳过逻辑
        let should_skip = match skip_mode {
            ExportSkipMode::SkipExported if chapter_info.is_cbz_exported => true,
            ExportSkipMode::SkipExisting if cbz_path.exists() => true,
            _ => false,
        };

        if should_skip {
            // 更新进度
            let current = created_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = ExportCbzEvent::Progress {
                uuid: create_event_uuid.clone(),
                current,
            }
            .emit(app);
            return Ok(());
        }

        // 生成ComicInfo
        let comic_info = ComicInfo::from(comic, &chapter_info);
        // 序列化ComicInfo为xml
        let comic_info_xml = yaserde::ser::to_string_with_config(&comic_info, &cfg)
            .map_err(|err_msg| eyre!("序列化`ComicInfo.xml`失败: {err_msg}"))?;

        let image_paths = get_image_paths(chapter_download_dir).wrap_err(format!(
            "获取`{}`中的图片失败",
            chapter_download_dir.display()
        ))?;

        create_cbz_file(image_paths, &comic_info_xml, &cbz_path)?;

        // 更新章节导出状态
        chapter_info.is_cbz_exported = true;
        chapter_info.save_metadata()?;

        // 更新创建cbz的进度
        let current = created_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        // 发送创建cbz进度事件
        let _ = ExportCbzEvent::Progress {
            uuid: create_event_uuid.clone(),
            current,
        }
        .emit(app);

        Ok(())
    })?;

    // 标记为成功，后面drop时就不会发送Error事件
    create_error_event_guard.success = true;

    // 发送创建cbz完成事件
    let _ = ExportCbzEvent::End {
        uuid: create_event_uuid,
        comic_path_word: comic.comic.path_word.clone(),
        export_dir,
    }
    .emit(app);

    Ok(())
}

#[instrument(level = "error", skip_all, fields(cbz_path = %cbz_path.display()))]
fn create_cbz_file(
    image_paths: Vec<PathBuf>,
    comic_info_xml: &str,
    cbz_path: &Path,
) -> eyre::Result<()> {
    let cbz_file = std::fs::File::create(cbz_path)
        .wrap_err(format!("创建文件`{}`失败", cbz_path.display()))?;
    let mut zip_writer = ZipWriter::new(cbz_file);

    zip_writer
        .start_file("ComicInfo.xml", SimpleFileOptions::default())
        .wrap_err(format!("在`{}`创建`ComicInfo.xml`失败", cbz_path.display()))?;
    zip_writer
        .write_all(comic_info_xml.as_bytes())
        .wrap_err("写入`ComicInfo.xml`失败")?;

    for image_path in image_paths {
        let filename = image_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_eyre(format!("获取`{}`的文件名失败", image_path.display()))?;

        zip_writer
            .start_file(filename, SimpleFileOptions::default())
            .wrap_err(format!("在`{}`创建`{filename}`失败", cbz_path.display()))?;
        let mut file = std::fs::File::open(&image_path)
            .wrap_err(format!("打开`{}`失败", image_path.display()))?;
        std::io::copy(&mut file, &mut zip_writer).wrap_err(format!(
            "将`{}`写入`{}`失败",
            image_path.display(),
            cbz_path.display()
        ))?;
    }

    zip_writer
        .finish()
        .wrap_err(format!("关闭`{}`失败", cbz_path.display()))?;

    Ok(())
}
