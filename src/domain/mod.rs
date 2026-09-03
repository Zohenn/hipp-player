use crate::database::core::database::Database;
use crate::domain::login::{LoginEvent, LoginService};
use crate::open_subsonic::{OpenSubsonicClient, OpenSubsonicOptions};
use color_eyre::Result;
use tokio::sync::mpsc::UnboundedSender;

pub mod login;

pub enum AppEvent {
    Login(LoginEvent),
}

pub struct ServiceContainer {
    pub database: Database,
    pub client: OpenSubsonicClient,
    pub login: LoginService,
}

impl ServiceContainer {
    pub fn new(database: Database, event_tx: UnboundedSender<AppEvent>) -> Result<Self> {
        Ok(Self {
            database: database.clone(),
            client: OpenSubsonicClient::new(OpenSubsonicOptions::default()),
            login: LoginService::new(database, event_tx),
        })
    }
}
