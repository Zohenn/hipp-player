use crate::database::core::database::Database;
use crate::database::user::UserRepository;
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
    pub client: OpenSubsonicClient,
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
            let client = OpenSubsonicClient::new(OpenSubsonicOptions {
                url: login_params.url.clone(),
                username: login_params.username.clone(),
                password: login_params.password.clone(),
                api_version: "1.16.1".to_owned(),
            });

            let result = client
                .ping()
                .await
                .map(|response| LoginResult { response, client });

            let to_send = match result {
                Ok(result) => {
                    let insert_result = UserRepository::new(db).insert(
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
