use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::{
    responses::{AuthorRespData, ComicInSearchRespData, Pagination, SearchRespData},
    types::Comic,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct SearchResult(Pagination<ComicInSearch>);

impl Deref for SearchResult {
    type Target = Pagination<ComicInSearch>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SearchResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SearchResult {
    pub fn from_resp_data(app: &AppHandle, resp_data: SearchRespData) -> Self {
        let total = resp_data.total;
        let limit = resp_data.limit;
        let offset = resp_data.offset;

        let mut list = Vec::with_capacity(resp_data.list.len());
        for comic in resp_data.0.list {
            let comic = ComicInSearch::from_resp_data(app, &comic);
            list.push(comic);
        }

        SearchResult(Pagination {
            list,
            total,
            limit,
            offset,
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInSearch {
    pub name: String,
    pub alias: Option<String>,
    pub path_word: String,
    pub cover: String,
    pub ban: i64,
    pub author: Vec<AuthorRespData>,
    pub popular: i64,
    pub is_downloaded: bool,
}

impl ComicInSearch {
    pub fn from_resp_data(app: &AppHandle, resp_data: &ComicInSearchRespData) -> Self {
        let mut comic = ComicInSearch {
            name: resp_data.name.clone(),
            alias: resp_data.alias.clone(),
            path_word: resp_data.path_word.clone(),
            cover: resp_data.cover.clone(),
            ban: resp_data.ban,
            author: resp_data.author.clone(),
            popular: resp_data.popular,
            is_downloaded: false,
        };

        comic.update_fields(app);

        comic
    }

    pub fn update_fields(&mut self, app: &AppHandle) {
        self.is_downloaded = Comic::get_is_downloaded(app, &self.name);
    }
}
