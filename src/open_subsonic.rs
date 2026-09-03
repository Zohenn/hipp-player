use rand::Rng;
use reqwest::Result;
use reqwest::{IntoUrl, RequestBuilder};
use std::collections::HashMap;

#[derive(Default)]
pub struct OpenSubsonicOptions {
    pub url: String,
    pub username: String,
    pub password: String,
    pub api_version: String,
}

impl OpenSubsonicOptions {
    pub fn new(url: String, username: String, password: String) -> Self {
        Self {
            url,
            username,
            password,
            api_version: String::from("1.16.1"),
        }
    }
}

pub struct OpenSubsonicClient {
    options: OpenSubsonicOptions,
    client: reqwest::Client,
}

impl OpenSubsonicClient {
    pub fn new(options: OpenSubsonicOptions) -> Self {
        Self {
            options,
            client: Default::default(),
        }
    }

    fn build_url(&self, url: &str) -> String {
        format!("{}/rest/{}", self.options.url, url)
    }

    // TODO: the result of this method will never change, no point in creating a new hash map for each request
    fn base_query(&self) -> HashMap<&str, String> {
        let mut salt = [0u8; 8];
        rand::rng().fill_bytes(&mut salt);
        let salted_password = [self.options.password.as_bytes(), &salt].concat();

        let password_hash_bytes = md5::compute(salted_password).0;

        let password_hash = String::from_utf8_lossy(&password_hash_bytes).to_string();
        let password_salt = String::from_utf8_lossy(&salt).to_string();

        HashMap::from([
            ("u", self.options.username.clone()),
            ("t", password_hash),
            ("s", password_salt),
            ("v", self.options.api_version.clone()),
            ("c", "Hipp Player".to_string()),
            ("f", "json".to_string()),
        ])
    }

    fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.client.get(url).query(&self.base_query())
    }

    pub async fn ping(&self) -> Result<PingResponse> {
        let request = self.get(self.build_url("ping")).build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<OpenSubsonicResponse<PingResponse>>()
            .await?;

        Ok(result.subsonic_response)
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct OpenSubsonicResponse<T> {
    subsonic_response: T,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    version: String,
    r#type: String,
    server_version: String,
}
