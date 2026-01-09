use std::{
    collections::{BTreeMap, HashMap},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicU32, Arc},
};

use anyhow::{anyhow, Context};
use float_ord::FloatOrd;
use lopdf::{
    content::{Content, Operation},
    dictionary, Bookmark, Document, Object, Stream,
};
use parking_lot::Mutex;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tauri::AppHandle;
use tauri_specta::Event;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    events::{ExportCbzEvent, ExportPdfEvent},
    extensions::{AppHandleExt, PathIsImg},
    types::{ChapterInfo, Comic, ComicInfo},
    utils,
};

enum Archive {
    Cbz,
    Pdf,
}

impl Archive {
    fn extension(&self) -> &str {
        match self {
            Archive::Cbz => "cbz",
            Archive::Pdf => "pdf",
        }
    }
}

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

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
pub fn cbz(app: &AppHandle, comic: &Comic) -> anyhow::Result<()> {
    let comic_title = &comic.comic.name;
    let downloaded_chapters = get_downloaded_chapters(comic.comic.groups.clone());
    // 用于生成格式化的xml
    let xml_cfg = yaserde::ser::Config {
        perform_indent: true,
        ..Default::default()
    };
    let event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始导出cbz事件
    let _ = ExportCbzEvent::Start {
        uuid: event_uuid.clone(),
        comic_title: comic_title.clone(),
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

    let extension = Archive::Cbz.extension();
    let comic_export_dir = comic
        .get_comic_export_dir(app)
        .context(format!("`{comic_title}` 获取导出目录失败"))?;
    let cbz_export_dir = comic_export_dir.join(extension);

    // 并发处理
    let downloaded_chapters = downloaded_chapters.into_par_iter();
    downloaded_chapters.try_for_each(|chapter_info| -> anyhow::Result<()> {
        let chapter_title = &chapter_info.chapter_title;
        let group_name = &chapter_info.group_name;
        let err_prefix = format!("`{comic_title} - {group_name} - {chapter_title}`");
        // 生成ComicInfo
        let comic_info = ComicInfo::from(comic, &chapter_info);
        // 序列化ComicInfo为xml
        let comic_info_xml = yaserde::ser::to_string_with_config(&comic_info, &xml_cfg)
            .map_err(|err_msg| anyhow!("{err_prefix} 序列化`ComicInfo.xml`失败: {err_msg}"))?;
        // 创建cbz文件
        let chapter_download_dir = chapter_info
            .chapter_download_dir
            .as_ref()
            .context(format!("{err_prefix} `chapter_download_dir`字段为`None`"))?;
        let chapter_download_dir_name = chapter_download_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context(format!(
                "{err_prefix} 获取`{}`的目录名失败",
                chapter_download_dir.display()
            ))?;
        let chapter_relative_dir = chapter_info
            .get_chapter_relative_dir(comic)
            .context(format!("{err_prefix} 获取章节相对目录失败"))?;
        let chapter_relative_dir_parent = chapter_relative_dir.parent().context(format!(
            "{err_prefix} `{}`没有父目录",
            chapter_relative_dir.display()
        ))?;
        let chapter_export_dir = cbz_export_dir.join(chapter_relative_dir_parent);
        // 保证导出目录存在
        std::fs::create_dir_all(&chapter_export_dir).context(format!(
            "{err_prefix} 创建目录`{}`失败",
            chapter_export_dir.display()
        ))?;
        let zip_path = chapter_export_dir.join(format!("{chapter_download_dir_name}.{extension}"));
        let zip_file = std::fs::File::create(&zip_path)
            .context(format!("{err_prefix} 创建文件`{}`失败", zip_path.display()))?;
        let mut zip_writer = ZipWriter::new(zip_file);
        // 把ComicInfo.xml写入cbz
        zip_writer
            .start_file("ComicInfo.xml", SimpleFileOptions::default())
            .context(format!(
                "{err_prefix} 在`{}`创建`ComicInfo.xml`失败",
                zip_path.display()
            ))?;
        zip_writer
            .write_all(comic_info_xml.as_bytes())
            .context(format!("{err_prefix} 写入`ComicInfo.xml`失败"))?;

        let image_paths = get_image_paths(chapter_download_dir).context(format!(
            "{err_prefix} 获取`{}`中的图片失败",
            chapter_download_dir.display()
        ))?;

        for image_path in image_paths {
            let filename = image_path
                .file_name()
                .and_then(|name| name.to_str())
                .context(format!(
                    "{err_prefix} 获取`{}`的目录名失败",
                    chapter_download_dir.display()
                ))?;
            // 将文件写入cbz
            zip_writer
                .start_file(filename, SimpleFileOptions::default())
                .context(format!(
                    "{err_prefix} 在`{}`创建`{filename:?}`失败",
                    zip_path.display()
                ))?;
            let mut file = std::fs::File::open(&image_path)
                .context(format!("{err_prefix} 打开`{}`失败", image_path.display()))?;
            std::io::copy(&mut file, &mut zip_writer).context(format!(
                "{err_prefix} 将`{}`写入`{}`失败",
                image_path.display(),
                zip_path.display()
            ))?;
        }

        zip_writer
            .finish()
            .context(format!("{err_prefix} 关闭`{}`失败", zip_path.display()))?;
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
        chapter_export_dir: cbz_export_dir,
    }
    .emit(app);

    Ok(())
}

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

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_lines)]
pub fn pdf(app: &AppHandle, comic: &Comic) -> anyhow::Result<()> {
    let comic_title = &comic.comic.name;
    let downloaded_chapters = get_downloaded_chapters(comic.comic.groups.clone());
    let create_event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始创建pdf事件
    let _ = ExportPdfEvent::CreateStart {
        uuid: create_event_uuid.clone(),
        comic_title: comic_title.clone(),
        total: downloaded_chapters.len() as u32,
    }
    .emit(app);
    // 如果success为false，drop时发送CreateError事件
    let mut create_error_event_guard = PdfCreateErrorEventGuard {
        uuid: create_event_uuid.clone(),
        app: app.clone(),
        success: false,
    };
    // 用来记录创建pdf的进度
    let created_count = Arc::new(AtomicU32::new(0));

    let extension = Archive::Pdf.extension();
    let comic_export_dir = comic
        .get_comic_export_dir(app)
        .context(format!("`{comic_title}` 获取导出目录失败"))?;
    let pdf_export_dir = comic_export_dir.join(extension);
    // 章节和他们对应的pdf路径
    let chapter_and_pdf_path_pairs = Mutex::new(Vec::new());
    // 并发处理
    let create_pdf_concurrency = app.get_config().read().create_pdf_concurrency;
    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(create_pdf_concurrency)
        .build()
        .context("rayon线程池创建失败")?;

    thread_pool.install(|| {
        let downloaded_chapters = downloaded_chapters.into_par_iter();
        downloaded_chapters.try_for_each(|chapter_info| -> anyhow::Result<()> {
            let chapter_title = &chapter_info.chapter_title;
            let group_name = &chapter_info.group_name;
            let err_prefix = format!("`{comic_title} - {group_name} - {chapter_title}`");
            // 创建pdf文件
            let chapter_download_dir = chapter_info
                .chapter_download_dir
                .as_ref()
                .context(format!("{err_prefix} `chapter_download_dir`字段为`None`"))?;
            let chapter_download_dir_name = chapter_download_dir
                .file_name()
                .and_then(|name| name.to_str())
                .context(format!(
                    "{err_prefix} 获取`{}`的目录名失败",
                    chapter_download_dir.display()
                ))?;
            let chapter_relative_dir = chapter_info
                .get_chapter_relative_dir(comic)
                .context(format!("{err_prefix} 获取章节相对目录失败"))?;
            let chapter_relative_dir_parent = chapter_relative_dir.parent().context(format!(
                "{err_prefix} `{}`没有父目录",
                chapter_relative_dir.display()
            ))?;
            let chapter_export_dir = pdf_export_dir.join(chapter_relative_dir_parent);
            // 保证导出目录存在
            std::fs::create_dir_all(&chapter_export_dir).context(format!(
                "{err_prefix} 创建目录`{}`失败",
                chapter_export_dir.display()
            ))?;

            let pdf_path =
                chapter_export_dir.join(format!("{chapter_download_dir_name}.{extension}"));

            let image_paths = get_image_paths(chapter_download_dir).context(format!(
                "{err_prefix} 获取`{}`中的图片失败",
                chapter_download_dir.display()
            ))?;

            create_pdf(image_paths, &pdf_path).context(format!("{err_prefix} 创建pdf失败"))?;

            chapter_and_pdf_path_pairs
                .lock()
                .push((chapter_info, pdf_path));
            // 更新创建pdf的进度
            let current = created_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            // 发送创建pdf进度事件
            let _ = ExportPdfEvent::CreateProgress {
                uuid: create_event_uuid.clone(),
                current,
            }
            .emit(app);
            Ok(())
        })
    })?;
    // 标记为成功，后面drop时就不会发送CreateError事件
    create_error_event_guard.success = true;
    // 发送创建pdf完成事件
    let _ = ExportPdfEvent::CreateEnd {
        uuid: create_event_uuid,
        chapter_export_dir: pdf_export_dir.clone(),
    }
    .emit(app);

    let enable_merge_pdf = app.get_config().read().enable_merge_pdf;
    if !enable_merge_pdf {
        return Ok(());
    }

    let mut chapter_and_pdf_path_pairs = std::mem::take(&mut *chapter_and_pdf_path_pairs.lock());
    chapter_and_pdf_path_pairs.sort_by_key(|(chapter_info, _)| FloatOrd(chapter_info.order));
    let chapter_pdf_paths: Vec<PathBuf> = chapter_and_pdf_path_pairs
        .into_iter()
        .map(|(_, pdf_path)| pdf_path)
        .collect();

    let mut chapter_export_dir_to_pdf_paths = HashMap::new();
    for chapter_pdf_path in chapter_pdf_paths {
        let Some(chapter_export_dir) = chapter_pdf_path.parent() else {
            continue;
        };
        if chapter_export_dir == pdf_export_dir {
            continue;
        }
        chapter_export_dir_to_pdf_paths
            .entry(chapter_export_dir.to_path_buf())
            .or_insert_with(Vec::new)
            .push(chapter_pdf_path);
    }

    let merge_event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始合并pdf事件
    let _ = ExportPdfEvent::MergeStart {
        uuid: merge_event_uuid.clone(),
        comic_title: comic_title.clone(),
        total: chapter_export_dir_to_pdf_paths.len() as u32,
    }
    .emit(app);
    // 如果success为false，drop时发送MergeError事件
    let mut merge_error_event_guard = PdfMergeErrorEventGuard {
        uuid: merge_event_uuid.clone(),
        app: app.clone(),
        success: false,
    };
    // 合并PDF很吃内存，为了减少爆内存的发生，不使用并发处理，而是逐个合并
    for (i, entry) in chapter_export_dir_to_pdf_paths.into_iter().enumerate() {
        let (chapter_export_dir, chapter_pdf_paths) = entry;
        let pdf_dir_name = chapter_export_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context(format!(
                "`{comic_title}` 获取`{}`的目录名失败",
                chapter_export_dir.display()
            ))?;
        let parent = chapter_export_dir.parent().context(format!(
            "`{comic_title}` `{}`没有父目录",
            chapter_export_dir.display()
        ))?;
        let pdf_path = parent.join(format!("{pdf_dir_name}.{extension}"));
        // 合并pdf
        merge_pdf_file(chapter_pdf_paths, &pdf_path)
            .context(format!("`{comic_title}` `{pdf_dir_name}`合并pdf失败"))?;
        // 发送合并pdf进度事件
        let _ = ExportPdfEvent::MergeProgress {
            uuid: merge_event_uuid.clone(),
            current: (i + 1) as u32,
        }
        .emit(app);
    }
    // 标记为成功，后面drop时就不会发送MergeError事件
    merge_error_event_guard.success = true;
    // 发送合并pdf完成事件
    let _ = ExportPdfEvent::MergeEnd {
        uuid: merge_event_uuid,
        chapter_export_dir: pdf_export_dir.clone(),
    }
    .emit(app);
    Ok(())
}

/// 用`image_paths`中的图片创建PDF文件，保存到`pdf_path`
#[allow(clippy::similar_names)]
#[allow(clippy::cast_possible_truncation)]
fn create_pdf(image_paths: Vec<PathBuf>, pdf_path: &Path) -> anyhow::Result<()> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = vec![];

    for image_path in image_paths {
        if !image_path.is_file() {
            continue;
        }

        let buffer = read_image_to_buffer(&image_path)
            .context(format!("将`{}`读取到buffer失败", image_path.display()))?;
        let (width, height) = utils::get_dimensions(&buffer)
            .context(format!("获取`{}`的尺寸失败", image_path.display()))?;
        let image_stream = lopdf::xobject::image_from(buffer)
            .context(format!("创建`{}`的图片流失败", image_path.display()))?;
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
        .context(format!("保存`{}`失败", pdf_path.display()))?;
    Ok(())
}

/// 读取`image_path`中的图片数据到buffer中
fn read_image_to_buffer(image_path: &Path) -> anyhow::Result<Vec<u8>> {
    let file =
        std::fs::File::open(image_path).context(format!("打开`{}`失败", image_path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = vec![];
    reader
        .read_to_end(&mut buffer)
        .context(format!("读取`{}`失败", image_path.display()))?;
    Ok(buffer)
}

/// 将`pdf_dir`中的PDF合并到`pdf_path`中
#[allow(clippy::cast_possible_truncation)]
fn merge_pdf_file(chapter_pdf_paths: Vec<PathBuf>, pdf_path: &Path) -> anyhow::Result<()> {
    let mut doc = Document::with_version("1.5");
    let mut doc_page_ids = vec![];
    let mut doc_objects = BTreeMap::new();

    for chapter_pdf_path in chapter_pdf_paths {
        let mut chapter_doc = Document::load(&chapter_pdf_path)
            .context(format!("加载`{}`失败", chapter_pdf_path.display()))?;
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
                    .context(format!("获取`{}`的文件名失败", chapter_pdf_path.display()))?
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
        .context(format!("保存`{}`失败", pdf_path.display()))?;
    Ok(())
}

/// 获取已下载的章节
fn get_downloaded_chapters(groups: HashMap<String, Vec<ChapterInfo>>) -> Vec<ChapterInfo> {
    groups
        .into_iter()
        .flat_map(|(_, chapters)| chapters)
        .filter(|chapter| chapter.is_downloaded.unwrap_or(false))
        .collect()
}

fn get_image_paths(images_dir: &Path) -> Result<Vec<PathBuf>, anyhow::Error> {
    let mut image_paths: Vec<PathBuf> = std::fs::read_dir(images_dir)
        .context(format!("读取目录`{}`失败", images_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_img())
        .collect();
    image_paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(image_paths)
}
