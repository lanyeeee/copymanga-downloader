use std::collections::HashMap;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::{
    config::Config,
    responses::{
        AuthorRespData, ChapterInGetChaptersRespData, GetComicRespData, GroupRespData,
        LabeledValueRespData, LastChapterRespData, ThemeRespData,
    },
    utils::filename_filter,
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
}
impl Comic {
    pub fn from(
        app: &AppHandle,
        comic_resp_data: GetComicRespData,
        groups_chapters: HashMap<String, Vec<ChapterInGetChaptersRespData>>,
    ) -> Comic {
        let is_banned = comic_resp_data.is_banned;
        let is_lock = comic_resp_data.is_lock;
        let is_login = comic_resp_data.is_login;
        let is_mobile_bind = comic_resp_data.is_mobile_bind;
        let is_vip = comic_resp_data.is_vip;
        let popular = comic_resp_data.popular;
        let groups = Group::from(comic_resp_data.groups.clone());
        let comic = ComicDetail::from(app, comic_resp_data, groups_chapters);

        Comic {
            is_banned,
            is_lock,
            is_login,
            is_mobile_bind,
            is_vip,
            comic,
            popular,
            groups,
        }
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
    #[serde(rename = "img_type")]
    pub img_type: i64,
    #[serde(rename = "seo_baidu")]
    pub seo_baidu: String,
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
    pub groups: HashMap<String, Vec<ChapterInfo>>,
}
impl ComicDetail {
    fn from(
        app: &AppHandle,
        comic_resp_data: GetComicRespData,
        mut groups_chapters: HashMap<String, Vec<ChapterInGetChaptersRespData>>,
    ) -> ComicDetail {
        let comic_detail_resp_data = comic_resp_data.comic;

        let free_type = LabeledValue::from(comic_detail_resp_data.free_type);
        let restrict = LabeledValue::from(comic_detail_resp_data.restrict);
        let reclass = LabeledValue::from(comic_detail_resp_data.reclass);
        let region = LabeledValue::from(comic_detail_resp_data.region);
        let status = LabeledValue::from(comic_detail_resp_data.status);
        let author = Author::from(comic_detail_resp_data.author);
        let theme = Theme::from(comic_detail_resp_data.theme);
        let last_chapter = LastChapter::from(comic_detail_resp_data.last_chapter);

        let comic_uuid = comic_detail_resp_data.uuid.clone();
        let comic_title = filename_filter(&comic_detail_resp_data.name);
        let comic_path_word = comic_detail_resp_data.path_word.clone();
        let mut groups = HashMap::new();
        for (group_path_word, group_resp_data) in comic_resp_data.groups {
            let group_name = filename_filter(&group_resp_data.name);

            let mut chapters = groups_chapters.remove(&group_path_word).unwrap_or_default();
            // 解决章节标题重复的问题
            let mut chapter_title_count = HashMap::new();
            // 统计章节标题出现的次数
            for chapter in &mut chapters {
                chapter.name = filename_filter(&chapter.name);
                let Some(count) = chapter_title_count.get_mut(&chapter.name) else {
                    chapter_title_count.insert(chapter.name.clone(), 1);
                    continue;
                };
                *count += 1;
            }
            // 只保留重复的章节标题
            chapter_title_count.retain(|_, v| *v > 1);
            // 为重复的章节标题添加序号
            for chapter in &mut chapters {
                let Some(count) = chapter_title_count.get_mut(&chapter.name) else {
                    continue;
                };
                chapter.name = format!("{}-{}", chapter.name, count);
                *count -= 1;
            }

            let chapter_infos: Vec<_> = chapters
                .into_iter()
                .map(|chapter| {
                    ChapterInfo::from(
                        app,
                        chapter,
                        comic_title.clone(),
                        group_name.clone(),
                        comic_uuid.clone(),
                        comic_path_word.clone(),
                    )
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
            name: comic_title,
            alias: comic_detail_resp_data.alias,
            path_word: comic_detail_resp_data.path_word,
            close_comment: comic_detail_resp_data.close_comment,
            close_roast: comic_detail_resp_data.close_roast,
            free_type,
            restrict,
            reclass,
            img_type: comic_detail_resp_data.img_type,
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
pub struct ChapterInfo {
    pub chapter_uuid: String,
    pub chapter_title: String,
    /// 以order为前缀的章节标题
    pub prefixed_chapter_title: String,
    pub comic_uuid: String,
    pub comic_title: String,
    pub comic_path_word: String,
    pub group_name: String,
    /// 此章节在group中的顺序
    pub order: f64,
    pub is_downloaded: bool,
}
impl ChapterInfo {
    #[allow(clippy::cast_precision_loss)]
    pub fn from(
        app: &AppHandle,
        chapter: ChapterInGetChaptersRespData,
        comic_title: String,
        group_name: String,
        comic_uuid: String,
        comic_path_word: String,
    ) -> ChapterInfo {
        let order = chapter.ordered as f64 / 10.0;
        let chapter_title = filename_filter(&chapter.name);
        let prefixed_chapter_title = format!("{order} {chapter_title}");
        let is_downloaded = app
            .state::<RwLock<Config>>()
            .read()
            .download_dir
            .join(&comic_title)
            .join(&group_name)
            .join(&prefixed_chapter_title)
            .exists();
        ChapterInfo {
            chapter_uuid: chapter.uuid,
            chapter_title,
            prefixed_chapter_title,
            comic_uuid,
            comic_title,
            comic_path_word,
            group_name,
            order,
            is_downloaded,
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
