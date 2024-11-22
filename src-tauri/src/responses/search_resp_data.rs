use serde::{Deserialize, Serialize};
use specta::Type;

use super::{AuthorRespData, Pagination};

pub type SearchRespData = Pagination<ComicInSearchRespData>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComicInSearchRespData {
    pub name: String,
    pub alias: Option<String>,
    #[serde(rename = "path_word")]
    pub path_word: String,
    pub cover: String,
    pub ban: i64,
    #[serde(rename = "img_type")]
    pub img_type: i64,
    pub author: Vec<AuthorRespData>,
    pub popular: i64,
}
