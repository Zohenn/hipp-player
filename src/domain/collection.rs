use crate::database::album::AlbumRepository;
use crate::database::artist::ArtistRepository;
use crate::database::core::database::Database;
use crate::database::song::SongRepository;
use crate::database::source::SourceRepository;
use crate::database::sync_run::SyncRunRepository;
use crate::domain::album::Album as PersistedAlbum;
use crate::domain::source::SourceKind;
use crate::domain::sync_run::SyncRunStatus;
use crate::open_subsonic::{Album, OpenSubsonicClient, Song};
use chrono::{DateTime, Local, Utc};
use color_eyre::eyre::Context;
use color_eyre::{Report, Result};
use std::fmt::Formatter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

pub enum CollectionAction {
    Sync,
}

#[derive(Clone)]
pub enum CollectionSyncState {
    Started,
    Fetching {
        artist: String,
        artist_number: u32,
        artist_count: u32,
        artist_album_number: u32,
        artist_album_count: u32,
        album_number: u32,
        album_count: u32,
    },
}

#[derive(Clone, Default)]
pub struct CollectionState {
    pub sync_state: Option<Result<CollectionSyncState, String>>,
    pub albums: Vec<PersistedAlbum>,
}

pub struct CollectionSyncResult {
    pub artist_errors: Vec<ArtistError>,
    pub album_errors: Vec<AlbumError>,
}

pub struct ArtistError {
    name: String,
    error: Report,
}

impl std::fmt::Display for ArtistError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:#}", self.name, self.error)
    }
}

pub struct AlbumError {
    name: String,
    error: Report,
}

impl std::fmt::Display for AlbumError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:#}", self.name, self.error)
    }
}

#[derive(Clone)]
pub struct CollectionService {
    database: Database,
    client: OpenSubsonicClient,
    state: Arc<RwLock<CollectionState>>,
    syncing: Arc<AtomicBool>,
}

impl CollectionService {
    pub fn new(database: Database, client: OpenSubsonicClient) -> Self {
        Self {
            database,
            client,
            state: Arc::new(RwLock::new(CollectionState::default())),
            syncing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn set_client(&mut self, client: OpenSubsonicClient) {
        self.client = client;
    }

    pub fn state(&self) -> CollectionState {
        self.state.read().unwrap().clone()
    }

    pub fn load_from_db(&self) -> Result<()> {
        let connection = self.database.connection()?;
        let albums = AlbumRepository::new()
            .list_all(&connection)
            .context("failed to load persisted collection")?;

        self.state.write().unwrap().albums = albums;

        Ok(())
    }

    pub fn should_sync(&self) -> Result<bool> {
        let source_id = self.source_id()?;
        let last_completed_at = SyncRunRepository::new(self.database.clone())
            .last_completed_at(source_id)
            .context("failed to check last sync time")?;

        Ok(match last_completed_at {
            None => true,
            Some(last_completed_at) => {
                last_completed_at.with_timezone(&Local).date_naive() < Local::now().date_naive()
            }
        })
    }

    fn source_id(&self) -> Result<i64> {
        SourceRepository::new(self.database.clone())
            .find_or_create(SourceKind::OpenSubsonic, &self.client.options().url)
            .context("failed to resolve source")
    }

    pub fn sync(&self) {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return;
        }

        let client = self.client.clone();
        let database = self.database.clone();
        let syncing = self.syncing.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            let result = Self::inner_sync(state.clone(), client, database).await;
            syncing.store(false, Ordering::SeqCst);

            if let Err(err) = result {
                state.write().unwrap().sync_state = Some(Err(format!("{err:#}")));
                return;
            }

            state.write().unwrap().sync_state = None;
        });
    }

    async fn inner_sync(
        state: Arc<RwLock<CollectionState>>,
        client: OpenSubsonicClient,
        database: Database,
    ) -> Result<CollectionSyncResult> {
        let source_id = SourceRepository::new(database.clone())
            .find_or_create(SourceKind::OpenSubsonic, &client.options().url)
            .context("failed to resolve source")?;
        let sync_run_repo = SyncRunRepository::new(database.clone());
        let sync_run_id = sync_run_repo
            .start(source_id)
            .context("failed to start sync run")?;

        let result = Self::fetch(&state, &client, &database, source_id).await;

        match &result {
            Ok(sync_result) => {
                let status = if sync_result.artist_errors.is_empty()
                    && sync_result.album_errors.is_empty()
                {
                    SyncRunStatus::Success
                } else {
                    SyncRunStatus::Partial
                };
                sync_run_repo.complete(sync_run_id, status, None)?;
            }
            Err(err) => {
                sync_run_repo.complete(
                    sync_run_id,
                    SyncRunStatus::Failed,
                    Some(&format!("{err:#}")),
                )?;
            }
        }

        result
    }

    async fn fetch(
        state: &Arc<RwLock<CollectionState>>,
        client: &OpenSubsonicClient,
        database: &Database,
        source_id: i64,
    ) -> Result<CollectionSyncResult> {
        Self::set_sync_progress(state, CollectionSyncState::Started);

        let artists = client
            .get_artists()
            .await
            .context("could not fetch artist list")?;

        let album_count: u32 = artists.iter().map(|artist| artist.album_count).sum();
        let artist_count = artists.len() as u32;

        let mut album_number = 0u32;
        let mut sync_result = CollectionSyncResult {
            artist_errors: Vec::new(),
            album_errors: Vec::new(),
        };

        for (artist_index, artist) in artists.into_iter().enumerate() {
            let albums = match client
                .get_artist_albums(&artist.id)
                .await
                .context("could not fetch album list")
            {
                Ok(albums) => albums,
                Err(err) => {
                    sync_result.artist_errors.push(ArtistError {
                        name: artist.name,
                        error: err,
                    });
                    continue;
                }
            };

            for (album_index, album) in albums.into_iter().enumerate() {
                album_number += 1;
                Self::set_sync_progress(
                    state,
                    CollectionSyncState::Fetching {
                        artist: artist.name.clone(),
                        artist_number: (artist_index + 1) as u32,
                        artist_count,
                        artist_album_number: (album_index + 1) as u32,
                        artist_album_count: artist.album_count,
                        album_number,
                        album_count,
                    },
                );

                let songs = match client
                    .get_album_songs(&album.id)
                    .await
                    .context("could not fetch songs list")
                {
                    Ok(songs) => songs,
                    Err(err) => {
                        sync_result.album_errors.push(AlbumError {
                            name: album.name,
                            error: err,
                        });
                        continue;
                    }
                };

                match Self::persist_album(
                    database,
                    source_id,
                    &artist.id,
                    &artist.name,
                    &album,
                    &songs,
                    Utc::now(),
                )
                .await
                {
                    Ok(db_album) => state.write().unwrap().albums.push(db_album),
                    Err(err) => sync_result.album_errors.push(AlbumError {
                        name: album.name,
                        error: err,
                    }),
                }
            }
        }

        Ok(sync_result)
    }

    async fn persist_album(
        database: &Database,
        source_id: i64,
        artist_external_id: &str,
        artist_name: &str,
        album: &Album,
        songs: &[Song],
        synced_at: DateTime<Utc>,
    ) -> Result<PersistedAlbum> {
        let database = database.clone();
        let artist_external_id = artist_external_id.to_string();
        let artist_name = artist_name.to_string();
        let album = album.clone();
        let songs = songs.to_vec();

        tokio::task::spawn_blocking(move || -> Result<PersistedAlbum> {
            let mut connection = database.connection()?;
            let tx = connection
                .transaction()
                .context("failed to begin album sync transaction")?;

            let db_artist = ArtistRepository::new().upsert(
                &tx,
                source_id,
                &artist_external_id,
                &artist_name,
                synced_at,
            )?;

            let db_album = AlbumRepository::new().upsert(
                &tx,
                db_artist.id,
                source_id,
                &album.id,
                &album.name,
                Some(&album.cover_art),
                None,
                synced_at,
            )?;

            let song_repo = SongRepository::new();
            for song in &songs {
                song_repo.upsert(
                    &tx,
                    db_album.id,
                    source_id,
                    &song.id,
                    &song.title,
                    None,
                    Some(song.track as i64),
                    song.duration.get() as i64,
                    synced_at,
                )?;
            }

            tx.commit()
                .context("failed to commit album sync transaction")?;

            Ok(db_album)
        })
        .await
        .context("album persistence task panicked")?
    }

    fn set_sync_progress(state: &Arc<RwLock<CollectionState>>, sync_state: CollectionSyncState) {
        state.write().unwrap().sync_state = Some(Ok(sync_state));
    }
}
