use crate::database::album::AlbumRepository;
use crate::database::artist::ArtistRepository;
use crate::database::core::database::Database;
use crate::database::cover_art::CoverArtRepository;
use crate::database::song::SongRepository;
use crate::database::source::SourceRepository;
use crate::database::sync_run::SyncRunRepository;
use crate::domain::album::Album as PersistedAlbum;
use crate::domain::artist::Artist as PersistedArtist;
use crate::domain::song::Song as PersistedSong;
use crate::domain::source::SourceKind;
use crate::domain::sync_run::{SyncKind, SyncRunStatus};
use crate::open_subsonic::{Album, OpenSubsonicClient, Song};
use chrono::{DateTime, Local, TimeDelta, Utc};
use color_eyre::eyre::Context;
use color_eyre::{Report, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Formatter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

const FULL_SYNC_INTERVAL: TimeDelta = TimeDelta::days(7);
// Re-checks albums added shortly before the previous sync's start, absorbing
// clock skew between us and the server; re-syncing an album is harmless.
const INCREMENTAL_SYNC_OVERLAP: TimeDelta = TimeDelta::days(1);
const NEWEST_ALBUMS_PAGE_SIZE: u32 = 50;

pub enum CollectionAction {
    Sync,
}

#[derive(Clone)]
pub enum CollectionSyncState {
    Started,
    Fetching {
        artist: String,
        album: String,
        album_number: u32,
        album_count: u32,
    },
}

#[derive(Clone, Default)]
pub struct CollectionState {
    pub sync_state: Option<Result<CollectionSyncState, String>>,
    pub albums: Vec<PersistedAlbum>,
    pub artists: HashMap<i64, PersistedArtist>,
    pub last_synced_at: Option<DateTime<Utc>>,
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

    /// Borrows the state under a read lock instead of cloning it, so callers
    /// like per-frame rendering don't copy the whole collection. Keep `f`
    /// short — sync progress updates block while it runs.
    pub fn with_state<R>(&self, f: impl FnOnce(&CollectionState) -> R) -> R {
        f(&self.state.read().unwrap())
    }

    pub fn load_from_db(&self) -> Result<()> {
        let connection = self.database.connection()?;
        let albums = AlbumRepository::new()
            .list_all(&connection)
            .context("failed to load persisted collection")?;
        let artists = ArtistRepository::new()
            .list_all(&connection)
            .context("failed to load persisted collection")?;
        // Same definition of "synced" as due_sync: failed runs don't count.
        let last_synced_at = SyncRunRepository::new(self.database.clone())
            .last_completed_at(self.source_id()?, None)
            .context("failed to check last sync time")?;

        let mut state = self.state.write().unwrap();
        state.albums = albums;
        state.artists = artists
            .into_iter()
            .map(|artist| (artist.id, artist))
            .collect();
        state.last_synced_at = last_synced_at;

        Ok(())
    }

    /// Songs aren't kept in state — only the opened album's are needed, so
    /// they're read from the db on demand.
    pub fn album_songs(&self, album_id: i64) -> Result<Vec<PersistedSong>> {
        let connection = self.database.connection()?;
        SongRepository::new()
            .list_by_album(&connection, album_id)
            .context("failed to load album songs")
    }

    pub fn due_sync(&self) -> Result<Option<SyncKind>> {
        let source_id = self.source_id()?;
        let sync_run_repo = SyncRunRepository::new(self.database.clone());

        let last_full_at = sync_run_repo
            .last_completed_at(source_id, Some(SyncKind::Full))
            .context("failed to check last full sync time")?;
        if last_full_at.is_none_or(|last_full_at| Utc::now() - last_full_at >= FULL_SYNC_INTERVAL) {
            return Ok(Some(SyncKind::Full));
        }

        let last_any_at = sync_run_repo
            .last_completed_at(source_id, None)
            .context("failed to check last sync time")?;
        let synced_today = last_any_at.is_some_and(|last_any_at| {
            last_any_at.with_timezone(&Local).date_naive() >= Local::now().date_naive()
        });

        Ok((!synced_today).then_some(SyncKind::Incremental))
    }

    fn source_id(&self) -> Result<i64> {
        SourceRepository::new(self.database.clone())
            .find_or_create(SourceKind::OpenSubsonic, &self.client.options().url)
            .context("failed to resolve source")
    }

    pub fn sync(&self, kind: SyncKind) {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return;
        }

        let client = self.client.clone();
        let database = self.database.clone();
        let syncing = self.syncing.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            let result = Self::inner_sync(state.clone(), client, database, kind).await;
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
        kind: SyncKind,
    ) -> Result<CollectionSyncResult> {
        let source_id = SourceRepository::new(database.clone())
            .find_or_create(SourceKind::OpenSubsonic, &client.options().url)
            .context("failed to resolve source")?;
        let sync_run_repo = SyncRunRepository::new(database.clone());
        // Read before starting the new run, otherwise the cutoff would be the
        // new run's own start time.
        let incremental_since = match kind {
            SyncKind::Full => None,
            SyncKind::Incremental => sync_run_repo
                .last_completed_started_at(source_id)
                .context("failed to check last sync time")?
                .map(|started_at| started_at - INCREMENTAL_SYNC_OVERLAP),
        };
        // Without a previous run there's nothing to be incremental against.
        let kind = if incremental_since.is_some() {
            SyncKind::Incremental
        } else {
            SyncKind::Full
        };

        let sync_run_id = sync_run_repo
            .start(source_id, kind)
            .context("failed to start sync run")?;

        let result = Self::fetch(&state, &client, &database, source_id, incremental_since).await;

        match &result {
            Ok(sync_result) => {
                // Best-effort: attribution now comes from each album's own
                // artist id, so an artist-index entry that turned out to be a
                // duplicate of another can be left with zero albums. Prune
                // those rather than leaving them dangling.
                let _ = Self::prune_orphaned_artists(&database, source_id).await;

                // TODO: after a successful full sync, albums/songs whose link
                // `synced_at` predates this run no longer exist on the server.
                // Don't delete them in the background — surface them to the
                // user and let them decide what to do.

                let status = if sync_result.artist_errors.is_empty()
                    && sync_result.album_errors.is_empty()
                {
                    SyncRunStatus::Success
                } else {
                    SyncRunStatus::Partial
                };
                let completed_at = Utc::now();
                sync_run_repo.complete(sync_run_id, status, None, completed_at)?;
                state.write().unwrap().last_synced_at = Some(completed_at);
            }
            Err(err) => {
                sync_run_repo.complete(
                    sync_run_id,
                    SyncRunStatus::Failed,
                    Some(&format!("{err:#}")),
                    Utc::now(),
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
        incremental_since: Option<DateTime<Utc>>,
    ) -> Result<CollectionSyncResult> {
        Self::set_sync_progress(state, CollectionSyncState::Started);

        let mut sync_result = CollectionSyncResult {
            artist_errors: Vec::new(),
            album_errors: Vec::new(),
        };

        let albums = match incremental_since {
            None => Self::discover_all_albums(client, &mut sync_result).await?,
            Some(since) => Self::discover_new_albums(client, since).await?,
        };

        let album_count = albums.len() as u32;
        for (album_index, album) in albums.into_iter().enumerate() {
            Self::set_sync_progress(
                state,
                CollectionSyncState::Fetching {
                    artist: album.artist.clone(),
                    album: album.name.clone(),
                    album_number: (album_index + 1) as u32,
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

            match Self::persist_album(database, source_id, &album, &songs, Utc::now()).await {
                Ok((db_artist, db_album)) => Self::upsert_state_album(state, db_artist, db_album),
                Err(err) => sync_result.album_errors.push(AlbumError {
                    name: album.name,
                    error: err,
                }),
            }
        }

        Ok(sync_result)
    }

    async fn discover_all_albums(
        client: &OpenSubsonicClient,
        sync_result: &mut CollectionSyncResult,
    ) -> Result<Vec<Album>> {
        let artists = client
            .get_artists()
            .await
            .context("could not fetch artist list")?;

        // The artist index can list the same real artist under multiple
        // entries with overlapping album sets, so discovery is deduped by
        // album id up front — otherwise a shared album gets fetched,
        // persisted, and pushed into state once per duplicate entry.
        let mut albums = Vec::new();
        let mut seen_album_ids = HashSet::new();
        for artist in artists {
            match client
                .get_artist_albums(&artist.id)
                .await
                .context("could not fetch album list")
            {
                Ok(artist_albums) => {
                    for album in artist_albums {
                        if seen_album_ids.insert(album.id.clone()) {
                            albums.push(album);
                        }
                    }
                }
                Err(err) => sync_result.artist_errors.push(ArtistError {
                    name: artist.name,
                    error: err,
                }),
            }
        }

        Ok(albums)
    }

    /// Pages through the server's albums newest-first until reaching ones
    /// added before `since`. Albums without a `created` date can't be placed
    /// in time, so they're synced and paging continues past them.
    async fn discover_new_albums(
        client: &OpenSubsonicClient,
        since: DateTime<Utc>,
    ) -> Result<Vec<Album>> {
        let mut albums = Vec::new();
        let mut seen_album_ids = HashSet::new();
        let mut offset = 0;

        loop {
            let page = client
                .get_newest_albums(NEWEST_ALBUMS_PAGE_SIZE, offset)
                .await
                .context("could not fetch newest albums")?;
            let page_len = page.len() as u32;

            let mut reached_known = false;
            for album in page {
                if album.created.is_some_and(|created| created < since) {
                    reached_known = true;
                    break;
                }
                // Pages can shift if albums are added mid-sync.
                if seen_album_ids.insert(album.id.clone()) {
                    albums.push(album);
                }
            }

            if reached_known || page_len < NEWEST_ALBUMS_PAGE_SIZE {
                return Ok(albums);
            }
            offset += page_len;
        }
    }

    async fn prune_orphaned_artists(database: &Database, source_id: i64) -> Result<()> {
        let database = database.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let connection = database.connection()?;
            ArtistRepository::new().delete_orphaned(&connection, source_id)
        })
        .await
        .context("artist cleanup task panicked")?
    }

    /// Attribution uses the album's own reported artist id/name, not whichever
    /// artist-index entry we discovered the album under — OpenSubsonic servers
    /// have been observed listing the same real artist under multiple index
    /// entries with overlapping album sets, but each album itself reports a
    /// single unambiguous artist.
    async fn persist_album(
        database: &Database,
        source_id: i64,
        album: &Album,
        songs: &[Song],
        synced_at: DateTime<Utc>,
    ) -> Result<(PersistedArtist, PersistedAlbum)> {
        let database = database.clone();
        let album = album.clone();
        let songs = songs.to_vec();

        tokio::task::spawn_blocking(move || -> Result<(PersistedArtist, PersistedAlbum)> {
            let mut connection = database.connection()?;
            let tx = connection
                .transaction()
                .context("failed to begin album sync transaction")?;

            let db_artist = ArtistRepository::new().upsert(
                &tx,
                source_id,
                &album.artist_id,
                &album.artist,
                synced_at,
            )?;

            let (db_album, album_link_id) = AlbumRepository::new().upsert(
                &tx,
                db_artist.id,
                source_id,
                &album.id,
                &album.name,
                None,
                synced_at,
            )?;

            let cover_art_repo = CoverArtRepository::new();
            match &album.cover_art {
                Some(cover_art) => {
                    cover_art_repo.upsert(&tx, album_link_id, cover_art, synced_at)?
                }
                None => cover_art_repo.delete_by_link(&tx, album_link_id)?,
            }

            let song_repo = SongRepository::new();
            for song in &songs {
                song_repo.upsert(
                    &tx,
                    db_album.id,
                    source_id,
                    &song.id,
                    &song.title,
                    song.disc_number.map(i64::from),
                    Some(song.track as i64),
                    song.duration.get() as i64,
                    synced_at,
                )?;
            }

            tx.commit()
                .context("failed to commit album sync transaction")?;

            Ok((db_artist, db_album))
        })
        .await
        .context("album persistence task panicked")?
    }

    /// State is pre-filled from the local db on startup, so a synced album may
    /// already be present — replace it in place rather than appending a duplicate.
    fn upsert_state_album(
        state: &Arc<RwLock<CollectionState>>,
        artist: PersistedArtist,
        album: PersistedAlbum,
    ) {
        let mut state = state.write().unwrap();
        state.artists.insert(artist.id, artist);
        let albums = &mut state.albums;
        match albums.iter_mut().find(|existing| existing.id == album.id) {
            Some(existing) => *existing = album,
            None => albums.push(album),
        }
    }

    fn set_sync_progress(state: &Arc<RwLock<CollectionState>>, sync_state: CollectionSyncState) {
        state.write().unwrap().sync_state = Some(Ok(sync_state));
    }
}
