use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{AuthorRespData, Pagination};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct GetFavoriteRespData(Pagination<FavoriteItemRespData>);

impl Deref for GetFavoriteRespData {
    type Target = Pagination<FavoriteItemRespData>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GetFavoriteRespData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteItemRespData {
    pub uuid: i64,
    #[serde(rename = "b_folder")]
    pub b_folder: bool,
    pub comic: ComicInGetFavoriteRespData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInGetFavoriteRespData {
    pub uuid: String,
    #[serde(rename = "b_display")]
    pub b_display: bool,
    pub name: String,
    #[serde(rename = "path_word")]
    pub path_word: String,
    pub author: Vec<AuthorRespData>,
    pub cover: String,
    pub status: i64,
    pub popular: i64,
    #[serde(rename = "datetime_updated")]
    pub datetime_updated: String,
    #[serde(rename = "last_chapter_id")]
    pub last_chapter_id: String,
    #[serde(rename = "last_chapter_name")]
    pub last_chapter_name: String,
}
