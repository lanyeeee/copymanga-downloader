use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResp {
    pub code: i64,
    pub message: String,
    pub results: serde_json::Value,
}
