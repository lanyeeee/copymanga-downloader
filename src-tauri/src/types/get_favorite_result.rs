use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::{
    responses::{AuthorRespData, ComicInGetFavoriteRespData, GetFavoriteRespData, Pagination},
    types::Comic,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct GetFavoriteResult(Pagination<FavoriteItem>);

impl Deref for GetFavoriteResult {
    type Target = Pagination<FavoriteItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GetFavoriteResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl GetFavoriteResult {
    pub fn from_resp_data(app: &AppHandle, resp_data: GetFavoriteRespData) -> Self {
        let total = resp_data.total;
        let limit = resp_data.limit;
        let offset = resp_data.offset;

        let mut list = Vec::with_capacity(resp_data.list.len());
        for item in resp_data.0.list {
            let comic = ComicInFavorite::from_resp_data(app, &item.comic);
            list.push(FavoriteItem {
                uuid: item.uuid,
                b_folder: item.b_folder,
                comic,
            });
        }

        GetFavoriteResult(Pagination {
            list,
            total,
            limit,
            offset,
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteItem {
    pub uuid: i64,
    pub b_folder: bool,
    pub comic: ComicInFavorite,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInFavorite {
    pub uuid: String,
    pub b_display: bool,
    pub name: String,
    pub path_word: String,
    pub author: Vec<AuthorRespData>,
    pub cover: String,
    pub status: i64,
    pub popular: i64,
    pub datetime_updated: String,
    pub last_chapter_id: String,
    pub last_chapter_name: String,
    pub is_downloaded: bool,
}

impl ComicInFavorite {
    pub fn from_resp_data(app: &AppHandle, resp_data: &ComicInGetFavoriteRespData) -> Self {
        let mut comic = ComicInFavorite {
            uuid: resp_data.uuid.clone(),
            b_display: resp_data.b_display,
            name: resp_data.name.clone(),
            path_word: resp_data.path_word.clone(),
            author: resp_data.author.clone(),
            cover: resp_data.cover.clone(),
            status: resp_data.status,
            popular: resp_data.popular,
            datetime_updated: resp_data.datetime_updated.clone(),
            last_chapter_id: resp_data.last_chapter_id.clone(),
            last_chapter_name: resp_data.last_chapter_name.clone(),
            is_downloaded: false,
        };

        comic.update_fields(app);

        comic
    }

    pub fn update_fields(&mut self, app: &AppHandle) {
        self.is_downloaded = Comic::get_is_downloaded(app, &self.name);
    }
}
