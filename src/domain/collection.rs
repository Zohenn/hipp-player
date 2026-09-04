use crate::database::core::database::Database;
use crate::domain::AppEvent;
use crate::open_subsonic::{Album, OpenSubsonicClient, Song};
use color_eyre::eyre::Context;
use color_eyre::{Report, Result};
use std::fmt::Formatter;
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
        }
    }

    pub(super) fn set_client(&mut self, client: OpenSubsonicClient) {
        self.client = client;
    }

    pub fn should_sync(&self) -> bool {
        // TODO
        true
    }

    pub fn sync(&self) {
        let event_tx = self.event_tx.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            let result = Self::inner_sync(event_tx.clone(), client).await;
            event_tx.send(AppEvent::Collection(CollectionEvent::SyncResult(result)));
        });
    }

    async fn inner_sync(
        event_tx: UnboundedSender<AppEvent>,
        client: OpenSubsonicClient,
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
