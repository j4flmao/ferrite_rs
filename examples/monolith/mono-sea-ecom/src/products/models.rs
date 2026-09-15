use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub price_cents: i64,
    pub stock: i64,
    pub category_id: Option<i64>,
    pub images: Vec<String>,
    #[serde(with = "ts_seconds")]
    pub created_at: DateTime<Utc>,
}
