use crate::database::core::database::Database;
use crate::database::core::migrations::get_available_migrations;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;

pub struct DatabaseMigration {
    name: String,
    version: u32,
    up: fn(&Database) -> Result<()>,
}

impl DatabaseMigration {
    pub fn new(name: String, version: u32, up: fn(&Database) -> Result<()>) -> Self {
        Self { name, version, up }
    }
}

pub fn migrate(database: &Database) -> Result<()> {
    let available_migrations = get_available_migrations();

    while let Some(migration) = find_next_migration(database, &available_migrations)? {
        (migration.up)(database)
            .wrap_err_with(|| format!("migration {} failed", migration.name))?;
        database.set_schema_version(migration.version)?;
    }

    Ok(())
}

pub fn has_pending_migrations(database: &Database) -> Result<bool> {
    Ok(find_next_migration(database, &get_available_migrations())?.is_some())
}

fn find_next_migration<'a>(
    database: &Database,
    available_migrations: &'a [DatabaseMigration],
) -> Result<Option<&'a DatabaseMigration>> {
    let current_version = database.get_schema_version()?;
    let next_version = current_version + 1;

    for migration in available_migrations {
        if migration.version == next_version {
            return Ok(Some(migration));
        }
    }

    Ok(None)
}
