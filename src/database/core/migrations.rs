use crate::database::core::database::Database;
use crate::database::core::migration::DatabaseMigration;

pub fn get_available_migrations() -> Vec<DatabaseMigration> {
    vec![version1()]
}

fn version1() -> DatabaseMigration {
    DatabaseMigration::new("Initial migration".into(), 1, |database: &Database| {
        database.connection().execute(
            "\
CREATE TABLE IF NOT EXISTS client_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    url TEXT NOT NULL,
    username VARCHAR(255) NOT NULL,
    password BLOB NOT NULL,
    nonce BLOD NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)
",
            (),
        )?;

        Ok(())
    })
}
