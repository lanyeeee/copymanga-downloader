use anyhow::{anyhow, Context};
use lopdf::{
    content::{Content, Operation},
    dictionary, Bookmark, Document, Object, Stream,
};
use parking_lot::RwLock;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashMap;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicU32, Arc},
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    config::Config,
    events::{ExportCbzEvent, ExportPdfEvent},
    types::{ChapterInfo, Comic, ComicInfo},
    utils,
};

enum Archive {
    Cbz,
    Pdf,
}
impl Archive {
    pub fn extension(&self) -> &str {
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
pub fn cbz(app: &AppHandle, comic: Comic) -> anyhow::Result<()> {
    let comic_title = &comic.comic.name.clone();
    let downloaded_chapters = get_downloaded_chapters(comic.comic.groups);
    // 生成格式化的xml
    let cfg = yaserde::ser::Config {
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
    // 并发处理
    let downloaded_chapters = downloaded_chapters.into_par_iter();
    downloaded_chapters.try_for_each(|chapter_info| -> anyhow::Result<()> {
        let chapter_title = chapter_info.chapter_title.clone();
        let prefixed_chapter_title = chapter_info.prefixed_chapter_title.clone();
        let group_name = chapter_info.group_name.clone();
        let err_prefix = format!("{group_name} - {chapter_title}");
        let chapter_download_dir = get_chapter_download_dir(app, &chapter_info);
        let chapter_export_dir = get_chapter_export_dir(app, &chapter_info, &Archive::Cbz);
        let comic_info_path = chapter_export_dir.join("ComicInfo.xml");
        // 生成ComicInfo
        let comic_info = ComicInfo::from(
            chapter_info,
            &comic.comic.author,
            &comic.comic.theme,
            comic.comic.brief.clone(),
        );
        // 序列化ComicInfo为xml
        let comic_info_xml = yaserde::ser::to_string_with_config(&comic_info, &cfg)
            .map_err(|err_msg| anyhow!("{err_prefix}序列化`{comic_info_path:?}`失败: {err_msg}"))?;
        // 保证导出目录存在
        std::fs::create_dir_all(&chapter_export_dir)
            .context(format!("{err_prefix}创建目录`{chapter_export_dir:?}`失败"))?;
        // 创建cbz文件
        let extension = Archive::Cbz.extension();
        let zip_path = chapter_export_dir.join(format!("{prefixed_chapter_title}.{extension}"));
        let zip_file = std::fs::File::create(&zip_path)
            .context(format!("{err_prefix}创建文件`{zip_path:?}`失败"))?;
        let mut zip_writer = ZipWriter::new(zip_file);
        // 把ComicInfo.xml写入cbz
        zip_writer
            .start_file("ComicInfo.xml", SimpleFileOptions::default())
            .context(format!(
                "{err_prefix}在`{zip_path:?}`创建`ComicInfo.xml`失败"
            ))?;
        zip_writer
            .write_all(comic_info_xml.as_bytes())
            .context(format!("{err_prefix}写入`ComicInfo.xml`失败"))?;
        // 遍历下载目录，将文件写入cbz
        let image_entries = std::fs::read_dir(&chapter_download_dir)
            .context(format!(
                "{err_prefix}读取目录`{chapter_download_dir:?}`失败"
            ))?
            .filter_map(Result::ok);
        for image_entry in image_entries {
            let image_path = image_entry.path();
            if !image_path.is_file() {
                continue;
            }

            let filename = match image_path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => continue,
            };
            // 将文件写入cbz
            zip_writer
                .start_file(&filename, SimpleFileOptions::default())
                .context(format!(
                    "{err_prefix}在`{zip_path:?}`创建`{filename:?}`失败"
                ))?;
            let mut file =
                std::fs::File::open(&image_path).context(format!("打开`{image_path:?}`失败"))?;
            std::io::copy(&mut file, &mut zip_writer).context(format!(
                "{err_prefix}将`{image_path:?}`写入`{zip_path:?}`失败"
            ))?;
        }

        zip_writer
            .finish()
            .context(format!("{err_prefix}关闭`{zip_path:?}`失败"))?;
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
    let _ = ExportCbzEvent::End { uuid: event_uuid }.emit(app);

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
pub fn pdf(app: &AppHandle, comic: Comic) -> anyhow::Result<()> {
    let comic_title = comic.comic.name.clone();
    let downloaded_chapters = get_downloaded_chapters(comic.comic.groups);
    let event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始创建pdf事件
    let _ = ExportPdfEvent::CreateStart {
        uuid: event_uuid.clone(),
        comic_title: comic_title.clone(),
        total: downloaded_chapters.len() as u32,
    }
    .emit(app);
    // 如果success为false，drop时发送CreateError事件
    let mut create_error_event_guard = PdfCreateErrorEventGuard {
        uuid: event_uuid.clone(),
        app: app.clone(),
        success: false,
    };
    // 用来记录创建pdf的进度
    let current = Arc::new(AtomicU32::new(0));
    // 并发处理
    let downloaded_chapters = downloaded_chapters.into_par_iter();
    downloaded_chapters.try_for_each(|chapter_info| -> anyhow::Result<()> {
        let chapter_download_dir = get_chapter_download_dir(app, &chapter_info);
        let chapter_export_dir = get_chapter_export_dir(app, &chapter_info, &Archive::Pdf);
        let group_name = chapter_info.group_name;
        let chapter_title = chapter_info.chapter_title;
        let prefixed_chapter_title = chapter_info.prefixed_chapter_title;
        // 保证导出目录存在
        std::fs::create_dir_all(&chapter_export_dir).context(format!(
            "`{group_name} - {chapter_title}`创建目录`{chapter_export_dir:?}`失败"
        ))?;
        // 创建pdf
        let extension = Archive::Pdf.extension();
        let pdf_path = chapter_export_dir.join(format!("{prefixed_chapter_title}.{extension}"));
        create_pdf(&chapter_download_dir, &pdf_path)
            .context(format!("`{group_name} - {chapter_title}`创建pdf失败"))?;
        // 更新创建pdf的进度
        let current = current.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        // 发送创建pdf进度事件
        let _ = ExportPdfEvent::CreateProgress {
            uuid: event_uuid.clone(),
            current,
        }
        .emit(app);
        Ok(())
    })?;
    // 标记为成功，后面drop时就不会发送CreateError事件
    create_error_event_guard.success = true;
    // 发送创建pdf完成事件
    let _ = ExportPdfEvent::CreateEnd { uuid: event_uuid }.emit(app);

    let group_export_dir = get_group_export_dir(app, &comic_title, &Archive::Pdf);
    let chapter_export_dirs = std::fs::read_dir(&group_export_dir)
        .context(format!("读取目录`{group_export_dir:?}`失败"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    let event_uuid = uuid::Uuid::new_v4().to_string();
    // 发送开始合并pdf事件
    let _ = ExportPdfEvent::MergeStart {
        uuid: event_uuid.clone(),
        comic_title: comic_title.clone(),
        total: chapter_export_dirs.len() as u32,
    }
    .emit(app);
    // 如果success为false，drop时发送MergeError事件
    let mut merge_error_event_guard = PdfMergeErrorEventGuard {
        uuid: event_uuid.clone(),
        app: app.clone(),
        success: false,
    };
    // 合并PDF很吃内存，为了减少爆内存的发生，不使用并发处理，而是逐个合并
    for (i, chapter_export_dir) in chapter_export_dirs.iter().enumerate() {
        let group_name = chapter_export_dir
            .file_name()
            .context(format!(
                "获取`{chapter_export_dir:?}`的目录名失败，请确保路径不是以`..`结尾"
            ))?
            .to_str()
            .context(format!(
                "获取`{chapter_export_dir:?}`的目录名失败，包含非法字符"
            ))?;
        let extension = Archive::Pdf.extension();
        let pdf_path = group_export_dir.join(format!("{group_name}.{extension}"));
        // 合并pdf
        merge_pdf(chapter_export_dir, &pdf_path).context(format!("`{group_name}`合并pdf失败"))?;
        // 发送合并pdf进度事件
        let _ = ExportPdfEvent::MergeProgress {
            uuid: event_uuid.clone(),
            current: (i + 1) as u32,
        }
        .emit(app);
    }
    // 标记为成功，后面drop时就不会发送MergeError事件
    merge_error_event_guard.success = true;
    // 发送合并pdf完成事件
    let _ = ExportPdfEvent::MergeEnd { uuid: event_uuid }.emit(app);
    Ok(())
}

/// 用`chapter_download_dir`中的图片创建PDF，保存到`pdf_path`中
#[allow(clippy::similar_names)]
#[allow(clippy::cast_possible_truncation)]
fn create_pdf(chapter_download_dir: &Path, pdf_path: &Path) -> anyhow::Result<()> {
    let mut image_paths = std::fs::read_dir(chapter_download_dir)
        .context(format!("读取目录`{chapter_download_dir:?}`失败"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    image_paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = vec![];

    for image_path in image_paths {
        if !image_path.is_file() {
            continue;
        }

        let buffer = read_image_to_buffer(&image_path)
            .context(format!("将`{image_path:?}`读取到buffer失败"))?;
        let (width, height) =
            utils::get_dimensions(&buffer).context(format!("获取`{image_path:?}`的尺寸失败"))?;
        let image_stream = lopdf::xobject::image_from(buffer)
            .context(format!("创建`{image_path:?}`的图片流失败"))?;
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
        .context(format!("保存`{pdf_path:?}`失败"))?;
    Ok(())
}

/// 读取`image_path`中的图片数据到buffer中
fn read_image_to_buffer(image_path: &Path) -> anyhow::Result<Vec<u8>> {
    let file = std::fs::File::open(image_path).context(format!("打开`{image_path:?}`失败"))?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = vec![];
    reader
        .read_to_end(&mut buffer)
        .context(format!("读取`{image_path:?}`失败"))?;
    Ok(buffer)
}

/// 合并`chapter_export_dir`中的PDF，保存到`pdf_path`中
#[allow(clippy::cast_possible_truncation)]
fn merge_pdf(chapter_export_dir: &Path, pdf_path: &Path) -> anyhow::Result<()> {
    let mut chapter_pdf_paths = std::fs::read_dir(chapter_export_dir)
        .context(format!("读取目录`{chapter_export_dir:?}`失败"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    // 按照目录名中的浮点数索引进行排序
    chapter_pdf_paths.sort_by(|a, b| {
        let get_index = |path: &PathBuf| -> f64 {
            // 获取文件名
            let Some(file_name) = path.file_name() else {
                return f64::MAX;
            };
            // 转换为字符串
            let Some(name_str) = file_name.to_str() else {
                return f64::MAX;
            };
            // 获取第一个空格前的内容作为索引字符串
            let Some(index_str) = name_str.split_whitespace().next() else {
                return f64::MAX;
            };
            // 将字符串解析为浮点数
            index_str.parse::<f64>().unwrap_or(f64::MAX)
        };

        // 将 f64 转换为可以比较的数据类型
        let index_a = get_index(a);
        let index_b = get_index(b);

        index_a
            .partial_cmp(&index_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut doc = Document::with_version("1.5");
    let mut doc_page_ids = vec![];
    let mut doc_objects = BTreeMap::new();

    for chapter_pdf_path in chapter_pdf_paths {
        let mut chapter_doc =
            Document::load(&chapter_pdf_path).context(format!("加载`{chapter_pdf_path:?}`失败"))?;
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
                    .context(format!(
                        "获取`{chapter_pdf_path:?}`的文件名失败，没有文件名"
                    ))?
                    .to_str()
                    .context(format!(
                        "获取`{chapter_pdf_path:?}`的文件名失败，包含非法字符"
                    ))?
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
        .context(format!("保存`{pdf_path:?}`失败"))?;
    Ok(())
}

fn get_chapter_export_dir(
    app: &AppHandle,
    chapter_info: &ChapterInfo,
    archive: &Archive,
) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .export_dir
        .join(&chapter_info.comic_title)
        .join(archive.extension())
        .join(&chapter_info.group_name)
}

fn get_group_export_dir(app: &AppHandle, comic_title: &str, archive: &Archive) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .export_dir
        .join(comic_title)
        .join(archive.extension())
}

fn get_chapter_download_dir(app: &AppHandle, chapter_info: &ChapterInfo) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .download_dir
        .join(&chapter_info.comic_title)
        .join(&chapter_info.group_name)
        .join(&chapter_info.prefixed_chapter_title)
}

/// 获取已下载的章节
fn get_downloaded_chapters(groups: HashMap<String, Vec<ChapterInfo>>) -> Vec<ChapterInfo> {
    groups
        .into_iter()
        .flat_map(|(_, chapters)| chapters)
        .filter(|chapter| chapter.is_downloaded.unwrap_or(false))
        .collect::<Vec<_>>()
}
