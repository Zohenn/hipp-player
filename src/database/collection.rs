use crate::database::core::database::Database;
use crate::domain::album::AlbumWithSongs;
use crate::domain::collection::CollectionSyncResult;
use crate::domain::source::Source;
use color_eyre::Result;

pub struct CollectionRepository {
    database: Database,
}

impl CollectionRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn store_sync_result(&self, source: &Source, result: &CollectionSyncResult) -> Result<()> {
        todo!()
    }

    pub fn albums(&self) -> Result<Vec<AlbumWithSongs>> {
        todo!()
    }
}
