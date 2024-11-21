use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct LoginRespData {
    pub token: String,
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub avatar: String,
    pub datetime_created: String,
    pub ticket: f64,
    pub reward_ticket: f64,
    pub downloads: i64,
    pub vip_downloads: i64,
    pub reward_downloads: i64,
    pub scy_answer: bool,
}
