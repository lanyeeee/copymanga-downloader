use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct GetChapterRespData {
    #[serde(rename = "is_banned")]
    pub is_banned: bool,
    #[serde(rename = "show_app")]
    pub show_app: bool,
    #[serde(rename = "is_lock")]
    pub is_lock: bool,
    #[serde(rename = "is_login")]
    pub is_login: bool,
    #[serde(rename = "is_mobile_bind")]
    pub is_mobile_bind: bool,
    #[serde(rename = "is_vip")]
    pub is_vip: bool,
    pub comic: ComicInGetChapterRespData,
    pub chapter: ChapterInGetChapterRespData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInGetChapterRespData {
    pub name: String,
    pub uuid: String,
    #[serde(rename = "path_word")]
    pub path_word: String,
    pub restrict: RestrictRespData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RestrictRespData {
    pub value: i64,
    pub display: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInGetChapterRespData {
    pub index: i64,
    pub uuid: String,
    pub count: i64,
    pub ordered: i64,
    pub size: i64,
    pub name: String,
    #[serde(rename = "comic_id")]
    pub comic_id: String,
    #[serde(rename = "comic_path_word")]
    pub comic_path_word: String,
    #[serde(rename = "group_id")]
    pub group_id: Option<String>,
    #[serde(rename = "group_path_word")]
    pub group_path_word: String,
    #[serde(rename = "type")]
    pub type_field: i64,
    #[serde(rename = "img_type")]
    pub img_type: i64,
    pub news: String,
    #[serde(rename = "datetime_created")]
    pub datetime_created: String,
    pub prev: Option<String>,
    pub next: Option<String>,
    pub contents: Vec<ContentRespData>,
    pub words: Vec<i64>,
    #[serde(rename = "is_long")]
    pub is_long: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ContentRespData {
    pub url: String,
}
