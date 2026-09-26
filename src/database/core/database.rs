use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use r2d2::{CustomizeConnection, Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, Name};
use std::ffi::CStr;
use std::path::PathBuf;

const MAX_POOL_SIZE: u32 = 8;

pub fn app_data_dir() -> PathBuf {
    dirs::data_dir().unwrap().join("hipp-player")
}

#[derive(Debug)]
struct ConnectionPragmas;

impl CustomizeConnection<Connection, rusqlite::Error> for ConnectionPragmas {
    fn on_acquire(&self, connection: &mut Connection) -> Result<(), rusqlite::Error> {
        connection.execute_batch(
            "\
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
",
        )
    }
}

#[derive(Clone)]
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let data_dir = app_data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let db_file_path = data_dir.join("data.db");

        let manager = SqliteConnectionManager::file(db_file_path);
        let pool = Pool::builder()
            .max_size(MAX_POOL_SIZE)
            .connection_customizer(Box::new(ConnectionPragmas))
            .build(manager)
            .context("failed to build database connection pool")?;

        Ok(Self { pool })
    }

    pub fn table_exists(&self, table_name: &CStr) -> Result<bool> {
        Ok(self
            .connection()?
            .table_exists(Some(rusqlite::MAIN_DB), table_name)?)
    }

    pub fn get_schema_version(&self) -> Result<u32> {
        Ok(self
            .connection()?
            .query_row("PRAGMA user_version;", [], |row| row.get(0))?)
    }

    pub fn set_schema_version(&self, version: u32) -> Result<()> {
        self.connection()?
            .execute(&format!("PRAGMA user_version = {};", version), [])?;

        Ok(())
    }

    pub fn connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .context("failed to check out a database connection from the pool")
    }
}
