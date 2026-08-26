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
    pub client: OpenSubsonicClient,
    pub login: LoginService,
}

impl ServiceContainer {
    pub fn new(event_tx: UnboundedSender<AppEvent>) -> Result<Self> {
        let database = Database::new()?;

        Ok(Self {
            client: OpenSubsonicClient::new(OpenSubsonicOptions::default()),
            login: LoginService::new(database, event_tx),
        })
    }
}
