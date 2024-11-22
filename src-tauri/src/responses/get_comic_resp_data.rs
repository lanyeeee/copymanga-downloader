use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

use super::AuthorRespData;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct GetComicRespData {
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
    pub comic: ComicInGetComicRespData,
    pub popular: i64,
    pub groups: HashMap<String, GroupRespData>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct ComicInGetComicRespData {
    pub uuid: String,
    #[serde(rename = "b_404")]
    pub b_404: bool,
    #[serde(rename = "b_hidden")]
    pub b_hidden: bool,
    pub ban: i64,
    #[serde(rename = "ban_ip")]
    pub ban_ip: bool,
    pub name: String,
    pub alias: Option<String>,
    #[serde(rename = "path_word")]
    pub path_word: String,
    #[serde(rename = "close_comment")]
    pub close_comment: bool,
    #[serde(rename = "close_roast")]
    pub close_roast: bool,
    #[serde(rename = "free_type")]
    pub free_type: LabeledValueRespData,
    pub restrict: LabeledValueRespData,
    pub reclass: LabeledValueRespData,
    #[serde(rename = "img_type")]
    pub img_type: i64,
    #[serde(rename = "seo_baidu")]
    pub seo_baidu: String,
    pub region: LabeledValueRespData,
    pub status: LabeledValueRespData,
    pub author: Vec<AuthorRespData>,
    pub theme: Vec<ThemeRespData>,
    pub brief: String,
    #[serde(rename = "datetime_updated")]
    pub datetime_updated: String,
    pub cover: String,
    #[serde(rename = "last_chapter")]
    pub last_chapter: LastChapterRespData,
    pub popular: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct LabeledValueRespData {
    pub value: i64,
    pub display: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeRespData {
    pub name: String,
    #[serde(rename = "path_word")]
    pub path_word: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct LastChapterRespData {
    pub uuid: String,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct GroupRespData {
    #[serde(rename = "path_word")]
    path_word: String,
    count: u32,
    name: String,
}
