mod comic_resp_data;
mod copy_resp;
mod login_resp_data;
mod search_resp_data;
mod user_profile_resp_data;

pub use comic_resp_data::*;
pub use copy_resp::*;
pub use login_resp_data::*;
pub use search_resp_data::*;

use serde::{Deserialize, Serialize};
use specta::Type;
pub use user_profile_resp_data::*;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct AuthorRespData {
    pub name: String,
    pub alias: String,
    #[serde(rename = "path_word")]
    pub path_word: String,
}
