use crate::domain::artist::Artist;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use rusqlite::{Connection, OptionalExtension};
use serde_rusqlite::{from_row, from_rows};

pub struct ArtistRepository;

impl ArtistRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list_all(&self, connection: &Connection) -> Result<Vec<Artist>> {
        let mut stmt = connection
            .prepare("SELECT * FROM artists ORDER BY id")
            .context("failed to prepare artists query")?;

        from_rows::<Artist>(stmt.query(()).context("failed to query artists")?)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to deserialize artists")
    }

    pub fn find_by_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
    ) -> Result<Option<Artist>> {
        connection
            .query_row(
                "SELECT artists.* FROM artists \
                 JOIN artist_links ON artist_links.artist_id = artists.id \
                 WHERE artist_links.source_id = ?1 AND artist_links.external_id = ?2",
                (source_id, external_id),
                |row| Ok(from_row::<Artist>(row)),
            )
            .optional()
            .context("failed to query artists by link")?
            .transpose()
            .context("failed to deserialize artist row")
    }

    pub fn insert(&self, connection: &Connection, name: &str) -> Result<Artist> {
        connection
            .query_row(
                "INSERT INTO artists (name) VALUES (?1) RETURNING *",
                (name,),
                |row| Ok(from_row::<Artist>(row)),
            )
            .context("failed to insert artists row")?
            .context("failed to deserialize inserted artist row")
    }

    pub fn update(&self, connection: &Connection, id: i64, name: &str) -> Result<Artist> {
        connection
            .query_row(
                "UPDATE artists SET name = ?1 WHERE id = ?2 RETURNING *",
                (name, id),
                |row| Ok(from_row::<Artist>(row)),
            )
            .context("failed to update artists row")?
            .context("failed to deserialize updated artist row")
    }

    pub fn create_link(
        &self,
        connection: &Connection,
        artist_id: i64,
        source_id: i64,
        external_id: &str,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        connection
            .execute(
                "INSERT INTO artist_links (artist_id, source_id, external_id, synced_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                (artist_id, source_id, external_id, synced_at),
            )
            .context("failed to insert artist_links row")?;

        Ok(())
    }

    pub fn touch_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        connection
            .execute(
                "UPDATE artist_links SET synced_at = ?1 WHERE source_id = ?2 AND external_id = ?3",
                (synced_at, source_id, external_id),
            )
            .context("failed to update artist_links row")?;

        Ok(())
    }

    pub fn upsert(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
        name: &str,
        synced_at: DateTime<Utc>,
    ) -> Result<Artist> {
        if let Some(artist) = self.find_by_link(connection, source_id, external_id)? {
            self.touch_link(connection, source_id, external_id, synced_at)?;
            self.update(connection, artist.id, name)
        } else {
            let artist = self.insert(connection, name)?;
            self.create_link(connection, artist.id, source_id, external_id, synced_at)?;
            Ok(artist)
        }
    }

    pub fn delete_orphaned(&self, connection: &Connection, source_id: i64) -> Result<()> {
        connection
            .execute(
                "DELETE FROM artist_links \
                 WHERE source_id = ?1 AND artist_id NOT IN (SELECT DISTINCT artist_id FROM albums)",
                (source_id,),
            )
            .context("failed to delete orphaned artist_links rows")?;

        connection
            .execute(
                "DELETE FROM artists \
                 WHERE id NOT IN (SELECT artist_id FROM artist_links) \
                   AND id NOT IN (SELECT DISTINCT artist_id FROM albums)",
                (),
            )
            .context("failed to delete orphaned artists rows")?;

        Ok(())
    }
}
