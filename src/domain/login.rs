use crate::database::client_config::ClientConfigRepository;
use crate::database::core::database::Database;
use crate::domain::AppEvent;
use crate::open_subsonic::{OpenSubsonicClient, OpenSubsonicOptions, PingResponse};
use color_eyre::Result;
use tokio::sync::mpsc::UnboundedSender;

pub enum LoginAction {
    Login(LoginParams),
}

pub struct LoginParams {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub enum LoginEvent {
    LoginResult(Result<LoginResult>),
}

pub struct LoginResult {
    pub response: PingResponse,
    pub options: OpenSubsonicOptions,
}

pub struct LoginService {
    database: Database,
    event_tx: UnboundedSender<AppEvent>,
}

impl LoginService {
    pub fn new(database: Database, event_tx: UnboundedSender<AppEvent>) -> Self {
        Self { database, event_tx }
    }

    pub fn handle_action(&self, action: LoginAction) {
        match action {
            LoginAction::Login(login_params) => self.login(login_params),
        }
    }

    pub fn login(&self, login_params: LoginParams) {
        let tx = self.event_tx.clone();
        let db = self.database.clone();

        tokio::spawn(async move {
            let options = OpenSubsonicOptions::new(
                login_params.url.clone(),
                login_params.username.clone(),
                login_params.password.clone(),
            );

            // Verify the credentials with a throwaway client before applying them.
            let result = OpenSubsonicClient::new(options.clone())
                .ping()
                .await
                .map(|response| LoginResult { response, options });

            let to_send = match result {
                Ok(result) => {
                    let insert_result = ClientConfigRepository::new(db).insert(
                        &login_params.url,
                        &login_params.username,
                        &login_params.password,
                    );

                    match insert_result {
                        Ok(_) => Ok(result),
                        Err(err) => Err(err),
                    }
                }
                Err(err) => Err(err.into()),
            };

            tx.send(AppEvent::Login(LoginEvent::LoginResult(to_send)))
        });
    }
}
