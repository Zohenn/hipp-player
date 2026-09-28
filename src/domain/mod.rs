use crate::database::core::database::Database;
use crate::domain::collection::CollectionService;
use crate::domain::cover_art::{CoverArtEvent, CoverArtService};
use crate::domain::login::{LoginEvent, LoginService};
use crate::open_subsonic::{OpenSubsonicClient, OpenSubsonicOptions};
use color_eyre::Result;
use tokio::sync::mpsc::UnboundedSender;

pub mod album;
pub mod artist;
pub mod client_config;
pub mod collection;
pub mod cover_art;
pub mod login;
pub mod song;
pub mod source;
pub mod sync_run;

pub enum AppEvent {
    Login(LoginEvent),
    CoverArt(CoverArtEvent),
}

pub struct ServiceContainer {
    pub database: Database,
    pub client: OpenSubsonicClient,
    pub login: LoginService,
    pub collection: CollectionService,
    pub cover_art: CoverArtService,
}

impl ServiceContainer {
    pub fn new(database: Database, event_tx: UnboundedSender<AppEvent>) -> Result<Self> {
        let client = OpenSubsonicClient::new(OpenSubsonicOptions::default());
        Ok(Self {
            database: database.clone(),
            client: client.clone(),
            login: LoginService::new(database.clone(), event_tx.clone()),
            collection: CollectionService::new(database.clone(), client.clone()),
            cover_art: CoverArtService::new(database, client, event_tx)?,
        })
    }

    pub fn set_client(&mut self, client: OpenSubsonicClient) {
        self.client = client.clone();
        self.collection.set_client(client.clone());
        self.cover_art.set_client(client);
    }
}
