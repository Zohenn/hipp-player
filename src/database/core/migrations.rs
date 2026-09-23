use crate::database::core::database::Database;
use crate::database::core::migration::DatabaseMigration;

pub fn get_available_migrations() -> Vec<DatabaseMigration> {
    vec![version1(), version2(), version3()]
}

fn version1() -> DatabaseMigration {
    DatabaseMigration::new("Initial migration".into(), 1, |database: &Database| {
        database.connection()?.execute(
            "\
CREATE TABLE IF NOT EXISTS client_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    url TEXT NOT NULL,
    username VARCHAR(255) NOT NULL,
    password BLOB NOT NULL,
    nonce BLOB NOT NULL,
    created_at TIMESTAMP DEFAULT (strftime('%Y-%m-%d %H:%M:%SZ', 'now'))
)
",
            (),
        )?;

        Ok(())
    })
}

fn version2() -> DatabaseMigration {
    DatabaseMigration::new("Collection tables".into(), 2, |database: &Database| {
        database.connection()?.execute_batch(
            "\
CREATE TABLE sources (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('opensubsonic', 'spotify')),
    identifier TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT (strftime('%Y-%m-%d %H:%M:%SZ', 'now')),
    UNIQUE (kind, identifier)
);

CREATE TABLE artists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT (strftime('%Y-%m-%d %H:%M:%SZ', 'now'))
);

CREATE TABLE albums (
    id INTEGER PRIMARY KEY,
    artist_id INTEGER NOT NULL REFERENCES artists(id),
    name TEXT NOT NULL,
    cover_art TEXT,
    created_at TIMESTAMP DEFAULT (strftime('%Y-%m-%d %H:%M:%SZ', 'now'))
);

CREATE TABLE songs (
    id INTEGER PRIMARY KEY,
    album_id INTEGER NOT NULL REFERENCES albums(id),
    title TEXT NOT NULL,
    disc_number INTEGER,
    track_number INTEGER,
    duration_seconds INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT (strftime('%Y-%m-%d %H:%M:%SZ', 'now'))
);

CREATE TABLE artist_links (
    id INTEGER PRIMARY KEY,
    artist_id INTEGER NOT NULL REFERENCES artists(id),
    source_id INTEGER NOT NULL REFERENCES sources(id),
    external_id TEXT NOT NULL,
    synced_at TIMESTAMP NOT NULL,
    UNIQUE (source_id, external_id)
);

CREATE TABLE album_links (
    id INTEGER PRIMARY KEY,
    album_id INTEGER NOT NULL REFERENCES albums(id),
    source_id INTEGER NOT NULL REFERENCES sources(id),
    external_id TEXT NOT NULL,
    music_folder_id TEXT,
    synced_at TIMESTAMP NOT NULL,
    UNIQUE (source_id, external_id)
);

CREATE TABLE song_links (
    id INTEGER PRIMARY KEY,
    song_id INTEGER NOT NULL REFERENCES songs(id),
    source_id INTEGER NOT NULL REFERENCES sources(id),
    external_id TEXT NOT NULL,
    synced_at TIMESTAMP NOT NULL,
    UNIQUE (source_id, external_id)
);
",
        )?;

        Ok(())
    })
}

fn version3() -> DatabaseMigration {
    DatabaseMigration::new("Sync run history".into(), 3, |database: &Database| {
        database.connection()?.execute_batch(
            "\
CREATE TABLE sync_runs (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL REFERENCES sources(id),
    started_at TIMESTAMP NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%SZ', 'now')),
    completed_at TIMESTAMP,
    status TEXT NOT NULL CHECK (status IN ('running', 'success', 'failed', 'partial')),
    error TEXT
);

CREATE INDEX idx_sync_runs_source_completed ON sync_runs (source_id, completed_at);
",
        )?;

        Ok(())
    })
}
