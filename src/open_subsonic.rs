use crate::types::Seconds;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use rand::RngExt;
use reqwest::RequestBuilder;
use reqwest::Result;
use std::sync::{Arc, RwLock};
use tokio_stream::Stream;

#[derive(Default, Clone)]
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

const CLIENT_NAME: &str = "Hipp Player";

#[derive(Clone)]
pub struct OpenSubsonicClient {
    options: Arc<RwLock<OpenSubsonicOptions>>,
    client: reqwest::Client,
}

impl OpenSubsonicClient {
    pub fn new(options: OpenSubsonicOptions) -> Self {
        Self {
            options: Arc::new(RwLock::new(options)),
            client: Default::default(),
        }
    }

    pub fn options(&self) -> OpenSubsonicOptions {
        self.options.read().unwrap().clone()
    }

    pub fn set_options(&self, options: OpenSubsonicOptions) {
        *self.options.write().unwrap() = options;
    }

    fn get(&self, path: &str) -> RequestBuilder {
        let options = self.options.read().unwrap();
        let (salt, token) = auth_token(&options.password);
        self.client
            .get(format!("{}/rest/{}", options.url, path))
            .query(&[
                ("u", options.username.as_str()),
                ("t", &token),
                ("s", &salt),
                ("v", &options.api_version),
                ("c", CLIENT_NAME),
                ("f", "json"),
            ])
    }

    pub async fn ping(&self) -> Result<PingResponse> {
        let request = self.get("ping").build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<OpenSubsonicResponse<PingResponse>>()
            .await?;

        Ok(result.subsonic_response)
    }

    pub async fn get_artists(&self) -> Result<Vec<Artist>> {
        let request = self.get("getArtists").build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<OpenSubsonicResponse<GetArtistsResponse>>()
            .await?;

        Ok(result
            .subsonic_response
            .artists
            .index
            .into_iter()
            .map(|i| i.artist)
            .flatten()
            .collect())
    }

    pub async fn get_artist_albums(&self, artist_id: &str) -> Result<Vec<Album>> {
        let request = self.get("getArtist").query(&[("id", artist_id)]).build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<OpenSubsonicResponse<GetArtistResponse>>()
            .await?;

        Ok(result.subsonic_response.artist.album)
    }

    pub async fn get_newest_albums(&self, size: u32, offset: u32) -> Result<Vec<Album>> {
        let request = self
            .get("getAlbumList2")
            .query(&[
                ("type", "newest".to_string()),
                ("size", size.to_string()),
                ("offset", offset.to_string()),
            ])
            .build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<OpenSubsonicResponse<GetAlbumList2Response>>()
            .await?;

        Ok(result.subsonic_response.album_list2.album)
    }

    pub async fn get_album(&self, album_id: &str) -> Result<AlbumWithSongs> {
        let request = self.get("getAlbum").query(&[("id", album_id)]).build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<OpenSubsonicResponse<GetAlbumResponse>>()
            .await?;

        Ok(result.subsonic_response.album)
    }

    /// Returns the raw image bytes, in whatever format the server stores.
    /// `size` asks the server to scale it down, which not every server does.
    pub async fn get_cover_art(&self, cover_art_id: &str, size: u32) -> Result<Vec<u8>> {
        let request = self
            .get("getCoverArt")
            .query(&[("id", cover_art_id.to_string()), ("size", size.to_string())])
            .build()?;

        self.client
            .execute(request)
            .await?
            .error_for_status()?
            .bytes()
            .await
            .map(Vec::from)
    }

    pub async fn stream_song(
        &self,
        song_id: &str,
    ) -> Result<impl Stream<Item = Result<Bytes>> + use<>> {
        let request = self
            .get("stream")
            .query(&[("id", song_id.to_string())])
            .build()?;

        Ok(self.client.execute(request).await?.bytes_stream())
    }
}

/// Returns `(salt, token)`; the salt should be fresh for every request.
fn auth_token(password: &str) -> (String, String) {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    let salt: String = (0..8)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect();

    let salted_password = [password.as_bytes(), salt.as_bytes()].concat();
    let token = format!("{:x}", md5::compute(salted_password));

    (salt, token)
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArtistsResponse {
    artists: ArtistsIndex,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistsIndex {
    index: Vec<ArtistIndexEntry>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtistIndexEntry {
    artist: Vec<Artist>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub album_count: u32,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArtistResponse {
    artist: GetArtistArtist,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArtistArtist {
    album: Vec<Album>,
}

#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub artist: String,
    // Optional in the OpenSubsonic spec; missing for albums without art.
    #[serde(default)]
    pub cover_art: Option<String>,
    pub duration: Seconds,
    #[serde(default, deserialize_with = "lenient_datetime")]
    pub created: Option<DateTime<Utc>>,
}

/// A malformed date shouldn't fail the whole response it's part of.
fn lenient_datetime<'de, D>(deserializer: D) -> std::result::Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(value.and_then(|value| {
        DateTime::parse_from_rfc3339(&value)
            .or_else(|_| DateTime::parse_from_rfc2822(&value))
            .ok()
            .map(|dt| dt.to_utc())
    }))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAlbumList2Response {
    album_list2: AlbumList2,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumList2 {
    // Omitted by servers when the page is empty.
    #[serde(default)]
    album: Vec<Album>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAlbumResponse {
    album: AlbumWithSongs,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumWithSongs {
    // List endpoints don't reliably report it, so artist attribution comes from here.
    pub artist_id: String,
    pub song: Vec<Song>,
}

#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub title: String,
    pub track: u16,
    // Optional in the OpenSubsonic spec; servers omit it for single-disc albums.
    #[serde(default)]
    pub disc_number: Option<u16>,
    pub year: u16,
    pub duration: Seconds,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[derive(serde::Deserialize)]
    struct Dated {
        #[serde(default, deserialize_with = "lenient_datetime")]
        created: Option<DateTime<Utc>>,
    }

    fn parse(json: &str) -> Option<DateTime<Utc>> {
        serde_json::from_str::<Dated>(json).unwrap().created
    }

    #[test]
    fn lenient_datetime_accepts_rfc3339() {
        assert_eq!(
            parse(r#"{"created": "2026-07-24T15:45:51Z"}"#),
            Some(Utc.with_ymd_and_hms(2026, 7, 24, 15, 45, 51).unwrap())
        );
    }

    #[test]
    fn lenient_datetime_accepts_rfc2822() {
        assert_eq!(
            parse(r#"{"created": "24 Jul 2026 15:45:51 GMT"}"#),
            Some(Utc.with_ymd_and_hms(2026, 7, 24, 15, 45, 51).unwrap())
        );
    }

    #[test]
    fn lenient_datetime_ignores_garbage() {
        assert_eq!(parse(r#"{"created": "yesterday"}"#), None);
        assert_eq!(parse(r#"{}"#), None);
    }
}
