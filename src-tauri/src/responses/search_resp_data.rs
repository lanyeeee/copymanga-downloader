use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{AuthorRespData, Pagination};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct SearchRespData(Pagination<ComicInSearchRespData>);

impl Deref for SearchRespData {
    type Target = Pagination<ComicInSearchRespData>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SearchRespData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInSearchRespData {
    pub name: String,
    pub alias: Option<String>,
    #[serde(rename = "path_word")]
    pub path_word: String,
    pub cover: String,
    pub ban: i64,
    pub author: Vec<AuthorRespData>,
    pub popular: i64,
}
