use crate::domain::cover_art::AlbumCover;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use rusqlite::{Connection, OptionalExtension};
use serde_rusqlite::from_row;

pub struct CoverArtRepository;

impl CoverArtRepository {
    pub fn new() -> Self {
        Self
    }

    /// With several links the oldest one wins, until there's a notion of
    /// source priority.
    pub fn find_by_album(
        &self,
        connection: &Connection,
        album_id: i64,
    ) -> Result<Option<AlbumCover>> {
        connection
            .query_row(
                "SELECT album_links.source_id, album_link_covers.external_id \
                 FROM album_link_covers \
                 JOIN album_links ON album_links.id = album_link_covers.album_link_id \
                 WHERE album_links.album_id = ?1 \
                 ORDER BY album_links.id \
                 LIMIT 1",
                (album_id,),
                |row| Ok(from_row::<AlbumCover>(row)),
            )
            .optional()
            .context("failed to query album cover")?
            .transpose()
            .context("failed to deserialize album cover row")
    }

    pub fn upsert(
        &self,
        connection: &Connection,
        album_link_id: i64,
        external_id: &str,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        connection
            .execute(
                "INSERT INTO album_link_covers (album_link_id, external_id, synced_at) \
                 VALUES (?1, ?2, ?3) \
                 ON CONFLICT (album_link_id) DO UPDATE \
                 SET external_id = excluded.external_id, synced_at = excluded.synced_at",
                (album_link_id, external_id, synced_at),
            )
            .context("failed to upsert album_link_covers row")?;

        Ok(())
    }

    pub fn delete_by_link(&self, connection: &Connection, album_link_id: i64) -> Result<()> {
        connection
            .execute(
                "DELETE FROM album_link_covers WHERE album_link_id = ?1",
                (album_link_id,),
            )
            .context("failed to delete album_link_covers row")?;

        Ok(())
    }
}
