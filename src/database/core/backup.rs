use crate::database::core::database::{Database, app_data_dir};
use chrono::Local;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use std::path::PathBuf;
use std::time::Duration;

const KEEP_BACKUPS: usize = 3;
// The app can stay open across midnight, so "is today's backup missing" is
// re-checked periodically rather than only at startup.
const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);
const FILE_PREFIX: &str = "data-";
const FILE_SUFFIX: &str = ".db";

#[derive(Clone)]
pub struct DatabaseBackup {
    database: Database,
    dir: PathBuf,
}

impl DatabaseBackup {
    pub fn new(database: Database) -> Result<Self> {
        let dir = app_data_dir().join("backups");
        std::fs::create_dir_all(&dir).context("failed to create backup directory")?;

        Ok(Self { database, dir })
    }

    pub fn create_if_due(&self) -> Result<()> {
        if self.today_path().exists() {
            return Ok(());
        }

        self.create()
    }

    pub fn create(&self) -> Result<()> {
        let path = self.today_path();
        let tmp_path = path.with_extension("db.tmp");

        if tmp_path.exists() {
            std::fs::remove_file(&tmp_path).context("failed to remove stale backup temp file")?;
        }

        // Copying data.db directly would miss writes still sitting in the WAL
        // file; VACUUM INTO produces a consistent snapshot.
        self.database
            .connection()?
            .execute("VACUUM INTO ?1", (tmp_path.to_string_lossy(),))
            .context("failed to write database backup")?;
        std::fs::rename(&tmp_path, &path).context("failed to finalize database backup")?;

        self.prune()
    }

    pub fn spawn_periodic(self) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(CHECK_INTERVAL);
            loop {
                interval.tick().await;
                let backup = self.clone();
                // TODO: surface failures once there's a place to log them.
                let _ = tokio::task::spawn_blocking(move || backup.create_if_due()).await;
            }
        });
    }

    fn today_path(&self) -> PathBuf {
        self.dir.join(format!(
            "{FILE_PREFIX}{}{FILE_SUFFIX}",
            Local::now().format("%Y-%m-%d")
        ))
    }

    fn prune(&self) -> Result<()> {
        let mut backups = std::fs::read_dir(&self.dir)
            .context("failed to list backups")?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with(FILE_PREFIX) && name.ends_with(FILE_SUFFIX)
                    })
            })
            .collect::<Vec<_>>();

        // ISO dates in the names sort chronologically.
        backups.sort();
        let excess = backups.len().saturating_sub(KEEP_BACKUPS);
        for path in &backups[..excess] {
            std::fs::remove_file(path)
                .with_context(|| format!("failed to remove old backup {}", path.display()))?;
        }

        Ok(())
    }
}
