use crate::database::core::database::Database;
use crate::database::source::SourceRepository;
use crate::database::sync_run::SyncRunRepository;
use crate::domain::source::SourceKind;
use crate::domain::sync_run::SyncRunStatus;
use crate::open_subsonic::{Album, OpenSubsonicClient, Song};
use chrono::Local;
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

/// Current, readable-without-a-DB-query view of the collection: the latest
/// sync progress/error (if a sync ever ran) and the albums fetched by the
/// most recent successful/partial sync.
#[derive(Clone, Default)]
pub struct CollectionState {
    pub sync_state: Option<Result<CollectionSyncState, String>>,
    pub albums: Vec<AlbumWithSongs>,
}

pub struct CollectionSyncResult {
    pub albums: Vec<AlbumWithSongs>,
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

            let mut state = state.write().unwrap();
            match result {
                Ok(sync_result) => {
                    state.sync_state = None;
                    state.albums = sync_result.albums;
                }
                Err(err) => {
                    state.sync_state = Some(Err(format!("{err:#}")));
                }
            }
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
        let sync_run_repo = SyncRunRepository::new(database);
        let sync_run_id = sync_run_repo
            .start(source_id)
            .context("failed to start sync run")?;

        let result = Self::fetch(&state, &client).await;

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
            albums: Vec::<AlbumWithSongs>::with_capacity(album_count as usize),
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

                sync_result.albums.push(AlbumWithSongs { album, songs })
            }
        }

        Ok(sync_result)
    }

    fn set_sync_progress(state: &Arc<RwLock<CollectionState>>, sync_state: CollectionSyncState) {
        state.write().unwrap().sync_state = Some(Ok(sync_state));
    }
}

#[derive(Clone)]
pub struct AlbumWithSongs {
    pub album: Album,
    pub songs: Vec<Song>,
}
