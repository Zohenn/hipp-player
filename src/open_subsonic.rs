use color_eyre::Result;
use reqwest::RequestBuilder;
use std::collections::HashMap;

pub struct OpenSubsonicOptions {
    url: String,
    username: String,
    password_hash: String,
    password_salt: String,
    api_version: String,
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
        format!("{}/{}", self.options.url, url)
    }

    fn base_query(&self) -> HashMap<&str, String> {
        HashMap::from([
            ("u", self.options.username.clone()),
            ("t", self.options.password_hash.clone()),
            ("s", self.options.password_salt.clone()),
            ("v", self.options.api_version.clone()),
            ("c", "Hipp Player".to_string()),
            ("f", "json".to_string()),
        ])
    }

    fn get(&self) -> RequestBuilder {
        self.client
            .get(self.build_url("ping"))
            .query(&self.base_query())
    }

    pub async fn ping(&self) -> Result<HashMap<String, String>> {
        let request = self.get().build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<HashMap<String, String>>()
            .await?;

        Ok(result)
    }
}
