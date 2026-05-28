use std::{collections::HashMap, io::Cursor, path::PathBuf};

use eyre::{OptionExt, WrapErr};
use image::ImageReader;
use regex_lite::{Captures, Regex};
use tauri::AppHandle;
use tracing::instrument;
use walkdir::WalkDir;

use crate::{
    extensions::{AppHandleExt, WalkDirEntryExt},
    types::Comic,
};

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

#[instrument(level = "error", skip_all)]
pub fn get_dimensions(img_data: &[u8]) -> eyre::Result<(u32, u32)> {
    let reader = ImageReader::new(Cursor::new(&img_data)).with_guessed_format()?;
    let dimensions = reader.into_dimensions()?;
    Ok(dimensions)
}

#[instrument(level = "error", skip_all)]
pub fn create_path_word_to_dir_map(app: &AppHandle) -> eyre::Result<HashMap<String, PathBuf>> {
    let mut path_word_to_dir_map: HashMap<String, PathBuf> = HashMap::new();
    let download_dir = app.get_config().read().download_dir.clone();
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
            std::fs::read_to_string(path).wrap_err(format!("读取`{}`失败", path.display()))?;
        let comic_json: serde_json::Value = serde_json::from_str(&metadata_str).wrap_err(
            format!("将`{}`反序列化为serde_json::Value失败", path.display()),
        )?;
        let path_word = comic_json
            .pointer("/comic/path_word")
            .and_then(|path_word| path_word.as_str())
            .ok_or_eyre(format!("`{}`没有`comic.path_word`字段", path.display()))?
            .to_string();

        let parent = path
            .parent()
            .ok_or_eyre(format!("`{}`没有父目录", path.display()))?;

        path_word_to_dir_map
            .entry(path_word)
            .or_insert(parent.to_path_buf());
    }

    Ok(path_word_to_dir_map)
}

#[instrument(level = "error", skip_all, fields(comic_path_word = comic_path_word))]
pub async fn get_comic(app: AppHandle, comic_path_word: &str) -> eyre::Result<Comic> {
    let copy_client = app.get_copy_client();

    let get_comic_resp_data = copy_client.get_comic(comic_path_word).await?;
    // TODO: 这里可以并发获取groups_chapters
    let mut groups_chapters = HashMap::new();
    for group_path_word in get_comic_resp_data.groups.keys() {
        let chapters = copy_client
            .get_group_chapters(comic_path_word, group_path_word)
            .await?;
        groups_chapters.insert(group_path_word.clone(), chapters);
    }
    let comic = Comic::from_resp_data(&app, get_comic_resp_data, groups_chapters)?;

    Ok(comic)
}

/// 预处理`fmt`中的`order`占位符
///
/// ### 功能描述
/// 标准的格式化(如`{order:0>4}`)会将宽度补齐应用于整个数字字符串
/// 当`order`为`5.1`时，标准输出为`05.1`(总长度4)
///
/// 本函数旨在实现**仅对整数部分补齐，小数部分追加在后**的效果
/// 当`order`为`5.1`时，本函数会将其转换为`0005.1`(整数补齐至4位，小数保留)
///
/// ### 处理流程
/// 1. **解析数值**：从`vars`中提取`order`，将其拆分为整数部分和小数部分
/// 2. **正则扫描**：使用正则查找模板中的`{order}`或`{order:xxx}`占位符，同时兼容`{{` 和 `}}`转义
/// 3. **自定义格式化**：
///    - 提取占位符中的格式参数(如`0>4`)
///    - 仅将该参数应用于整数部分
///    - 若存在非零小数部分，将其追加到格式化后的整数后面
/// 4. **原地替换**：将计算出的最终字符串(如 `0005.1`)直接替换掉原模板中的占位符
///
/// ### 示例
/// - 输入 fmt: `"{order:0>3} {chapter_title}"`, order: `"1.5"`
/// - 处理后 fmt: `"001.5 {chapter_title}"`
#[instrument(level = "error", skip_all, fields(fmt = fmt))]
pub fn preprocess_order_placeholder(
    fmt: &mut String,
    vars: &HashMap<String, String>,
) -> eyre::Result<()> {
    use strfmt::strfmt;

    let Some(order_str) = vars.get("order") else {
        return Ok(());
    };

    // 分离整数和小数
    let (int_part, frac_part) = match order_str.split_once('.') {
        Some((i, f)) => (i, f),
        None => (order_str.as_str(), ""),
    };
    let should_append_frac = !frac_part.is_empty() && frac_part != "0";

    // group 1: "{{" (转义左括号)
    // group 2: "}}" (转义右括号)
    // group 3: "{order...}" (真正的目标)
    // group 4: 冒号后的格式参数 (仅当 group 3 匹配时有效)
    let re = Regex::new(r"(\{\{)|(\}\})|(\{order(?::(.*?))?\})")?;

    // 执行替换
    let new_fmt = re.replace_all(fmt, |caps: &Captures| {
        // 遇到 {{，原样返回，消耗掉字符避免后续匹配误伤
        if caps.get(1).is_some() {
            return "{{".to_string();
        }
        // 遇到 }}，同理
        if caps.get(2).is_some() {
            return "}}".to_string();
        }
        // 匹配到了 {order...}
        // 此时 Group 4 是格式参数 (例如 "0>4")
        let fmt_spec = caps.get(4).map_or("", |m| m.as_str());

        // 构造临时模板 "{v:xxx}" 来格式化整数部分
        let int_fmt = format!("{{v:{fmt_spec}}}");
        let mut temp_vars = HashMap::new();
        temp_vars.insert("v".to_string(), int_part.to_string());

        let formatted_int = strfmt(&int_fmt, &temp_vars).unwrap_or(int_part.to_string());

        if should_append_frac {
            format!("{formatted_int}.{frac_part}")
        } else {
            formatted_int
        }
    });

    *fmt = new_fmt.to_string();

    Ok(())
}
