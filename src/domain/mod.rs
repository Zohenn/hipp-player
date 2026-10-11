use crate::database::core::database::Database;
use crate::domain::collection::CollectionService;
use crate::domain::cover_art::{CoverArtEvent, CoverArtService};
use crate::domain::login::{LoginEvent, LoginService};
use crate::domain::open_subsonic_streaming::OpenSubsonicStreamingService;
use crate::domain::playback::{PlaybackCommand, PlaybackService};
use crate::domain::player::{PlayerEvent, PlayerService};
use crate::domain::queue::QueueService;
use crate::open_subsonic::{OpenSubsonicClient, OpenSubsonicOptions};
use color_eyre::Result;
use tokio::sync::mpsc::UnboundedSender;

pub mod album;
pub mod artist;
pub mod client_config;
pub mod collection;
pub mod cover_art;
pub mod login;
pub mod notification;
pub mod open_subsonic_streaming;
pub mod playback;
pub mod player;
pub mod queue;
pub mod song;
pub mod source;
pub mod sync_run;

pub enum AppEvent {
    Login(LoginEvent),
    CoverArt(CoverArtEvent),
    Player(PlayerEvent),
    PlaybackCommand(PlaybackCommand),
}

pub struct ServiceContainer {
    pub database: Database,
    pub client: OpenSubsonicClient,
    pub login: LoginService,
    pub collection: CollectionService,
    pub cover_art: CoverArtService,
    pub playback: PlaybackService,
}

impl ServiceContainer {
    pub fn new(database: Database, event_tx: UnboundedSender<AppEvent>) -> Result<Self> {
        let client = OpenSubsonicClient::new(OpenSubsonicOptions::default());
        let cover_art = CoverArtService::new(database.clone(), client.clone(), event_tx.clone())?;
        Ok(Self {
            database: database.clone(),
            client: client.clone(),
            login: LoginService::new(database.clone(), event_tx.clone()),
            collection: CollectionService::new(database.clone(), client.clone()),
            cover_art: cover_art.clone(),
            playback: PlaybackService::new(
                PlayerService::new(
                    database.clone(),
                    OpenSubsonicStreamingService::new(client)?,
                    event_tx.clone(),
                ),
                QueueService::new(database),
                event_tx,
                cover_art,
            ),
        })
    }
}
