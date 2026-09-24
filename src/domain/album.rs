use crate::domain::song::Song;
use chrono::{DateTime, Utc};

pub struct Album {
    pub id: i64,
    pub artist_id: i64,
    pub name: String,
    pub cover_art: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct AlbumWithSongs {
    pub album: Album,
    pub songs: Vec<Song>,
}
