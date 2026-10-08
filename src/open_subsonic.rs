use crate::data::log_error;
use crate::types::Seconds;
use bytes::Bytes;
use chrono::{DateTime, Local, Utc};
use color_eyre::eyre::{self, eyre};
use rand::RngExt;
use reqwest::Result;
use reqwest::{Request, RequestBuilder};
use serde::de::DeserializeOwned;
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
const BODY_LOG_LIMIT: usize = 2000;

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

    pub async fn ping(&self) -> eyre::Result<PingResponse> {
        let request = self.get("ping").build()?;

        self.execute_json(request).await
    }

    pub async fn get_artists(&self) -> eyre::Result<Vec<Artist>> {
        let request = self.get("getArtists").build()?;

        let result: GetArtistsResponse = self.execute_json(request).await?;

        Ok(result
            .artists
            .index
            .into_iter()
            .map(|i| i.artist)
            .flatten()
            .collect())
    }

    pub async fn get_artist_albums(&self, artist_id: &str) -> eyre::Result<Vec<Album>> {
        let request = self.get("getArtist").query(&[("id", artist_id)]).build()?;

        let result: GetArtistResponse = self.execute_json(request).await?;

        Ok(result.artist.album)
    }

    pub async fn get_newest_albums(&self, size: u32, offset: u32) -> eyre::Result<Vec<Album>> {
        let request = self
            .get("getAlbumList2")
            .query(&[
                ("type", "newest".to_string()),
                ("size", size.to_string()),
                ("offset", offset.to_string()),
            ])
            .build()?;

        let result: GetAlbumList2Response = self.execute_json(request).await?;

        Ok(result.album_list2.album)
    }

    pub async fn get_album(&self, album_id: &str) -> eyre::Result<AlbumWithSongs> {
        let request = self.get("getAlbum").query(&[("id", album_id)]).build()?;

        let result: GetAlbumResponse = self.execute_json(request).await?;

        Ok(result.album)
    }

    /// Runs a request and parses its Subsonic JSON envelope. On failure the
    /// short error is returned and the full response is written to error.log.
    async fn execute_json<T: DeserializeOwned>(&self, request: Request) -> eyre::Result<T> {
        let url = redacted_url(request.url());
        let endpoint = request
            .url()
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .unwrap_or_default()
            .to_owned();
        let response =
            self.client.execute(request).await.map_err(|err| {
                eyre!("{endpoint}: request to {url} failed: {}", err.without_url())
            })?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let body = response
            .text()
            .await
            .map_err(|err| eyre!("{endpoint}: reading response failed: {}", err.without_url()))?;

        let error = match parse_response(&content_type, &body) {
            Ok(result) => return Ok(result),
            Err(ParseFailure::ServerError(message)) => return Err(eyre!("{endpoint}: {message}")),
            Err(ParseFailure::Unexpected(message)) => format!("{endpoint}: {message}"),
        };

        log_error(format!(
            "[{}] {error}\n  url: {url}\n  status: {status}\n  content-type: {content_type}\n  body (first {BODY_LOG_LIMIT} bytes):\n{}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            truncate(&body, BODY_LOG_LIMIT),
        ));

        Err(eyre!("{error} (details in error.log)"))
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

/// The URL without the auth params, safe to write to a log.
fn redacted_url(url: &reqwest::Url) -> String {
    let mut url = url.clone();
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !matches!(key.as_ref(), "u" | "t" | "s"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut().clear().extend_pairs(pairs);
    url.to_string()
}

enum ParseFailure {
    /// The server answered properly, with a Subsonic error.
    ServerError(String),
    /// The response isn't something we understand.
    Unexpected(String),
}

fn parse_response<T: DeserializeOwned>(
    content_type: &str,
    body: &str,
) -> std::result::Result<T, ParseFailure> {
    let envelope = match serde_json::from_str::<OpenSubsonicResponse<StatusEnvelope>>(body) {
        Ok(envelope) => envelope.subsonic_response,
        Err(err) => {
            return Err(ParseFailure::Unexpected(
                match non_json_kind(content_type, body) {
                    Some(kind) => format!("server returned {kind} instead of JSON"),
                    None => format!("unexpected response: {err}"),
                },
            ));
        }
    };

    if envelope.status == "failed" {
        return Err(ParseFailure::ServerError(match envelope.error {
            Some(error) => format!("server error {}: {}", error.code, error.message),
            None => "server reported failure without details".to_owned(),
        }));
    }

    serde_json::from_str::<OpenSubsonicResponse<T>>(body)
        .map(|response| response.subsonic_response)
        .map_err(|err| ParseFailure::Unexpected(format!("unexpected response: {err}")))
}

fn non_json_kind(content_type: &str, body: &str) -> Option<&'static str> {
    let body = body.trim_start();
    if content_type.contains("html") || body.starts_with("<!") || body.starts_with("<html") {
        Some("HTML")
    } else if content_type.contains("xml") || body.starts_with('<') {
        Some("XML")
    } else if body.is_empty() {
        Some("an empty body")
    } else {
        None
    }
}

fn truncate(text: &str, max_bytes: usize) -> &str {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[derive(serde::Deserialize)]
struct StatusEnvelope {
    status: String,
    error: Option<SubsonicError>,
}

#[derive(serde::Deserialize)]
struct SubsonicError {
    code: i64,
    #[serde(default)]
    message: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct OpenSubsonicResponse<T> {
    subsonic_response: T,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
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
