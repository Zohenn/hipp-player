use crate::domain::album::Album;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use rusqlite::{Connection, OptionalExtension};
use serde_rusqlite::{from_row, from_rows};

pub struct AlbumRepository;

impl AlbumRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list_all(&self, connection: &Connection) -> Result<Vec<Album>> {
        let mut stmt = connection
            .prepare("SELECT * FROM albums ORDER BY id")
            .context("failed to prepare albums query")?;

        from_rows::<Album>(stmt.query(()).context("failed to query albums")?)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to deserialize albums")
    }

    pub fn find_by_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
    ) -> Result<Option<Album>> {
        connection
            .query_row(
                "SELECT albums.* FROM albums \
                 JOIN album_links ON album_links.album_id = albums.id \
                 WHERE album_links.source_id = ?1 AND album_links.external_id = ?2",
                (source_id, external_id),
                |row| Ok(from_row::<Album>(row)),
            )
            .optional()
            .context("failed to query albums by link")?
            .transpose()
            .context("failed to deserialize album row")
    }

    pub fn insert(&self, connection: &Connection, artist_id: i64, name: &str) -> Result<Album> {
        connection
            .query_row(
                "INSERT INTO albums (artist_id, name) VALUES (?1, ?2) RETURNING *",
                (artist_id, name),
                |row| Ok(from_row::<Album>(row)),
            )
            .context("failed to insert albums row")?
            .context("failed to deserialize inserted album row")
    }

    pub fn update(
        &self,
        connection: &Connection,
        id: i64,
        artist_id: i64,
        name: &str,
    ) -> Result<Album> {
        connection
            .query_row(
                "UPDATE albums SET artist_id = ?1, name = ?2 WHERE id = ?3 RETURNING *",
                (artist_id, name, id),
                |row| Ok(from_row::<Album>(row)),
            )
            .context("failed to update albums row")?
            .context("failed to deserialize updated album row")
    }

    pub fn create_link(
        &self,
        connection: &Connection,
        album_id: i64,
        source_id: i64,
        external_id: &str,
        music_folder_id: Option<&str>,
        synced_at: DateTime<Utc>,
    ) -> Result<i64> {
        connection
            .query_row(
                "INSERT INTO album_links (album_id, source_id, external_id, music_folder_id, synced_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
                (album_id, source_id, external_id, music_folder_id, synced_at),
                |row| row.get(0),
            )
            .context("failed to insert album_links row")
    }

    pub fn touch_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
        music_folder_id: Option<&str>,
        synced_at: DateTime<Utc>,
    ) -> Result<i64> {
        connection
            .query_row(
                "UPDATE album_links SET music_folder_id = ?1, synced_at = ?2 \
                 WHERE source_id = ?3 AND external_id = ?4 RETURNING id",
                (music_folder_id, synced_at, source_id, external_id),
                |row| row.get(0),
            )
            .context("failed to update album_links row")
    }

    /// Returns the album along with the id of its link to `source_id`.
    pub fn upsert(
        &self,
        connection: &Connection,
        artist_id: i64,
        source_id: i64,
        external_id: &str,
        name: &str,
        music_folder_id: Option<&str>,
        synced_at: DateTime<Utc>,
    ) -> Result<(Album, i64)> {
        if let Some(album) = self.find_by_link(connection, source_id, external_id)? {
            let link_id = self.touch_link(
                connection,
                source_id,
                external_id,
                music_folder_id,
                synced_at,
            )?;
            let album = self.update(connection, album.id, artist_id, name)?;
            Ok((album, link_id))
        } else {
            let album = self.insert(connection, artist_id, name)?;
            let link_id = self.create_link(
                connection,
                album.id,
                source_id,
                external_id,
                music_folder_id,
                synced_at,
            )?;
            Ok((album, link_id))
        }
    }
}
