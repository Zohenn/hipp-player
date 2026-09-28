use crate::database::core::database::{Database, app_data_dir};
use crate::database::cover_art::CoverArtRepository;
use crate::domain::AppEvent;
use crate::open_subsonic::OpenSubsonicClient;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use image::DynamicImage;
use image::imageops::FilterType;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

/// Edge length in pixels covers are fetched and cached at — plenty for a
/// terminal, and keeps the originals (often several MB) off the disk.
const COVER_SIZE: u32 = 600;
const WEBP_QUALITY: f32 = 80.0;
const MEMORY_CACHE_CAPACITY: NonZeroUsize = NonZeroUsize::new(32).unwrap();

#[derive(serde::Deserialize)]
pub struct AlbumCover {
    pub source_id: i64,
    pub external_id: String,
}

pub enum CoverArtEvent {
    /// `Ok(None)` means the album has no cover.
    Loaded {
        album_id: i64,
        result: Result<Option<Arc<DynamicImage>>, String>,
    },
}

#[derive(Clone)]
pub struct CoverArtService {
    database: Database,
    client: OpenSubsonicClient,
    event_tx: UnboundedSender<AppEvent>,
    cache_dir: PathBuf,
    /// Keyed by cache path, i.e. by cover rather than album, so a cover
    /// changed by a sync isn't shadowed by the old one.
    memory: Arc<Mutex<LruCache<PathBuf, Arc<DynamicImage>>>>,
    /// Albums waiting on each cover that's being loaded.
    loading: Arc<Mutex<HashMap<PathBuf, Vec<i64>>>>,
}

impl CoverArtService {
    pub fn new(
        database: Database,
        client: OpenSubsonicClient,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<Self> {
        let cache_dir = app_data_dir().join("cache").join("covers");
        std::fs::create_dir_all(&cache_dir).context("failed to create cover cache directory")?;

        Ok(Self {
            database,
            client,
            event_tx,
            cache_dir,
            memory: Arc::new(Mutex::new(LruCache::new(MEMORY_CACHE_CAPACITY))),
            loading: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(super) fn set_client(&mut self, client: OpenSubsonicClient) {
        self.client = client;
    }

    /// The cover always arrives as a `CoverArtEvent::Loaded`, even when it's
    /// already in memory, so callers have a single path to handle. Albums
    /// requesting a cover that's already loading wait on the pending load.
    pub fn request(&self, album_id: i64) {
        // A quick indexed lookup, done inline like the song list query that
        // runs when an album is opened.
        let cover = self
            .database
            .connection()
            .and_then(|connection| CoverArtRepository::new().find_by_album(&connection, album_id));
        let cover = match cover {
            Ok(Some(cover)) => cover,
            Ok(None) => return self.send_loaded(album_id, Ok(None)),
            Err(err) => return self.send_loaded(album_id, Err(format!("{err:#}"))),
        };

        let path = self.cache_path(&cover);
        if let Some(image) = self.memory.lock().unwrap().get(&path) {
            return self.send_loaded(album_id, Ok(Some(image.clone())));
        }

        {
            let mut loading = self.loading.lock().unwrap();
            if let Some(waiting) = loading.get_mut(&path) {
                waiting.push(album_id);
                return;
            }
            loading.insert(path.clone(), vec![album_id]);
        }

        let service = self.clone();
        tokio::spawn(async move {
            let result = service.load(&cover, &path).await.map(Arc::new);
            // Failures aren't remembered, so reopening the album retries.
            if let Ok(image) = &result {
                service
                    .memory
                    .lock()
                    .unwrap()
                    .put(path.clone(), image.clone());
            }
            let waiting = service.loading.lock().unwrap().remove(&path);

            let result = result.map(Some).map_err(|err| format!("{err:#}"));
            for album_id in waiting.unwrap_or_default() {
                service.send_loaded(album_id, result.clone());
            }
        });
    }

    fn send_loaded(&self, album_id: i64, result: Result<Option<Arc<DynamicImage>>, String>) {
        let _ = self
            .event_tx
            .send(AppEvent::CoverArt(CoverArtEvent::Loaded {
                album_id,
                result,
            }));
    }

    async fn load(&self, cover: &AlbumCover, path: &Path) -> Result<DynamicImage> {
        let path = path.to_owned();
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return tokio::task::spawn_blocking(move || {
                image::open(&path).context("failed to decode cached cover art")
            })
            .await
            .context("cover decoding task panicked")?;
        }

        let bytes = self
            .client
            .get_cover_art(&cover.external_id, COVER_SIZE)
            .await
            .context("could not fetch cover art")?;
        tokio::task::spawn_blocking(move || store_cover(&bytes, &path))
            .await
            .context("cover conversion task panicked")?
    }

    /// Covers are keyed by the source's cover id rather than the album, so a
    /// cover shared between albums is stored once and a cover changed on the
    /// server gets a new file instead of a stale hit.
    fn cache_path(&self, cover: &AlbumCover) -> PathBuf {
        self.cache_dir.join(format!(
            "{}-{:x}.webp",
            cover.source_id,
            md5::compute(&cover.external_id)
        ))
    }
}

/// Written under a temporary name and renamed into place, so an interrupted
/// write never leaves a truncated file that would later be read as cached.
/// Loads are deduplicated per cover, so nothing else writes the same file.
fn store_cover(bytes: &[u8], path: &Path) -> Result<DynamicImage> {
    let image = image::load_from_memory(bytes).context("server did not return a readable image")?;
    // Servers are free to ignore the requested size.
    let image = if image.width() > COVER_SIZE || image.height() > COVER_SIZE {
        image.resize(COVER_SIZE, COVER_SIZE, FilterType::Lanczos3)
    } else {
        image
    };

    // The image crate's own WebP encoder is lossless only, which for photos
    // tends to come out larger than the JPEG it started as.
    let encoded = if image.color().has_alpha() {
        let rgba = image.to_rgba8();
        webp::Encoder::from_rgba(&rgba, rgba.width(), rgba.height()).encode(WEBP_QUALITY)
    } else {
        let rgb = image.to_rgb8();
        webp::Encoder::from_rgb(&rgb, rgb.width(), rgb.height()).encode(WEBP_QUALITY)
    };

    let temp_path = path.with_extension("webp.tmp");
    std::fs::write(&temp_path, &*encoded).context("failed to write cover art to cache")?;
    std::fs::rename(&temp_path, path).context("failed to move cover art into cache")?;

    Ok(image)
}
