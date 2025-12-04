use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::{
    responses::{AuthorRespData, ComicInSearchRespData, Pagination, SearchRespData},
    utils,
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
    pub fn from_resp_data(
        app: &AppHandle,
        resp_data: SearchRespData,
    ) -> anyhow::Result<SearchResult> {
        let total = resp_data.total;
        let limit = resp_data.limit;
        let offset = resp_data.offset;

        let path_word_to_dir_map =
            utils::create_path_word_to_dir_map(app).context("创建漫画路径词到下载目录映射失败")?;
        let mut list = Vec::with_capacity(resp_data.list.len());

        for comic in resp_data.0.list {
            let comic = ComicInSearch::from_resp_data(&comic, &path_word_to_dir_map);
            list.push(comic);
        }

        let search_result = SearchResult(Pagination {
            list,
            total,
            limit,
            offset,
        });

        Ok(search_result)
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
    pub comic_download_dir: PathBuf,
}

impl ComicInSearch {
    pub fn from_resp_data(
        resp_data: &ComicInSearchRespData,
        path_word_to_dir_map: &HashMap<String, PathBuf>,
    ) -> Self {
        let mut comic = ComicInSearch {
            name: resp_data.name.clone(),
            alias: resp_data.alias.clone(),
            path_word: resp_data.path_word.clone(),
            cover: resp_data.cover.clone(),
            ban: resp_data.ban,
            author: resp_data.author.clone(),
            popular: resp_data.popular,
            is_downloaded: false,
            comic_download_dir: PathBuf::new(),
        };

        comic.update_fields(path_word_to_dir_map);

        comic
    }

    pub fn update_fields(&mut self, path_word_to_dir_map: &HashMap<String, PathBuf>) {
        if let Some(comic_download_dir) = path_word_to_dir_map.get(&self.path_word) {
            self.comic_download_dir = comic_download_dir.clone();
            self.is_downloaded = true;
        }
    }
}
