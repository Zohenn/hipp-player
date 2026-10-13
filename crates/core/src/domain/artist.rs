use chrono::{DateTime, Utc};

#[derive(Clone, serde::Deserialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
}
