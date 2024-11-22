use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::Pagination;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct GetChaptersRespData(Pagination<ChapterInGetChaptersRespData>);

impl Deref for GetChaptersRespData {
    type Target = Pagination<ChapterInGetChaptersRespData>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GetChaptersRespData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInGetChaptersRespData {
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
}
