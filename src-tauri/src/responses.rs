mod copy_resp;
mod get_chapter_resp_data;
mod get_chapters_resp_data;
mod get_comic_resp_data;
mod login_resp_data;
mod search_resp_data;
mod user_profile_resp_data;
mod get_favorite_resp_data;

pub use copy_resp::*;
pub use get_chapter_resp_data::*;
pub use get_chapters_resp_data::*;
pub use get_comic_resp_data::*;
pub use login_resp_data::*;
pub use search_resp_data::*;
pub use user_profile_resp_data::*;
pub use get_favorite_resp_data::*;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct Pagination<T> {
    pub list: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct AuthorRespData {
    pub name: String,
    pub alias: Option<String>,
    #[serde(rename = "path_word")]
    pub path_word: String,
}
