use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use eyre::{OptionExt, WrapErr};
use regex_lite::{Captures, Regex};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::{extensions::AppHandleExt, types::Comic, utils};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ChapterInfo {
    pub chapter_uuid: String,
    pub chapter_title: String,
    /// 此章节有多少页
    pub chapter_size: i64,
    pub comic_uuid: String,
    pub comic_title: String,
    pub comic_path_word: String,
    pub group_path_word: String,
    pub group_name: String,
    /// 此章节对应的group有多少章节
    pub group_size: i64,
    /// 此章节在group中的顺序
    pub order: f64,
    /// 漫画的连载状态
    pub comic_status: ComicStatus,
    /// 是否曾导出过PDF
    pub is_pdf_exported: bool,
    /// 是否曾导出过CBZ
    pub is_cbz_exported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_download_dir: Option<PathBuf>,
}

impl ChapterInfo {
    pub fn save_metadata(&self) -> eyre::Result<()> {
        let mut chapter_info = self.clone();
        // 将is_downloaded和chapter_download_dir字段设置为None
        // 这样能使这些字段在序列化时被忽略
        chapter_info.is_downloaded = None;
        chapter_info.chapter_download_dir = None;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;
        let metadata_path = chapter_download_dir.join("章节元数据.json");

        std::fs::create_dir_all(chapter_download_dir)
            .wrap_err(format!("创建目录`{}`失败", chapter_download_dir.display()))?;

        let chapter_json = serde_json::to_string_pretty(&chapter_info)
            .wrap_err("将ChapterInfo序列化为json失败")?;

        std::fs::write(&metadata_path, chapter_json)
            .wrap_err(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    pub fn get_temp_download_dir(&self) -> eyre::Result<PathBuf> {
        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        let chapter_download_dir_name = chapter_download_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_eyre(format!(
                "获取`{}`的目录名失败",
                chapter_download_dir.display()
            ))?;

        let parent = chapter_download_dir.parent().ok_or_eyre(format!(
            "`{}`的父目录不存在",
            chapter_download_dir.display()
        ))?;

        let temp_download_dir = parent.join(format!(".下载中-{chapter_download_dir_name}"));
        Ok(temp_download_dir)
    }

    pub fn get_chapter_relative_dir(&self, comic: &Comic) -> eyre::Result<PathBuf> {
        let comic_download_dir = comic
            .comic_download_dir
            .as_ref()
            .ok_or_eyre("`comic_download_dir`字段为`None`")?;

        let chapter_download_dir = self
            .chapter_download_dir
            .as_ref()
            .ok_or_eyre("`chapter_download_dir`字段为`None`")?;

        let relative_dir = chapter_download_dir
            .strip_prefix(comic_download_dir)
            .wrap_err(format!(
                "无法从路径`{}`中移除前缀`{}`",
                chapter_download_dir.display(),
                comic_download_dir.display()
            ))?;

        Ok(relative_dir.to_path_buf())
    }

    pub fn get_chapter_download_dir_by_fmt(
        app: &AppHandle,
        comic_download_dir: &Path,
        fmt_params: &ChapterDirFmtParams,
    ) -> eyre::Result<PathBuf> {
        use strfmt::strfmt;

        let json_value = serde_json::to_value(fmt_params)
            .wrap_err("将ChapterDirFmtParams转为serde_json::Value失败")?;

        let json_map = json_value
            .as_object()
            .ok_or_eyre("ChapterDirFmtParams不是JSON对象")?;

        let vars: HashMap<String, String> = json_map
            .into_iter()
            .map(|(k, v)| {
                let key = k.clone();
                let value = match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                };
                (key, value)
            })
            .collect();
        let mut chapter_dir_fmt = app.get_config().read().chapter_dir_fmt.clone();
        Self::preprocess_order_placeholder(&mut chapter_dir_fmt, &vars)
            .wrap_err("预处理`order`占位符失败")?;

        let dir_fmt_parts: Vec<&str> = chapter_dir_fmt.split('/').collect();

        let mut dir_names = Vec::new();
        for fmt in dir_fmt_parts {
            let dir_name = strfmt(fmt, &vars).wrap_err("格式化目录名失败")?;
            let dir_name = utils::filename_filter(&dir_name);
            if !dir_name.is_empty() {
                dir_names.push(dir_name);
            }
        }
        // 将格式化后的目录名拼接成完整的目录路径
        let mut chapter_download_dir = comic_download_dir.to_path_buf();
        for dir_name in dir_names {
            chapter_download_dir = chapter_download_dir.join(dir_name);
        }

        Ok(chapter_download_dir)
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
    fn preprocess_order_placeholder(
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
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub enum ComicStatus {
    #[default]
    Ongoing,
    Completed,
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ChapterDirFmtParams {
    pub comic_uuid: String,
    pub comic_path_word: String,
    pub comic_title: String,
    pub author: String,
    pub group_path_word: String,
    pub group_title: String,
    pub chapter_uuid: String,
    pub chapter_title: String,
    pub order: f64,
}
