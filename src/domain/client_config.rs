use chrono::{DateTime, Utc};

pub struct ClientConfig {
    pub(crate) id: u32,
    pub url: String,
    pub username: String,
    pub password: String,
    pub(crate) nonce: [u8; 12],
    pub(crate) created_at: DateTime<Utc>,
}
