use crate::domain::artist::Artist;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use rusqlite::Connection;

pub struct ArtistRepository;

impl ArtistRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn find_by_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
    ) -> Result<Option<Artist>> {
        todo!()
    }

    pub fn insert(&self, connection: &Connection, name: &str) -> Result<Artist> {
        todo!()
    }

    pub fn create_link(
        &self,
        connection: &Connection,
        artist_id: i64,
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
