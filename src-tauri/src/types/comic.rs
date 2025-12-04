use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use walkdir::WalkDir;

use crate::{
    extensions::{AppHandleExt, WalkDirEntryExt},
    responses::{
        AuthorRespData, ChapterInGetChaptersRespData, GetComicRespData, GroupRespData,
        LabeledValueRespData, LastChapterRespData, ThemeRespData,
    },
    types::{ChapterInfo, ComicStatus},
    utils,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
#[allow(clippy::struct_field_names)]
pub struct Comic {
    #[serde(rename = "is_banned")]
    pub is_banned: bool,
    #[serde(rename = "is_lock")]
    pub is_lock: bool,
    #[serde(rename = "is_login")]
    pub is_login: bool,
    #[serde(rename = "is_mobile_bind")]
    pub is_mobile_bind: bool,
    #[serde(rename = "is_vip")]
    pub is_vip: bool,
    pub comic: ComicDetail,
    pub popular: i64,
    pub groups: HashMap<String, Group>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_downloaded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comic_download_dir: Option<PathBuf>,
}
impl Comic {
    pub fn from_resp_data(
        app: &AppHandle,
        comic_resp_data: GetComicRespData,
        groups_chapters: HashMap<String, Vec<ChapterInGetChaptersRespData>>,
    ) -> anyhow::Result<Comic> {
        let is_banned = comic_resp_data.is_banned;
        let is_lock = comic_resp_data.is_lock;
        let is_login = comic_resp_data.is_login;
        let is_mobile_bind = comic_resp_data.is_mobile_bind;
        let is_vip = comic_resp_data.is_vip;
        let popular = comic_resp_data.popular;
        let groups = Group::from(comic_resp_data.groups.clone());
        let comic = ComicDetail::from_resp_data(comic_resp_data, groups_chapters);

        let mut comic = Comic {
            is_banned,
            is_lock,
            is_login,
            is_mobile_bind,
            is_vip,
            comic,
            popular,
            groups,
            is_downloaded: None,
            comic_download_dir: None,
        };

        let path_word_to_dir_map =
            utils::create_path_word_to_dir_map(app).context("创建漫画路径词到下载目录映射失败")?;

        // TODO: 这是为了兼容v0.10.2及之前的版本，后续需要移除，计划在v0.12.0之后移除
        if let Some(comic_download_dir) = path_word_to_dir_map.get(&comic.comic.path_word) {
            comic
                .create_chapter_metadata_for_old_version(comic_download_dir)
                .context("为旧版本创建章节元数据失败")?;
        }

        comic
            .update_fields(&path_word_to_dir_map)
            .context(format!("`{}`更新Comic的字段失败", comic.comic.name))?;

        Ok(comic)
    }

    pub fn from_metadata(metadata_path: &Path) -> anyhow::Result<Comic> {
        let comic_json = std::fs::read_to_string(metadata_path).context(format!(
            "从元数据转为Comic失败，读取元数据文件`{}`失败",
            metadata_path.display()
        ))?;
        let mut comic = serde_json::from_str::<Comic>(&comic_json).context(format!(
            "从元数据转为Comic失败，将`{}`反序列化为Comic失败",
            metadata_path.display()
        ))?;
        let parent = metadata_path
            .parent()
            .context(format!("`{}`没有父目录", metadata_path.display()))?;
        let comic_download_dir = parent.to_path_buf();

        // TODO: 这是为了兼容v0.10.2及之前的版本，后续需要移除，计划在v0.12.0之后移除
        comic
            .create_chapter_metadata_for_old_version(&comic_download_dir)
            .context("为旧版本创建章节元数据失败")?;

        comic.comic_download_dir = Some(comic_download_dir);
        comic.is_downloaded = Some(true);

        // 来自元数据的章节信息没有`chapter_download_dir`和`is_downloaded`字段，需要更新
        comic
            .update_chapter_infos_fields()
            .context("更新章节信息字段失败")?;

        Ok(comic)
    }

    pub fn update_fields(
        &mut self,
        path_word_to_dir_map: &HashMap<String, PathBuf>,
    ) -> anyhow::Result<()> {
        if let Some(comic_download_dir) = path_word_to_dir_map.get(&self.comic.path_word) {
            self.comic_download_dir = Some(comic_download_dir.clone());
            self.is_downloaded = Some(true);

            self.update_chapter_infos_fields()
                .context("更新章节信息字段失败")?;
        }
        Ok(())
    }

    fn update_chapter_infos_fields(&mut self) -> anyhow::Result<()> {
        let Some(comic_download_dir) = &self.comic_download_dir else {
            return Err(anyhow!("`comic_download_dir`字段为`None`"));
        };

        if !comic_download_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(comic_download_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.is_chapter_metadata() {
                continue;
            }

            let metadata_path = entry.path();

            let metadata_str = std::fs::read_to_string(metadata_path)
                .context(format!("读取`{}`失败", metadata_path.display()))?;

            let chapter_json: serde_json::Value =
                serde_json::from_str(&metadata_str).context(format!(
                    "将`{}`反序列化为serde_json::Value失败",
                    metadata_path.display()
                ))?;

            let chapter_uuid = chapter_json
                .get("chapterUuid")
                .and_then(|uuid| uuid.as_str())
                .context(format!(
                    "`{}`没有`chapterUuid`字段",
                    metadata_path.display()
                ))?
                .to_string();

            let group_path_word = chapter_json
                .get("groupPathWord")
                .and_then(|word| word.as_str())
                .context(format!(
                    "`{}`没有`groupPathWord`字段",
                    metadata_path.display()
                ))?
                .to_string();

            let Some(group) = self.comic.groups.get_mut(&group_path_word) else {
                continue;
            };

            if let Some(chapter_info) = group
                .iter_mut()
                .find(|chapter| chapter.chapter_uuid == chapter_uuid)
            {
                let parent = metadata_path
                    .parent()
                    .context(format!("`{}`没有父目录", metadata_path.display()))?;
                chapter_info.chapter_download_dir = Some(parent.to_path_buf());
                chapter_info.is_downloaded = Some(true);
            }
        }

        Ok(())
    }

    pub fn save_metadata(&self) -> anyhow::Result<()> {
        let mut comic = self.clone();
        // 将所有的is_downloaded字段设置为None，这样能使is_downloaded字段在序列化时被忽略
        comic.is_downloaded = None;
        for chapter_infos in comic.comic.groups.values_mut() {
            for chapter_info in chapter_infos.iter_mut() {
                chapter_info.is_downloaded = None;
            }
        }

        let comic_download_dir = self
            .comic_download_dir
            .as_ref()
            .context("`comic_download_dir`字段为`None`")?;
        let metadata_path = comic_download_dir.join("元数据.json");

        std::fs::create_dir_all(comic_download_dir)
            .context(format!("创建目录`{}`失败", comic_download_dir.display()))?;

        let comic_json = serde_json::to_string_pretty(&comic).context("将Comic序列化为json失败")?;

        std::fs::write(&metadata_path, comic_json)
            .context(format!("写入文件`{}`失败", metadata_path.display()))?;

        Ok(())
    }

    pub fn get_comic_export_dir(&self, app: &AppHandle) -> anyhow::Result<PathBuf> {
        let (download_dir, export_dir) = {
            let config = app.get_config();
            let config = config.read();
            (config.download_dir.clone(), config.export_dir.clone())
        };

        let Some(comic_download_dir) = self.comic_download_dir.clone() else {
            return Err(anyhow!("`comic_download_dir`字段为`None`"));
        };

        let relative_dir = comic_download_dir
            .strip_prefix(&download_dir)
            .context(format!(
                "无法从路径`{}`中移除前缀`{}`",
                comic_download_dir.display(),
                download_dir.display()
            ))?;

        let comic_export_dir = export_dir.join(relative_dir);
        Ok(comic_export_dir)
    }

    fn create_chapter_metadata_for_old_version(
        &self,
        comic_download_dir: &Path,
    ) -> anyhow::Result<()> {
        let mut chapter_dirs = HashSet::new();
        for group_entry in std::fs::read_dir(comic_download_dir)?.filter_map(Result::ok) {
            let Ok(file_type) = group_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }

            for chapter_entry in std::fs::read_dir(group_entry.path())?.filter_map(Result::ok) {
                let Ok(file_type) = chapter_entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                chapter_dirs.insert(chapter_entry.path());
            }
        }

        for chapter_info in self.comic.groups.values().flatten() {
            let group_title = utils::filename_filter(&chapter_info.group_name);
            let chapter_title = utils::filename_filter(&chapter_info.chapter_title);
            let order = chapter_info.order;
            let prefixed_chapter_title = format!("{order} {chapter_title}");

            let old_chapter_dir = comic_download_dir
                .join(&group_title)
                .join(&prefixed_chapter_title);

            let old_chapter_dir_exists = chapter_dirs.contains(&old_chapter_dir);
            let old_chapter_metadata_exists = old_chapter_dir.join("章节元数据.json").exists();

            if old_chapter_dir_exists && !old_chapter_metadata_exists {
                // 如果旧版本的章节目录存在，但没有元数据文件，就创建一个
                let mut info = chapter_info.clone();
                info.chapter_download_dir = Some(old_chapter_dir);
                info.is_downloaded = Some(true);
                info.save_metadata()?;
            }
        }

        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
#[allow(clippy::module_name_repetitions)]
pub struct ComicDetail {
    pub uuid: String,
    #[serde(rename = "b_404")]
    pub b_404: bool,
    #[serde(rename = "b_hidden")]
    pub b_hidden: bool,
    pub ban: i64,
    #[serde(rename = "ban_ip")]
    pub ban_ip: Option<bool>,
    pub name: String,
    pub alias: Option<String>,
    #[serde(rename = "path_word")]
    pub path_word: String,
    #[serde(rename = "close_comment")]
    pub close_comment: bool,
    #[serde(rename = "close_roast")]
    pub close_roast: bool,
    #[serde(rename = "free_type")]
    pub free_type: LabeledValue,
    pub restrict: LabeledValue,
    pub reclass: LabeledValue,
    #[serde(rename = "seo_baidu")]
    pub seo_baidu: Option<String>,
    pub region: LabeledValue,
    pub status: LabeledValue,
    pub author: Vec<Author>,
    pub theme: Vec<Theme>,
    pub brief: String,
    #[serde(rename = "datetime_updated")]
    pub datetime_updated: String,
    pub cover: String,
    #[serde(rename = "last_chapter")]
    pub last_chapter: LastChapter,
    pub popular: i64,
    /// `group_path_word` -> `chapter_infos`
    pub groups: HashMap<String, Vec<ChapterInfo>>,
}
impl ComicDetail {
    #[allow(clippy::cast_precision_loss)]
    fn from_resp_data(
        comic_resp_data: GetComicRespData,
        mut groups_chapters: HashMap<String, Vec<ChapterInGetChaptersRespData>>,
    ) -> ComicDetail {
        let comic_detail_resp_data = comic_resp_data.comic;

        let comic_status = if comic_detail_resp_data.status.value == 0 {
            ComicStatus::Ongoing
        } else {
            ComicStatus::Completed
        };

        let free_type = LabeledValue::from(comic_detail_resp_data.free_type);
        let restrict = LabeledValue::from(comic_detail_resp_data.restrict);
        let reclass = LabeledValue::from(comic_detail_resp_data.reclass);
        let region = LabeledValue::from(comic_detail_resp_data.region);
        let status = LabeledValue::from(comic_detail_resp_data.status);
        let author = Author::from(comic_detail_resp_data.author);
        let theme = Theme::from(comic_detail_resp_data.theme);
        let last_chapter = LastChapter::from(comic_detail_resp_data.last_chapter);

        let comic_uuid = &comic_detail_resp_data.uuid;
        let comic_title = &comic_detail_resp_data.name;
        let comic_path_word = &comic_detail_resp_data.path_word;

        let mut groups = HashMap::new();
        for (group_path_word, group_resp_data) in comic_resp_data.groups {
            let chapters = groups_chapters.remove(&group_path_word).unwrap_or_default();

            let chapter_infos: Vec<ChapterInfo> = chapters
                .into_iter()
                .map(|chapter| ChapterInfo {
                    chapter_uuid: chapter.uuid,
                    chapter_title: chapter.name,
                    chapter_size: chapter.size,
                    comic_uuid: comic_uuid.clone(),
                    comic_title: comic_title.clone(),
                    comic_path_word: comic_path_word.clone(),
                    group_path_word: group_path_word.clone(),
                    group_name: group_resp_data.name.clone(),
                    group_size: chapter.count,
                    order: chapter.ordered as f64 / 10.0,
                    comic_status,
                    is_downloaded: None,
                    chapter_download_dir: None,
                })
                .collect();

            groups.insert(group_path_word, chapter_infos);
        }

        ComicDetail {
            uuid: comic_detail_resp_data.uuid,
            b_404: comic_detail_resp_data.b_404,
            b_hidden: comic_detail_resp_data.b_hidden,
            ban: comic_detail_resp_data.ban,
            ban_ip: comic_detail_resp_data.ban_ip,
            name: comic_title.clone(),
            alias: comic_detail_resp_data.alias,
            path_word: comic_detail_resp_data.path_word,
            close_comment: comic_detail_resp_data.close_comment,
            close_roast: comic_detail_resp_data.close_roast,
            free_type,
            restrict,
            reclass,
            seo_baidu: comic_detail_resp_data.seo_baidu,
            region,
            status,
            author,
            theme,
            brief: comic_detail_resp_data.brief,
            datetime_updated: comic_detail_resp_data.datetime_updated,
            cover: comic_detail_resp_data.cover,
            last_chapter,
            popular: comic_resp_data.popular,
            groups,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub name: String,
    pub alias: Option<String>,
    #[serde(rename = "path_word")]
    pub path_word: String,
}
impl Author {
    fn from(author: Vec<AuthorRespData>) -> Vec<Author> {
        author
            .into_iter()
            .map(|author| Author {
                name: author.name,
                alias: author.alias,
                path_word: author.path_word,
            })
            .collect()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabeledValue {
    pub value: i64,
    pub display: String,
}
impl LabeledValue {
    fn from(labeled_value: LabeledValueRespData) -> LabeledValue {
        LabeledValue {
            value: labeled_value.value,
            display: labeled_value.display,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub name: String,
    #[serde(rename = "path_word")]
    pub path_word: String,
}
impl Theme {
    fn from(theme: Vec<ThemeRespData>) -> Vec<Theme> {
        theme
            .into_iter()
            .map(|theme| Theme {
                name: theme.name,
                path_word: theme.path_word,
            })
            .collect()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LastChapter {
    pub uuid: String,
    pub name: String,
}
impl LastChapter {
    fn from(last_chapter: LastChapterRespData) -> LastChapter {
        LastChapter {
            uuid: last_chapter.uuid,
            name: last_chapter.name,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    #[serde(rename = "path_word")]
    path_word: String,
    count: u32,
    name: String,
}
impl Group {
    fn from(group: HashMap<String, GroupRespData>) -> HashMap<String, Group> {
        group
            .into_iter()
            .map(|(key, val)| {
                let group = Group {
                    path_word: val.path_word,
                    count: val.count,
                    name: val.name,
                };
                (key, group)
            })
            .collect()
    }
}
