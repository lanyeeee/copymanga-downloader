use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum GetFavoriteOrdering {
    /// 按加到书架时间降序排序
    Added,
    /// 按作品更新时间降序排序
    Updated,
    /// 按上次阅读时间排序
    Read,
}

impl GetFavoriteOrdering {
    pub fn as_params(&self) -> &'static str {
        match self {
            GetFavoriteOrdering::Added => "-datetime_modifier",
            GetFavoriteOrdering::Updated => "-datetime_updated",
            GetFavoriteOrdering::Read => "-datetime_browse",
        }
    }
}
