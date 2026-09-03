use color_eyre::Result;
use rusqlite::{Connection, Name};
use std::ffi::CStr;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let mut db_file_path = dirs::data_dir().unwrap();
        db_file_path.push("hipp-player");
        std::fs::create_dir_all(&db_file_path)?;
        db_file_path.push("data.db");

        Ok(Self {
            connection: Arc::new(Mutex::new(Connection::open(db_file_path)?)),
        })
    }

    pub fn table_exists(&self, table_name: &CStr) -> Result<bool> {
        Ok(self
            .connection
            .lock()
            .unwrap()
            .table_exists(Some(rusqlite::MAIN_DB), table_name)?)
    }

    pub fn get_schema_version(&self) -> Result<u32> {
        Ok(self
            .connection
            .lock()
            .unwrap()
            .query_row("PRAGMA user_version;", [], |row| row.get(0))?)
    }

    pub fn set_schema_version(&self, version: u32) -> Result<()> {
        self.connection
            .lock()
            .unwrap()
            .execute(&format!("PRAGMA user_version = {};", version), [])?;

        Ok(())
    }

    pub fn connection(&self) -> MutexGuard<Connection> {
        self.connection.lock().unwrap()
    }
}
