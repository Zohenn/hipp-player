use crate::types::Seconds;
use chrono::{DateTime, Utc};
use rand::RngExt;
use reqwest::Result;
use reqwest::{IntoUrl, RequestBuilder};
use std::collections::HashMap;

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

#[derive(Clone)]
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

    pub fn options(&self) -> &OpenSubsonicOptions {
        &self.options
    }

    fn build_url(&self, url: &str) -> String {
        format!("{}/rest/{}", self.options.url, url)
    }

    // TODO: the result of this method will never change, no point in creating a new hash map for each request
    fn base_query(&self) -> HashMap<&str, String> {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::rng();
        let password_salt: String = (0..8)
            .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
            .collect();

        let salted_password = [self.options.password.as_bytes(), password_salt.as_bytes()].concat();
        let password_hash = format!("{:x}", md5::compute(salted_password));

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

    pub async fn get_artists(&self) -> Result<Vec<Artist>> {
        let request = self.get(self.build_url("getArtists")).build()?;

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
        let request = self
            .get(self.build_url("getArtist"))
            .query(&[("id", artist_id)])
            .build()?;

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
            .get(self.build_url("getAlbumList2"))
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

    pub async fn get_album_songs(&self, album_id: &str) -> Result<Vec<Song>> {
        let request = self
            .get(self.build_url("getAlbum"))
            .query(&[("id", album_id)])
            .build()?;

        let result = self
            .client
            .execute(request)
            .await?
            .json::<OpenSubsonicResponse<GetAlbumResponse>>()
            .await?;

        Ok(result.subsonic_response.album.song)
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
    pub artist_id: String,
    pub cover_art: String,
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
    song: Vec<Song>,
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
