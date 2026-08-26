use crate::database::core::database::Database;
use aes_gcm::aead::rand_core::Rng;
use aes_gcm::aead::{Aead, Generate};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use serde_rusqlite::from_row;

#[derive(Deserialize, Serialize)]
pub struct ClientConfig {
    id: u32,
    url: String,
    username: String,
    password: String,
    nonce: String,
    created_at: DateTime<Utc>,
}

pub struct UserRepository {
    database: Database,
}

impl UserRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn insert(&self, url: &str, username: &str, password: &str) -> Result<ClientConfig> {
        let cipher = get_cipher()?;
        let nonce = Nonce::generate();

        let encrypted_password = cipher.encrypt(&nonce, password.as_bytes())?;

        let connection = self.database.connection();
        let mut stmt = connection
            .prepare(
                "INSERT INTO users (url, username, password, nonce) VALUES (?1, ?2, ?3, ?4) RETURNING *",
            )
            .wrap_err("failed to prepare user insert statement")?;

        let user = stmt
            .query_row((url, username, encrypted_password, nonce.0), |row| {
                Ok(from_row::<ClientConfig>(row))
            })
            .wrap_err("failed to execute user insert query")?
            .wrap_err("failed to deserialize inserted row into User")?;

        Ok(user)
    }
}

fn get_or_create_encryption_key() -> Result<[u8; 32]> {
    let entry = Entry::new("hipp-player", "encryption_key")?;

    match entry.get_password() {
        Ok(hex_key) => {
            let mut key = [0u8; 32];
            key.copy_from_slice(&hex::decode(hex_key)?);

            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            rand::rng().fill_bytes(key.as_mut_slice());

            entry.set_password(&hex::encode(key))?;

            Ok(key)
        }
        Err(err) => Err(err.into()),
    }
}

fn get_cipher() -> Result<Aes256Gcm> {
    let key = Key::<Aes256Gcm>::try_from(get_or_create_encryption_key()?)?;

    Ok(Aes256Gcm::new(&key))
}
