use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    #[serde(with = "ts_seconds")]
    pub created_at: DateTime<Utc>,
}
