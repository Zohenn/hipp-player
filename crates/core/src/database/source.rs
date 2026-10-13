use crate::database::core::database::Database;
use crate::domain::source::SourceKind;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use rusqlite::OptionalExtension;

pub struct SourceRepository {
    database: Database,
}

impl SourceRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn find_or_create(&self, kind: SourceKind, identifier: &str) -> Result<i64> {
        let connection = self.database.connection()?;
        let kind = kind.as_str();

        let existing = connection
            .query_row(
                "SELECT id FROM sources WHERE kind = ?1 AND identifier = ?2",
                (kind, identifier),
                |row| row.get(0),
            )
            .optional()
            .context("failed to query sources")?;

        if let Some(id) = existing {
            return Ok(id);
        }

        connection
            .query_row(
                "INSERT INTO sources (kind, identifier) VALUES (?1, ?2) RETURNING id",
                (kind, identifier),
                |row| row.get(0),
            )
            .context("failed to insert sources row")
    }
}
