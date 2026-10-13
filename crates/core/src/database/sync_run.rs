use crate::database::core::database::Database;
use crate::domain::sync_run::{SyncKind, SyncRunStatus};
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::WrapErr;

pub struct SyncRunRepository {
    database: Database,
}

impl SyncRunRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn start(&self, source_id: i64, kind: SyncKind) -> Result<i64> {
        let connection = self.database.connection()?;

        // Any sync with running state is stale and probably never finished due to app being killed or crashing mid-sync - mark those as failed
        connection
            .execute(
                "UPDATE sync_runs \
                 SET status = 'failed', error = 'interrupted (app restart)', completed_at = ?1 \
                 WHERE source_id = ?2 AND status = 'running'",
                (Utc::now(), source_id),
            )
            .context("failed to reconcile stale sync_runs rows")?;

        connection
            .query_row(
                "INSERT INTO sync_runs (source_id, kind, status, started_at) \
                 VALUES (?1, ?2, 'running', ?3) RETURNING id",
                (source_id, kind.as_str(), Utc::now()),
                |row| row.get(0),
            )
            .context("failed to insert sync_runs row")
    }

    pub fn complete(
        &self,
        id: i64,
        status: SyncRunStatus,
        error: Option<&str>,
        completed_at: DateTime<Utc>,
    ) -> Result<()> {
        self.database
            .connection()?
            .execute(
                "UPDATE sync_runs SET status = ?1, error = ?2, completed_at = ?3 WHERE id = ?4",
                (status.as_str(), error, completed_at, id),
            )
            .context("failed to complete sync_runs row")?;

        Ok(())
    }

    /// `kind: None` matches runs of any kind.
    pub fn last_completed_at(
        &self,
        source_id: i64,
        kind: Option<SyncKind>,
    ) -> Result<Option<DateTime<Utc>>> {
        self.last_completed_column("completed_at", source_id, kind)
    }

    /// Start time (not completion time) of the most recent completed run of
    /// any kind, so anything added on the server while it was running is
    /// still considered new by the next incremental sync.
    pub fn last_completed_started_at(&self, source_id: i64) -> Result<Option<DateTime<Utc>>> {
        self.last_completed_column("started_at", source_id, None)
    }

    fn last_completed_column(
        &self,
        column: &str,
        source_id: i64,
        kind: Option<SyncKind>,
    ) -> Result<Option<DateTime<Utc>>> {
        self.database
            .connection()?
            .query_row(
                &format!(
                    "SELECT MAX({column}) FROM sync_runs \
                     WHERE source_id = ?1 AND status IN ('success', 'partial') \
                       AND (?2 IS NULL OR kind = ?2)"
                ),
                (source_id, kind.map(|kind| kind.as_str())),
                |row| row.get(0),
            )
            .context("failed to query last completed sync run")
    }
}
