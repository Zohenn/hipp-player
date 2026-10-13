use crate::database::core::database::Database;
use crate::database::queue::QueueRepository;
use crate::domain::player::PlayingSongDetails;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;

pub struct QueueService {
    database: Database,
    entries: Vec<PlayingSongDetails>,
    current: Option<usize>,
}

impl QueueService {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            entries: Vec::new(),
            current: None,
        }
    }

    pub fn load_from_db(&mut self) -> Result<()> {
        let connection = self.database.connection()?;
        let repository = QueueRepository::new();
        self.entries = repository
            .list(&connection)
            .context("failed to load persisted queue")?;
        self.current = repository
            .current_position(&connection)?
            .map(|position| position as usize)
            .filter(|&position| position < self.entries.len());

        Ok(())
    }

    pub fn entries(&self) -> &[PlayingSongDetails] {
        &self.entries
    }

    pub fn current(&self) -> Option<(usize, &PlayingSongDetails)> {
        self.current
            .and_then(|index| self.entries.get(index).map(|details| (index, details)))
    }

    pub fn replace(
        &mut self,
        entries: Vec<PlayingSongDetails>,
        start: usize,
    ) -> Result<Option<PlayingSongDetails>> {
        let current = (start < entries.len()).then_some(start);

        let mut connection = self.database.connection()?;
        let tx = connection
            .transaction()
            .context("failed to begin queue transaction")?;
        let repository = QueueRepository::new();
        repository.clear(&tx)?;
        for (position, details) in entries.iter().enumerate() {
            repository.insert(&tx, position as i64, details.song_id)?;
        }
        repository.set_current_position(&tx, current.map(|index| index as i64))?;
        tx.commit().context("failed to commit queue transaction")?;

        self.entries = entries;
        self.current = current;

        Ok(self.current().map(|(_, details)| details.clone()))
    }

    /// Whatever is playing keeps playing; there's just nothing to move to
    /// once it ends.
    pub fn clear(&mut self) -> Result<()> {
        self.replace(Vec::new(), 0)?;

        Ok(())
    }

    pub fn append(&mut self, details: PlayingSongDetails) -> Result<()> {
        let connection = self.database.connection()?;
        QueueRepository::new().insert(&connection, self.entries.len() as i64, details.song_id)?;
        self.entries.push(details);

        Ok(())
    }

    /// Returns the details to play, or `None` if `index` is out of range.
    pub fn select(&mut self, index: usize) -> Result<Option<PlayingSongDetails>> {
        if index >= self.entries.len() {
            return Ok(None);
        }

        let connection = self.database.connection()?;
        QueueRepository::new().set_current_position(&connection, Some(index as i64))?;
        self.current = Some(index);

        Ok(self.entries.get(index).cloned())
    }

    /// Does nothing past the end of the queue.
    pub fn next(&mut self) -> Result<Option<PlayingSongDetails>> {
        match self.current {
            Some(index) => self.select(index + 1),
            // Songs appended to a queue that never played start from the top.
            None => self.select(0),
        }
    }

    /// Does nothing at the start of the queue.
    pub fn previous(&mut self) -> Result<Option<PlayingSongDetails>> {
        match self.current {
            Some(index) if index > 0 => self.select(index - 1),
            _ => Ok(None),
        }
    }
}
