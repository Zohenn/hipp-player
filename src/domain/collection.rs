use crate::database::core::database::Database;
use crate::database::source::SourceRepository;
use crate::database::sync_run::SyncRunRepository;
use crate::domain::AppEvent;
use crate::domain::source::SourceKind;
use crate::domain::sync_run::SyncRunStatus;
use crate::open_subsonic::{Album, OpenSubsonicClient, Song};
use chrono::Local;
use color_eyre::eyre::Context;
use color_eyre::{Report, Result};
use std::fmt::Formatter;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::UnboundedSender;

pub enum CollectionAction {
    Sync,
}

pub enum CollectionEvent {
    SyncState(CollectionSyncState),
    SyncResult(Result<CollectionSyncResult>),
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
    event_tx: UnboundedSender<AppEvent>,
    syncing: Arc<AtomicBool>,
}

impl CollectionService {
    pub fn new(
        database: Database,
        client: OpenSubsonicClient,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Self {
        Self {
            database,
            client,
            event_tx,
            syncing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn set_client(&mut self, client: OpenSubsonicClient) {
        self.client = client;
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

        let event_tx = self.event_tx.clone();
        let client = self.client.clone();
        let database = self.database.clone();
        let syncing = self.syncing.clone();
        tokio::spawn(async move {
            let result = Self::inner_sync(event_tx.clone(), client, database).await;
            syncing.store(false, Ordering::SeqCst);
            event_tx.send(AppEvent::Collection(CollectionEvent::SyncResult(result)));
        });
    }

    async fn inner_sync(
        event_tx: UnboundedSender<AppEvent>,
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

        let result = Self::fetch(&event_tx, &client).await;

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
        event_tx: &UnboundedSender<AppEvent>,
        client: &OpenSubsonicClient,
    ) -> Result<CollectionSyncResult> {
        event_tx.send(AppEvent::Collection(CollectionEvent::SyncState(
            CollectionSyncState::Started,
        )))?;

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
                event_tx.send(AppEvent::Collection(CollectionEvent::SyncState(
                    CollectionSyncState::Fetching {
                        artist: artist.name.clone(),
                        artist_number: (artist_index + 1) as u32,
                        artist_count,
                        artist_album_number: (album_index + 1) as u32,
                        artist_album_count: artist.album_count,
                        album_number,
                        album_count,
                    },
                )))?;

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
}

#[derive(Clone)]
pub struct AlbumWithSongs {
    pub album: Album,
    pub songs: Vec<Song>,
}
