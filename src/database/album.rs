use crate::domain::album::Album;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use rusqlite::Connection;

pub struct AlbumRepository;

impl AlbumRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn find_by_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
    ) -> Result<Option<Album>> {
        todo!()
    }

    pub fn insert(
        &self,
        connection: &Connection,
        artist_id: i64,
        name: &str,
        cover_art: Option<&str>,
    ) -> Result<Album> {
        todo!()
    }

    pub fn create_link(
        &self,
        connection: &Connection,
        album_id: i64,
        source_id: i64,
        external_id: &str,
        music_folder_id: Option<&str>,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        todo!()
    }

    pub fn touch_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
        music_folder_id: Option<&str>,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        todo!()
    }
}
