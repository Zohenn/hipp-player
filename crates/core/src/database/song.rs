use crate::domain::song::Song;
use crate::domain::source::SourceKind;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use serde_rusqlite::{from_row, from_rows};

pub struct SongRepository;

impl SongRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list_by_album(&self, connection: &Connection, album_id: i64) -> Result<Vec<Song>> {
        let mut stmt = connection
            .prepare(
                "SELECT * FROM songs WHERE album_id = ?1 \
                 ORDER BY disc_number, track_number, id",
            )
            .context("failed to prepare songs query")?;

        from_rows::<Song>(stmt.query((album_id,)).context("failed to query songs")?)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to deserialize songs")
    }

    pub fn find_by_link(
        &self,
        connection: &Connection,
        source_id: i64,
        external_id: &str,
    ) -> Result<Option<Song>> {
        connection
            .query_row(
                "SELECT songs.* FROM songs \
                 JOIN song_links ON song_links.song_id = songs.id \
                 WHERE song_links.source_id = ?1 AND song_links.external_id = ?2",
                (source_id, external_id),
                |row| Ok(from_row::<Song>(row)),
            )
            .optional()
            .context("failed to query songs by link")?
            .transpose()
            .context("failed to deserialize song row")
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
        connection
            .query_row(
                "INSERT INTO songs (album_id, title, disc_number, track_number, duration_seconds) \
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING *",
                (album_id, title, disc_number, track_number, duration_seconds),
                |row| Ok(from_row::<Song>(row)),
            )
            .context("failed to insert songs row")?
            .context("failed to deserialize inserted song row")
    }

    pub fn update(
        &self,
        connection: &Connection,
        id: i64,
        album_id: i64,
        title: &str,
        disc_number: Option<i64>,
        track_number: Option<i64>,
        duration_seconds: i64,
    ) -> Result<Song> {
        connection
            .query_row(
                "UPDATE songs SET album_id = ?1, title = ?2, disc_number = ?3, track_number = ?4, \
                 duration_seconds = ?5 WHERE id = ?6 RETURNING *",
                (
                    album_id,
                    title,
                    disc_number,
                    track_number,
                    duration_seconds,
                    id,
                ),
                |row| Ok(from_row::<Song>(row)),
            )
            .context("failed to update songs row")?
            .context("failed to deserialize updated song row")
    }

    pub fn create_link(
        &self,
        connection: &Connection,
        song_id: i64,
        source_id: i64,
        external_id: &str,
        synced_at: DateTime<Utc>,
    ) -> Result<()> {
        connection
            .execute(
                "INSERT INTO song_links (song_id, source_id, external_id, synced_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                (song_id, source_id, external_id, synced_at),
            )
            .context("failed to insert song_links row")?;

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
                "UPDATE song_links SET synced_at = ?1 WHERE source_id = ?2 AND external_id = ?3",
                (synced_at, source_id, external_id),
            )
            .context("failed to update song_links row")?;

        Ok(())
    }

    pub fn upsert(
        &self,
        connection: &Connection,
        album_id: i64,
        source_id: i64,
        external_id: &str,
        title: &str,
        disc_number: Option<i64>,
        track_number: Option<i64>,
        duration_seconds: i64,
        synced_at: DateTime<Utc>,
    ) -> Result<Song> {
        if let Some(song) = self.find_by_link(connection, source_id, external_id)? {
            self.touch_link(connection, source_id, external_id, synced_at)?;
            self.update(
                connection,
                song.id,
                album_id,
                title,
                disc_number,
                track_number,
                duration_seconds,
            )
        } else {
            let song = self.insert(
                connection,
                album_id,
                title,
                disc_number,
                track_number,
                duration_seconds,
            )?;
            self.create_link(connection, song.id, source_id, external_id, synced_at)?;
            Ok(song)
        }
    }

    pub fn find_links(&self, connection: &Connection, song_id: i64) -> Result<Vec<SongLink>> {
        let mut stmt = connection
            .prepare(
                "SELECT song_links.source_id as source_id, sources.kind as source_kind, song_links.external_id as external_id \
            FROM song_links \
            JOIN sources ON song_links.source_id = sources.id \
            WHERE song_links.song_id = ?",
            )
            .context("failed to prepare song_links query")?;

        from_rows::<SongLink>(
            stmt.query((song_id,))
                .context("failed to query song_links")?,
        )
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to deserialize song_links")
    }
}

#[derive(Deserialize)]
pub struct SongLink {
    pub source_id: i64,
    pub source_kind: SourceKind,
    pub external_id: String,
}
