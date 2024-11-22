use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileRespData {
    #[serde(rename = "user_id")]
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub avatar: String,
    #[serde(rename = "datetime_created")]
    pub datetime_created: String,
    pub ticket: f64,
    #[serde(rename = "reward_ticket")]
    pub reward_ticket: f64,
    pub downloads: i64,
    #[serde(rename = "vip_downloads")]
    pub vip_downloads: i64,
    #[serde(rename = "reward_downloads")]
    pub reward_downloads: i64,
    #[serde(rename = "scy_answer")]
    pub scy_answer: bool,
    #[serde(rename = "day_downloads_refresh")]
    pub day_downloads_refresh: String,
    #[serde(rename = "day_downloads")]
    pub day_downloads: i64,
}
