use crate::domain::AppEvent;
use crate::open_subsonic::{OpenSubsonicClient, OpenSubsonicOptions, PingResponse};
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
    LoginResult(Result<LoginResult, reqwest::Error>),
}

pub struct LoginResult {
    response: PingResponse,
    client: OpenSubsonicClient,
}

pub struct LoginService {
    event_tx: UnboundedSender<AppEvent>,
}

impl LoginService {
    pub fn new(event_tx: UnboundedSender<AppEvent>) -> Self {
        Self { event_tx }
    }

    pub fn handle_action(&self, action: LoginAction) {
        match action {
            LoginAction::Login(login_params) => self.login(login_params),
        }
    }

    pub fn login(&self, login_params: LoginParams) {
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let client = OpenSubsonicClient::new(OpenSubsonicOptions {
                url: login_params.url,
                username: login_params.username,
                password: login_params.password,
                api_version: "1.16.1".to_owned(),
            });

            let result = client
                .ping()
                .await
                .map(|response| LoginResult { response, client });
            tx.send(AppEvent::Login(LoginEvent::LoginResult(result)))
        });
    }
}
