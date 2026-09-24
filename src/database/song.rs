use crate::domain::song::Song;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use rusqlite::Connection;

pub struct SongRepository;

impl SongRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn find_by_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
    ) -> Result<Option<Song>> {
        todo!()
    }

    pub fn insert(
        &self,
        connection: &Connection,
        album_id: i64,
        title: &str,
        disc_number: Option<i64>,
        track_number: Option<i64>,
        duration_seconds: i64,
    ) -> Result<Song> {
        todo!()
    }

    pub fn create_link(
        &self,
        connection: &Connection,
        song_id: i64,
        source_id: i64,
        external_id: &str,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        todo!()
    }

    pub fn touch_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        todo!()
    }
}
