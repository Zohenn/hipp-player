use crate::data::log_error;
use crate::database::core::database::Database;
use crate::database::song::SongRepository;
use crate::domain::AppEvent;
use crate::domain::open_subsonic_streaming::OpenSubsonicStreamingService;
use crate::domain::source::SourceKind;
use crate::open_subsonic::OpenSubsonicClient;
use color_eyre::Result;
use color_eyre::eyre::eyre;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct PlayerService {
    database: Database,
    open_subsonic_streaming_service: OpenSubsonicStreamingService,
    event_tx: UnboundedSender<AppEvent>,
    sink: Arc<Mutex<Option<(MixerDeviceSink, Player)>>>,
    current_request: u64,
}

pub enum PlayerEvent {
    Ready {
        request_id: u64,
        file: std::fs::File,
    },
    Error(String),
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
        }
    }

    pub fn set_client(&mut self, client: OpenSubsonicClient) {
        // TODO: this is dumb
        self.open_subsonic_streaming_service.set_client(client);
    }

    pub fn play(&mut self, song_id: i64) -> Result<()> {
        let links = SongRepository::new().find_links(&*self.database.connection()?, song_id)?;
        let preferred_link = links
            .get(0)
            .ok_or(eyre!("Song does not exist or has no links: {}", song_id))?;

        match preferred_link.source_kind {
            SourceKind::OpenSubsonic => self.play_from_opensubsonic(&preferred_link.external_id),
        }

        Ok(())
    }

    pub fn handle_event(&mut self, event: PlayerEvent) -> Result<()> {
        match event {
            PlayerEvent::Ready { request_id, file } => {
                if request_id != self.current_request {
                    return Ok(());
                }

                self.play_from_file(file)
            }
            PlayerEvent::Error(_) => {
                // we don't care about error events here
                Ok(())
            }
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
                Ok(file) => {
                    event_tx.send(AppEvent::Player(PlayerEvent::Ready { request_id, file }))
                }
                Err(err) => {
                    event_tx.send(AppEvent::Player(PlayerEvent::Error(format!("{:#}", err))))
                }
            };

            if let Err(err) = send_result {
                log_error(err);
            }
        });
    }

    fn play_from_file(&mut self, file: std::fs::File) -> Result<()> {
        let mut guard = self.sink.lock().unwrap();

        if guard.is_none() {
            let new_sink = DeviceSinkBuilder::open_default_sink()?;
            let player = Player::connect_new(&new_sink.mixer());
            *guard = Some((new_sink, player));
        }

        let (_, player) = guard.as_ref().unwrap();
        player.clear();
        player.append(rodio::decoder::Decoder::new(BufReader::new(file))?);

        player.set_volume(0.5);
        player.play();

        Ok(())
    }
}
