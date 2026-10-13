use chrono::{DateTime, Utc};

#[derive(Clone, serde::Deserialize)]
pub struct Song {
    pub id: i64,
    pub album_id: i64,
    pub title: String,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub duration_seconds: i64,
    pub created_at: DateTime<Utc>,
}
