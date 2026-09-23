use crate::database::core::database::Database;
use crate::domain::sync_run::SyncRunStatus;
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

    pub fn start(&self, source_id: i64) -> Result<i64> {
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
                "INSERT INTO sync_runs (source_id, status) VALUES (?1, 'running') RETURNING id",
                (source_id,),
                |row| row.get(0),
            )
            .context("failed to insert sync_runs row")
    }

    pub fn complete(&self, id: i64, status: SyncRunStatus, error: Option<&str>) -> Result<()> {
        self.database
            .connection()?
            .execute(
                "UPDATE sync_runs SET status = ?1, error = ?2, completed_at = ?3 WHERE id = ?4",
                (status.as_str(), error, Utc::now(), id),
            )
            .context("failed to complete sync_runs row")?;

        Ok(())
    }

    pub fn last_completed_at(&self, source_id: i64) -> Result<Option<DateTime<Utc>>> {
        self.database
            .connection()?
            .query_row(
                "SELECT MAX(completed_at) FROM sync_runs \
                 WHERE source_id = ?1 AND status IN ('success', 'partial')",
                (source_id,),
                |row| row.get(0),
            )
            .context("failed to query last completed sync run")
    }
}
