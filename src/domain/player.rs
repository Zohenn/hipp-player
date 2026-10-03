use crate::data::log_error;
use crate::database::core::database::Database;
use crate::database::song::SongRepository;
use crate::domain::AppEvent;
use crate::domain::album::Album;
use crate::domain::open_subsonic_streaming::OpenSubsonicStreamingService;
use crate::domain::song::Song;
use crate::domain::source::SourceKind;
use crate::open_subsonic::OpenSubsonicClient;
use color_eyre::Result;
use color_eyre::eyre::eyre;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct PlayerService {
    database: Database,
    open_subsonic_streaming_service: OpenSubsonicStreamingService,
    event_tx: UnboundedSender<AppEvent>,
    sink: Arc<Mutex<Option<(MixerDeviceSink, Player)>>>,
    current_request: u64,
    // Request whose file is currently in the sink; differs from
    // `current_request` while the next song is still being fetched.
    loaded_request: u64,
    current_song: Option<PlayingSongDetails>,
}

pub enum PlayerEvent {
    FileReady {
        request_id: u64,
        file: std::fs::File,
    },
    StreamFailed {
        request_id: u64,
        error: String,
    },
    Error(String),
}

#[derive(Clone)]
pub struct PlayingSongDetails {
    pub song_id: i64,
    pub song_title: String,
    pub song_duration: i64,
    pub artist_name: String,
    pub album_id: i64,
    pub album_name: String,
}

impl PlayingSongDetails {
    pub fn new(song: &Song, artist_name: String, album: &Album) -> Self {
        Self {
            song_id: song.id,
            song_title: song.title.clone(),
            song_duration: song.duration_seconds,
            artist_name,
            album_id: album.id,
            album_name: album.name.clone(),
        }
    }
}

pub enum PlaybackState {
    Loading,
    Playing(Duration),
    Paused(Duration),
    Stopped,
}

pub struct PlayerState<'a> {
    pub details: &'a PlayingSongDetails,
    pub playback_state: PlaybackState,
}

impl PlayerService {
    pub fn new(
        database: Database,
        open_subsonic_streaming_service: OpenSubsonicStreamingService,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Self {
        Self {
            database,
            open_subsonic_streaming_service,
            event_tx,
            sink: Arc::new(Mutex::new(None)),
            current_request: 0,
            loaded_request: 0,
            current_song: None,
        }
    }

    pub fn set_client(&mut self, client: OpenSubsonicClient) {
        // TODO: this is dumb
        self.open_subsonic_streaming_service.set_client(client);
    }

    pub fn play(&mut self, details: PlayingSongDetails) -> Result<()> {
        let links =
            SongRepository::new().find_links(&*self.database.connection()?, details.song_id)?;
        let preferred_link = links.get(0).ok_or(eyre!(
            "Song does not exist or has no links: {}",
            details.song_id
        ))?;

        self.current_song = Some(details);

        match preferred_link.source_kind {
            SourceKind::OpenSubsonic => self.play_from_opensubsonic(&preferred_link.external_id),
        }

        Ok(())
    }

    pub fn handle_event(&mut self, event: PlayerEvent) -> Result<()> {
        match event {
            PlayerEvent::FileReady { request_id, file } => {
                if request_id != self.current_request {
                    return Ok(());
                }

                let result = self.play_from_file(file, request_id);
                if result.is_err() {
                    self.current_song = None;
                }
                result
            }
            PlayerEvent::StreamFailed { request_id, .. } => {
                if request_id == self.current_request {
                    self.current_song = None;
                }
                Ok(())
            }
            PlayerEvent::Error(_) => Ok(()),
        }
    }

    fn play_from_opensubsonic(&mut self, subsonic_id: &str) {
        self.current_request += 1;
        let request_id = self.current_request;

        let mut streaming_service = self.open_subsonic_streaming_service.clone();
        let event_tx = self.event_tx.clone();
        let subsonic_id = subsonic_id.to_owned();

        tokio::spawn(async move {
            let send_result = match streaming_service.stream_song(&subsonic_id).await {
                Ok(file) => event_tx.send(AppEvent::Player(PlayerEvent::FileReady {
                    request_id,
                    file,
                })),
                Err(err) => event_tx.send(AppEvent::Player(PlayerEvent::StreamFailed {
                    request_id,
                    error: format!("{:#}", err),
                })),
            };

            if let Err(err) = send_result {
                log_error(err);
            }
        });
    }

    fn play_from_file(&mut self, file: std::fs::File, request_id: u64) -> Result<()> {
        let mut guard = self.sink.lock().unwrap();

        if guard.is_none() {
            let new_sink = DeviceSinkBuilder::open_default_sink()?;
            let player = Player::connect_new(&new_sink.mixer());
            *guard = Some((new_sink, player));
        }

        let (_, player) = guard.as_ref().unwrap();
        player.clear();
        player.append(rodio::decoder::Decoder::new(BufReader::new(file))?);

        player.set_volume(0.2);
        player.play();
        self.loaded_request = request_id;

        Ok(())
    }

    pub fn snapshot(&self) -> Option<PlayerState<'_>> {
        let details = self.current_song.as_ref()?;

        let playback_state = if self.loaded_request != self.current_request {
            PlaybackState::Loading
        } else {
            match self.sink.lock().unwrap().as_ref() {
                Some((_, player)) if player.empty() => PlaybackState::Stopped,
                Some((_, player)) if player.is_paused() => PlaybackState::Paused(player.get_pos()),
                Some((_, player)) => PlaybackState::Playing(player.get_pos()),
                None => PlaybackState::Loading,
            }
        };

        Some(PlayerState {
            details,
            playback_state,
        })
    }
}
