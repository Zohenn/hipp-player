use crate::domain::login::{LoginEvent, LoginService};
use tokio::sync::mpsc::UnboundedSender;

pub mod login;

pub enum AppEvent {
    Login(LoginEvent),
}

pub struct ServiceContainer {
    pub login: LoginService,
}

impl ServiceContainer {
    pub fn new(event_tx: UnboundedSender<AppEvent>) -> Self {
        Self {
            login: LoginService::new(event_tx),
        }
    }
}
