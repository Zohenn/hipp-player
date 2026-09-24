use chrono::{DateTime, Utc};

#[derive(Clone, serde::Deserialize)]
pub struct Album {
    pub id: i64,
    pub artist_id: i64,
    pub name: String,
    pub cover_art: Option<String>,
    pub created_at: DateTime<Utc>,
}
