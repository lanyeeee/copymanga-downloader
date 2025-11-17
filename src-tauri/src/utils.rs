use std::{collections::HashMap, io::Cursor, path::PathBuf};

use anyhow::Context;
use image::ImageReader;
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use walkdir::WalkDir;

use crate::{config::Config, extensions::WalkDirEntryExt};

pub fn filename_filter(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '/' => ' ',
            ':' => '：',
            '*' => '⭐',
            '?' => '？',
            '"' => '\'',
            '<' => '《',
            '>' => '》',
            '|' => '丨',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn get_dimensions(img_data: &[u8]) -> anyhow::Result<(u32, u32)> {
    let reader = ImageReader::new(Cursor::new(&img_data)).with_guessed_format()?;
    let dimensions = reader.into_dimensions()?;
    Ok(dimensions)
}

pub fn create_path_word_to_dir_map(app: &AppHandle) -> anyhow::Result<HashMap<String, PathBuf>> {
    let mut path_word_to_dir_map: HashMap<String, PathBuf> = HashMap::new();
    let download_dir = app.state::<RwLock<Config>>().read().download_dir.clone();
    if !download_dir.exists() {
        return Ok(path_word_to_dir_map);
    }

    for entry in WalkDir::new(&download_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.is_comic_metadata() {
            continue;
        }

        let metadata_str =
            std::fs::read_to_string(path).context(format!("读取`{}`失败", path.display()))?;
        let comic_json: serde_json::Value = serde_json::from_str(&metadata_str).context(
            format!("将`{}`反序列化为serde_json::Value失败", path.display()),
        )?;
        let path_word = comic_json
            .pointer("/comic/path_word")
            .and_then(|path_word| path_word.as_str())
            .context(format!("`{}`没有`comic.path_word`字段", path.display()))?
            .to_string();

        let parent = path
            .parent()
            .context(format!("`{}`没有父目录", path.display()))?;

        path_word_to_dir_map
            .entry(path_word)
            .or_insert(parent.to_path_buf());
    }

    Ok(path_word_to_dir_map)
}
