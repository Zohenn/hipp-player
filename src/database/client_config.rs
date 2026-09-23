use crate::database::core::database::Database;
use crate::domain::client_config::ClientConfig;
use aes_gcm::aead::consts::U12;
use aes_gcm::aead::rand_core::Rng;
use aes_gcm::aead::{Aead, Generate};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::{ContextCompat, WrapErr};
use keyring::Entry;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_rusqlite::from_row;

#[derive(Deserialize, Serialize)]
struct StoredClientConfig {
    id: u32,
    url: String,
    username: String,
    password: Vec<u8>,
    nonce: [u8; 12],
    created_at: DateTime<Utc>,
}

pub struct ClientConfigRepository {
    database: Database,
}

impl ClientConfigRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    // TODO: perhaps this should be more of an upsert? we do CHECK (id = 1) in the schema, so this insert might fail if the record already exists
    pub fn insert(&self, url: &str, username: &str, password: &str) -> Result<ClientConfig> {
        let cipher = get_cipher()?;
        let nonce = Nonce::generate();

        let encrypted_password = cipher.encrypt(&nonce, password.as_bytes())?;

        let connection = self.database.connection()?;
        let mut stmt = connection
            .prepare(
                "INSERT INTO client_config (url, username, password, nonce) VALUES (?1, ?2, ?3, ?4) RETURNING *",
            )
            .context("failed to prepare user insert statement")?;

        let stored_config = stmt
            .query_row((url, username, encrypted_password, nonce.0), |row| {
                Ok(from_row::<StoredClientConfig>(row))
            })
            .wrap_err("failed to execute client_config insert query")?
            .wrap_err("failed to deserialize inserted row into ClientConfig")?;

        Ok(convert_stored_client_config(stored_config)?)
    }

    pub fn get(&self) -> Result<Option<ClientConfig>> {
        let connection = self.database.connection()?;

        // Bruh, this is cursed
        connection
            .query_one("SELECT * FROM client_config WHERE id = 1", (), |row| {
                Ok(from_row::<StoredClientConfig>(row))
            })
            .optional()?
            .transpose()?
            .map(convert_stored_client_config)
            .transpose()
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

fn convert_stored_client_config(stored: StoredClientConfig) -> Result<ClientConfig> {
    let cipher = get_cipher()?;
    let decrypted_password =
        cipher.decrypt(&Nonce::from(stored.nonce), stored.password.as_slice())?;

    Ok(ClientConfig {
        id: stored.id,
        url: stored.url,
        username: stored.username,
        password: String::from_utf8(decrypted_password)?,
        nonce: stored.nonce,
        created_at: stored.created_at,
    })
}
