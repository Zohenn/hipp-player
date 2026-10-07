use crate::domain::player::PlayingSongDetails;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use rusqlite::{Connection, OptionalExtension};
use serde_rusqlite::from_rows;

pub struct QueueRepository;

impl QueueRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self, connection: &Connection) -> Result<Vec<PlayingSongDetails>> {
        let mut stmt = connection
            .prepare(
                "SELECT songs.id AS song_id, songs.title AS song_title, \
                 songs.duration_seconds AS song_duration, artists.name AS artist_name, \
                 albums.id AS album_id, albums.name AS album_name \
                 FROM queue \
                 JOIN songs ON songs.id = queue.song_id \
                 JOIN albums ON albums.id = songs.album_id \
                 JOIN artists ON artists.id = albums.artist_id \
                 ORDER BY queue.position",
            )
            .context("failed to prepare queue query")?;

        from_rows::<PlayingSongDetails>(stmt.query(()).context("failed to query queue")?)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to deserialize queue")
    }

    pub fn clear(&self, connection: &Connection) -> Result<()> {
        connection
            .execute("DELETE FROM queue", ())
            .context("failed to clear queue")?;

        Ok(())
    }

    pub fn insert(&self, connection: &Connection, position: i64, song_id: i64) -> Result<()> {
        connection
            .execute(
                "INSERT INTO queue (position, song_id) VALUES (?1, ?2)",
                (position, song_id),
            )
            .context("failed to insert queue row")?;

        Ok(())
    }

    pub fn current_position(&self, connection: &Connection) -> Result<Option<i64>> {
        Ok(connection
            .query_row(
                "SELECT current_position FROM queue_state WHERE id = 1",
                (),
                |row| row.get(0),
            )
            .optional()
            .context("failed to query queue_state")?
            .flatten())
    }

    pub fn set_current_position(
        &self,
        connection: &Connection,
        position: Option<i64>,
    ) -> Result<()> {
        connection
            .execute(
                "INSERT INTO queue_state (id, current_position) VALUES (1, ?1) \
                 ON CONFLICT (id) DO UPDATE SET current_position = excluded.current_position",
                (position,),
            )
            .context("failed to update queue_state")?;

        Ok(())
    }
}
