use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::AtomicU32, Arc},
};

use eyre::{eyre, OptionExt, WrapErr};
use float_ord::FloatOrd;
use lopdf::{
    content::{Content, Operation},
    dictionary, Bookmark, Document, Object, Stream,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Serialize;
use tauri::AppHandle;
use tauri_specta::Event;
use tracing::instrument;

use crate::{
    config::ExportSkipMode,
    events::ExportPdfEvent,
    export::{
        build_grouped_export_targets, contains_any_field, get_downloaded_chapters,
        get_downloaded_chapters_by_uuids, get_image_paths, ComicExportLockGuard, ExportFormat,
        ExportTarget, EXPORT_FMT_GROUP_FIELDS,
    },
    extensions::AppHandleExt,
    types::{ChapterInfo, Comic},
    utils,
};

struct PdfCreateErrorEventGuard {
    uuid: String,
    app: AppHandle,
    success: bool,
}

impl Drop for PdfCreateErrorEventGuard {
    fn drop(&mut self) {
        if self.success {
            return;
        }

        let uuid = self.uuid.clone();
        let _ = ExportPdfEvent::CreateError { uuid }.emit(&self.app);
    }
}

struct PdfMergeErrorEventGuard {
    uuid: String,
    app: AppHandle,
    success: bool,
}

impl Drop for PdfMergeErrorEventGuard {
    fn drop(&mut self) {
        if self.success {
            return;
        }

        let uuid = self.uuid.clone();
        let _ = ExportPdfEvent::MergeError { uuid }.emit(&self.app);
    }
}

/// 公开接口：导出全部已下载章节为PDF
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
#[instrument(level = "error", skip_all, fields(comic_uuid = comic.comic.uuid, comic_title = comic.comic.name))]
pub fn pdf(app: &AppHandle, comic: &Comic) -> eyre::Result<()> {
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
    let (skip_mode, enable_merge) = {
        let config = app.get_config().inner().read();

        let skip_mode = config.export_skip_mode;
        let enable_merge = if skip_mode == ExportSkipMode::SkipExported {
            false
        } else {
            config.enable_merge_pdf
        };

        (skip_mode, enable_merge)
    };

    // 获取已下载章节
    let downloaded_chapters = get_downloaded_chapters(&comic.comic.groups);

    // 调用内部实现
    export_pdf_internal(app, comic, downloaded_chapters, skip_mode, enable_merge)
}

/// 公开接口：导出指定已下载章节为PDF
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
#[instrument(level = "error", skip_all, fields(comic_uuid = comic.comic.uuid, comic_title = comic.comic.name))]
pub fn pdf_chapters(
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

    // 获取指定章节（用户主动选择，不跳过，不合并）
    let downloaded_chapters = get_downloaded_chapters_by_uuids(&comic.comic.groups, &chapter_uuids);

    // 调用内部实现
    export_pdf_internal(
        app,
        comic,
        downloaded_chapters,
        ExportSkipMode::None, // 用户主动选择，不跳过
        false,                // 选择性导出，不合并
    )
}

/// 内部实现：导出PDF
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
#[instrument(level = "error", skip_all, fields(skip_mode = ?skip_mode))]
fn export_pdf_internal(
    app: &AppHandle,
    comic: &Comic,
    downloaded_chapters: Vec<ChapterInfo>,
    skip_mode: ExportSkipMode,
    enable_merge: bool,
) -> eyre::Result<()> {
    let grouped_export_targets =
        build_grouped_export_targets(app, comic, downloaded_chapters, ExportFormat::Pdf)?;
    if grouped_export_targets.is_empty() {
        return Ok(());
    }

    let grouped_export_targets_for_merge = if enable_merge {
        Some(grouped_export_targets.clone())
    } else {
        None
    };

    let create_pdf_concurrency = app.get_config().read().create_pdf_concurrency;
    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(create_pdf_concurrency)
        .build()
        .wrap_err("rayon线程池创建失败")?;

    thread_pool.install(|| -> eyre::Result<()> {
        for (_group_path_word, export_targets) in grouped_export_targets {
            if export_targets.is_empty() {
                continue;
            }

            create_group_pdf_files(app, comic, export_targets, skip_mode)?;
        }

        Ok(())
    })?;

    // 合并PDF
    if let Some(grouped_export_targets) = grouped_export_targets_for_merge {
        let (export_dir, merge_pdf_fmt) = {
            let config = app.get_config();
            let config = config.read();
            (config.export_dir.clone(), config.merge_pdf_fmt.clone())
        };

        validate_merge_pdf_fmt(&merge_pdf_fmt)?;

        // 合并PDF很吃内存，为了减少爆内存的发生，不使用并发处理，而是逐个合并
        for (group_path_word, export_targets) in grouped_export_targets {
            if export_targets.is_empty() {
                continue;
            }

            let fmt_params = MergePdfFmtParams {
                comic_uuid: comic.comic.uuid.clone(),
                comic_path_word: comic.comic.path_word.clone(),
                comic_title: comic.comic.name.clone(),
                author: comic
                    .comic
                    .author
                    .iter()
                    .map(|author| author.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                group_path_word,
                group_title: export_targets[0].chapter_info.group_name.clone(),
            };

            let merge_pdf_path = fmt_params.to_merge_pdf_path(&export_dir, &merge_pdf_fmt)?;

            merge_group_pdf_files(app, comic, &merge_pdf_path, export_targets)?;
        }
    }

    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
#[instrument(level = "error", skip_all)]
fn create_group_pdf_files(
    app: &AppHandle,
    comic: &Comic,
    export_targets: Vec<ExportTarget>,
    skip_mode: ExportSkipMode,
) -> eyre::Result<()> {
    let create_event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始创建pdf事件
    let _ = ExportPdfEvent::CreateStart {
        uuid: create_event_uuid.clone(),
        comic_title: comic.comic.name.clone(),
        group_title: export_targets[0].chapter_info.group_name.clone(),
        total: export_targets.len() as u32,
    }
    .emit(app);
    // 如果success为false，drop时发送CreateError事件
    let mut create_error_event_guard = PdfCreateErrorEventGuard {
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

    // 用来记录创建pdf的进度
    let created_count = Arc::new(AtomicU32::new(0));
    //并发处理
    let current_span = tracing::Span::current();
    let export_targets = export_targets.into_par_iter();
    export_targets.try_for_each(|target| -> eyre::Result<()> {
        let mut chapter_info = target.chapter_info;
        let pdf_path = target.export_path;

        let _enter = current_span.enter();

        let span = tracing::error_span!(
            "export_pdf_rayon",
            group_name = chapter_info.group_name,
            chapter_title = chapter_info.chapter_title
        );
        let _enter = span.enter();

        // 创建pdf文件
        let chapter_download_dir = chapter_info
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        // 跳过逻辑
        let should_skip = match skip_mode {
            ExportSkipMode::SkipExported if chapter_info.is_pdf_exported => true,
            ExportSkipMode::SkipExisting if pdf_path.exists() => true,
            _ => false,
        };

        if should_skip {
            // 更新进度
            let current = created_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = ExportPdfEvent::CreateProgress {
                uuid: create_event_uuid.clone(),
                current,
            }
            .emit(app);
            return Ok(());
        }

        let image_paths = get_image_paths(chapter_download_dir).wrap_err(format!(
            "获取`{}`中的图片失败",
            chapter_download_dir.display()
        ))?;

        create_pdf_file(image_paths, &pdf_path).wrap_err("创建pdf失败")?;

        // 更新章节导出状态
        chapter_info.is_pdf_exported = true;
        chapter_info.save_metadata()?;

        // 更新创建pdf的进度
        let current = created_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        // 发送创建pdf进度事件
        let _ = ExportPdfEvent::CreateProgress {
            uuid: create_event_uuid.clone(),
            current,
        }
        .emit(app);

        Ok(())
    })?;

    // 标记为成功，后面drop时就不会发送CreateError事件
    create_error_event_guard.success = true;

    // 发送创建pdf完成事件
    let _ = ExportPdfEvent::CreateEnd {
        uuid: create_event_uuid,
        comic_path_word: comic.comic.path_word.clone(),
        export_dir,
    }
    .emit(app);

    Ok(())
}

#[instrument(level = "error", skip_all)]
fn merge_group_pdf_files(
    app: &AppHandle,
    comic: &Comic,
    merge_pdf_path: &Path,
    mut export_targets: Vec<ExportTarget>,
) -> eyre::Result<()> {
    let group_title = export_targets[0].chapter_info.group_name.clone();

    let merge_event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始合并pdf事件
    let _ = ExportPdfEvent::MergeStart {
        uuid: merge_event_uuid.clone(),
        comic_title: comic.comic.name.clone(),
        group_title: group_title.clone(),
        total: 1,
    }
    .emit(app);
    // 如果success为false，drop时发送MergeError事件
    let mut merge_error_event_guard = PdfMergeErrorEventGuard {
        uuid: merge_event_uuid.clone(),
        app: app.clone(),
        success: false,
    };

    export_targets.sort_by_key(|target| FloatOrd(target.chapter_info.order));

    let merge_pdf_dir = merge_pdf_path
        .parent()
        .ok_or_eyre(format!("获取`{}`的父目录失败", merge_pdf_path.display()))?;
    std::fs::create_dir_all(merge_pdf_dir)
        .wrap_err(format!("创建目录`{}`失败", merge_pdf_dir.display()))?;

    let chapter_pdf_paths: Vec<PathBuf> = export_targets
        .into_iter()
        .map(|target| target.export_path)
        .collect();

    // 合并pdf
    merge_pdf_file(chapter_pdf_paths, merge_pdf_path)?;

    // 标记为成功，后面drop时就不会发送MergeError事件
    merge_error_event_guard.success = true;
    // 发送合并pdf完成事件
    let _ = ExportPdfEvent::MergeEnd {
        uuid: merge_event_uuid,
        comic_path_word: comic.comic.path_word.clone(),
        export_dir: merge_pdf_dir.to_path_buf(),
    }
    .emit(app);

    Ok(())
}

const MERGE_FMT_COMMON_FIELDS: [&str; 4] =
    ["comic_uuid", "comic_path_word", "comic_title", "author"];

#[derive(Debug, Clone, Serialize)]
struct MergePdfFmtParams {
    comic_uuid: String,
    comic_path_word: String,
    comic_title: String,
    author: String,
    group_path_word: String,
    group_title: String,
}

impl MergePdfFmtParams {
    #[instrument(
        level = "error",
        skip_all,
        fields(
            comic_uuid = self.comic_uuid,
            comic_path_word = self.comic_path_word,
            comic_title = self.comic_title,
            group_path_word = self.group_path_word,
            group_title = self.group_title
        )
    )]
    fn to_merge_pdf_path(&self, export_dir: &Path, merge_pdf_fmt: &str) -> eyre::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value =
            serde_json::to_value(self).wrap_err("将MergePdfFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .ok_or_eyre("MergePdfFmtParams不是JSON对象")?;

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

        let dir_fmt_parts: Vec<&str> = merge_pdf_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt_part in dir_fmt_parts {
            let dir_name = strfmt(fmt_part, &vars).wrap_err("格式化合并pdf目录目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }

        let Some(filename) = dir_names.pop() else {
            let err_msg =
            "配置中的导出目录格式至少要有1个层级，例如这个例子是3个层级：{comic_title}/pdf/{group_title}";
            return Err(eyre!(err_msg));
        };

        let mut output_path = export_dir.to_path_buf();
        for dir_name in dir_names {
            output_path = output_path.join(dir_name);
        }

        Ok(output_path.join(format!("{filename}.pdf")))
    }
}

fn validate_merge_pdf_fmt(merge_pdf_fmt: &str) -> eyre::Result<()> {
    if !contains_any_field(merge_pdf_fmt, &EXPORT_FMT_GROUP_FIELDS) {
        return Err(eyre!(
            "`合并pdf目录格式`不合法，整个模板中必须至少包含一个分组字段 {:?}",
            EXPORT_FMT_GROUP_FIELDS
        ));
    }

    Ok(())
}

/// 用`image_paths`中的图片创建PDF文件，保存到`pdf_path`
#[allow(clippy::similar_names)]
#[allow(clippy::cast_possible_truncation)]
#[instrument(level = "error", skip_all, fields(pdf_path = %pdf_path.display()))]
fn create_pdf_file(image_paths: Vec<PathBuf>, pdf_path: &Path) -> eyre::Result<()> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = vec![];

    for image_path in image_paths {
        if !image_path.is_file() {
            continue;
        }

        let buffer = read_image_to_buffer(&image_path)
            .wrap_err(format!("将`{}`读取到buffer失败", image_path.display()))?;
        let (width, height) = utils::get_dimensions(&buffer)
            .wrap_err(format!("获取`{}`的尺寸失败", image_path.display()))?;
        let image_stream = lopdf::xobject::image_from(buffer)
            .wrap_err(format!("创建`{}`的图片流失败", image_path.display()))?;
        // 将图片流添加到doc中
        let img_id = doc.add_object(image_stream);
        // 图片的名称，用于 Do 操作在页面上显示图片
        let img_name = format!("X{}", img_id.0);
        // 用于设置图片在页面上的位置和大小
        let cm_operation = Operation::new(
            "cm",
            vec![
                width.into(),
                0.into(),
                0.into(),
                height.into(),
                0.into(),
                0.into(),
            ],
        );
        // 用于显示图片
        let do_operation = Operation::new("Do", vec![Object::Name(img_name.as_bytes().to_vec())]);
        // 创建页面，设置图片的位置和大小，然后显示图片
        // 因为是从零开始创建PDF，所以没必要用 q 和 Q 操作保存和恢复图形状态
        let content = Content {
            operations: vec![cm_operation, do_operation],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode()?));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
        });
        // 将图片以 XObject 的形式添加到文档中
        // Do 操作只能引用 XObject(所以前面定义的 Do 操作的参数是 img_name, 而不是 img_id)
        doc.add_xobject(page_id, img_name.as_bytes(), img_id)?;
        // 记录新创建的页面的 ID
        page_ids.push(page_id);
    }
    // 将"Pages"添加到doc中
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Count" => page_ids.len() as u32,
        "Kids" => page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    // 新建一个"Catalog"对象，将"Pages"对象添加到"Catalog"对象中，然后将"Catalog"对象添加到doc中
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    doc.compress();

    doc.save(pdf_path)
        .wrap_err(format!("保存`{}`失败", pdf_path.display()))?;
    Ok(())
}

/// 读取`image_path`中的图片数据到buffer中
#[instrument(level = "error", skip_all, fields(image_path = %image_path.display()))]
fn read_image_to_buffer(image_path: &Path) -> eyre::Result<Vec<u8>> {
    let file =
        std::fs::File::open(image_path).wrap_err(format!("打开`{}`失败", image_path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = vec![];
    reader
        .read_to_end(&mut buffer)
        .wrap_err(format!("读取`{}`失败", image_path.display()))?;
    Ok(buffer)
}

/// 将`chapter_pdf_paths`中的PDF合并到`pdf_path`中
#[allow(clippy::cast_possible_truncation)]
#[instrument(level = "error", skip_all, fields(pdf_path = %pdf_path.display()))]
fn merge_pdf_file(chapter_pdf_paths: Vec<PathBuf>, pdf_path: &Path) -> eyre::Result<()> {
    let mut doc = Document::with_version("1.5");
    let mut doc_page_ids = vec![];
    let mut doc_objects = BTreeMap::new();

    for chapter_pdf_path in chapter_pdf_paths {
        let mut chapter_doc = Document::load(&chapter_pdf_path)
            .wrap_err(format!("加载`{}`失败", chapter_pdf_path.display()))?;
        // 重新编号这个章节PDF的对象，避免与doc的对象编号冲突
        chapter_doc.renumber_objects_with(doc.max_id);
        doc.max_id = chapter_doc.max_id + 1;
        // 获取这个章节PDF中的所有页面，并给第一个页面添加书签
        let mut chapter_page_ids = vec![];
        for (page_num, object_id) in chapter_doc.get_pages() {
            // 第一个页面需要添加书签
            if page_num == 1 {
                let chapter_title = chapter_pdf_path
                    .file_stem()
                    .and_then(|file_stem| file_stem.to_str())
                    .ok_or_eyre(format!("获取`{}`的文件名失败", chapter_pdf_path.display()))?
                    .to_string();
                let bookmark = Bookmark::new(chapter_title, [0.0, 0.0, 1.0], 0, object_id);
                doc.add_bookmark(bookmark, None);
            }
            chapter_page_ids.push(object_id);
        }

        doc_page_ids.extend(chapter_page_ids);
        doc_objects.extend(chapter_doc.objects);
    }
    // 在doc中新建一个"Pages"对象，将所有章节的页面添加到这个"Pages"对象中
    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Count" => doc_page_ids.len() as u32,
        "Kids" => doc_page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
    });

    for (object_id, mut object) in doc_objects {
        match object.type_name().unwrap_or(b"") {
            b"Page" => {
                if let Ok(page_dict) = object.as_dict_mut() {
                    // 将页面对象的"Parent"字段设置为新建的"Pages"对象，这样这个页面就成为了"Pages"对象的子页面
                    page_dict.set("Parent", pages_id);
                    doc.objects.insert(object_id, object);
                };
            }
            // 忽略这些对象
            b"Catalog" | b"Pages" | b"Outlines" | b"Outline" => {}
            // 将所有其他对象添加到doc中
            _ => {
                doc.objects.insert(object_id, object);
            }
        }
    }
    // 新建一个"Catalog"对象，将"Pages"对象添加到"Catalog"对象中，然后将"Catalog"对象添加到doc中
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    // 如果有书签没有关联到具体页面，将这些书签指向第一个页面
    doc.adjust_zero_pages();
    // 将书签添加到doc中
    if let Some(outline_id) = doc.build_outline() {
        if let Ok(Object::Dictionary(catalog_dict)) = doc.get_object_mut(catalog_id) {
            catalog_dict.set("Outlines", Object::Reference(outline_id));
        }
    }
    // 重新编号doc的对象
    doc.renumber_objects();

    doc.compress();

    doc.save(pdf_path)
        .wrap_err(format!("保存`{}`失败", pdf_path.display()))?;
    Ok(())
}
