use serde::{Deserialize, Serialize};
use specta::Type;

use super::AuthorRespData;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchRespData {
    pub list: Vec<ComicInSearchRespData>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInSearchRespData {
    pub name: String,
    pub alias: String,
    #[serde(rename = "path_word")]
    pub path_word: String,
    pub cover: String,
    pub ban: i64,
    #[serde(rename = "img_type")]
    pub img_type: i64,
    pub author: Vec<AuthorRespData>,
    pub popular: i64,
}
