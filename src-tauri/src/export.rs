use std::{io::Write, path::PathBuf};

use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    config::Config,
    types::{ChapterInfo, Comic, ComicInfo},
};

#[allow(clippy::cast_possible_wrap)]
pub fn cbz(app: &AppHandle, comic: Comic) -> anyhow::Result<()> {
    // 获取已下载的章节
    let downloaded_chapters = comic
        .comic
        .groups
        .into_iter()
        .flat_map(|(_, chapters)| chapters)
        .filter(|chapter| chapter.is_downloaded.unwrap_or(false))
        .collect::<Vec<_>>();
    // 生成格式化的xml
    let cfg = yaserde::ser::Config {
        perform_indent: true,
        ..Default::default()
    };
    // TODO: 使用Rayon并行处理以提高效率
    for chapter_info in downloaded_chapters {
        let chapter_title = chapter_info.chapter_title.clone();
        let prefixed_chapter_title = chapter_info.prefixed_chapter_title.clone();
        let group_name = chapter_info.group_name.clone();
        let download_dir = get_download_dir(app, &chapter_info);
        let export_dir = get_export_dir(app, &chapter_info);
        let comic_info_path = export_dir.join("ComicInfo.xml");
        // 生成ComicInfo
        let comic_info = ComicInfo::from(
            chapter_info,
            &comic.comic.author,
            &comic.comic.theme,
            comic.comic.brief.clone(),
        );
        // 序列化ComicInfo为xml
        let comic_info_xml =
            yaserde::ser::to_string_with_config(&comic_info, &cfg).map_err(|err_msg| {
                anyhow!("{group_name} - {chapter_title} 序列化 {comic_info_path:?} 失败: {err_msg}")
            })?;
        // 保证导出目录存在
        std::fs::create_dir_all(&export_dir).context(format!(
            "{group_name} - {chapter_title} 创建目录 {export_dir:?} 失败"
        ))?;
        // 创建cbz文件
        let zip_path = export_dir.join(format!("{prefixed_chapter_title}.cbz"));
        let zip_file = std::fs::File::create(&zip_path).context(format!(
            "{group_name} - {chapter_title} 创建文件 {zip_path:?} 失败"
        ))?;
        let mut zip_writer = ZipWriter::new(zip_file);
        // 把ComicInfo.xml写入cbz
        zip_writer
            .start_file("ComicInfo.xml", SimpleFileOptions::default())
            .context(format!(
                "{group_name} - {chapter_title} 在 {zip_path:?} 创建 ComicInfo.xml 失败"
            ))?;
        zip_writer
            .write_all(comic_info_xml.as_bytes())
            .context("{group_name} - {chapter_title} 写入 ComicInfo.xml 失败")?;
        // 遍历下载目录，将文件写入cbz
        let entries = std::fs::read_dir(&download_dir)
            .context(format!(
                "{group_name} - {chapter_title} 读取目录 {download_dir:?} 失败"
            ))?
            .filter_map(Result::ok);
        for entry in entries {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => continue,
            };
            // 将文件写入cbz
            zip_writer
                .start_file(&filename, SimpleFileOptions::default())
                .context(format!(
                    "{group_name} - {chapter_title} 在 {zip_path:?} 创建 {filename:?} 失败"
                ))?;
            let mut file = std::fs::File::open(&path).context(format!("打开 {path:?} 失败"))?;
            std::io::copy(&mut file, &mut zip_writer).context(format!(
                "{group_name} - {chapter_title} 将 {path:?} 写入 {zip_path:?} 失败"
            ))?;
        }

        zip_writer.finish().context(format!(
            "{group_name} - {chapter_title} 关闭 {zip_path:?} 失败"
        ))?;
    }

    Ok(())
}

fn get_export_dir(app: &AppHandle, chapter_info: &ChapterInfo) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .export_dir
        .join(&chapter_info.comic_title)
        .join(&chapter_info.group_name)
}

fn get_download_dir(app: &AppHandle, chapter_info: &ChapterInfo) -> PathBuf {
    app.state::<RwLock<Config>>()
        .read()
        .download_dir
        .join(&chapter_info.comic_title)
        .join(&chapter_info.group_name)
        .join(&chapter_info.prefixed_chapter_title)
}
