use crate::domain::AppEvent;
use crate::domain::cover_art::CoverArtService;
use crate::domain::notification::NotificationService;
use crate::domain::player::{PlaybackState, PlayerService, PlayingSongDetails};
use crate::domain::queue::QueueService;
use color_eyre::Result;
use mpris_server::{Metadata, PlaybackStatus, Time, TrackId};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub struct PlaybackService {
    notification: NotificationService,
    cover_art: CoverArtService,
    published: Published,
    pending_art_album: Option<i64>,
}

pub enum PlaybackEvent {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
}

/// What the notification service was last told
#[derive(Default, PartialEq)]
struct Published {
    song_id: Option<i64>,
    status: Option<PlaybackStatus>,
    can_go_next: bool,
    can_go_previous: bool,
}

impl PlaybackService {
    pub fn new(event_tx: UnboundedSender<AppEvent>, cover_art: CoverArtService) -> Self {
        Self {
            notification: NotificationService::new(event_tx),
            cover_art,
            published: Published::default(),
            pending_art_album: None,
        }
    }

    pub fn handle_event(
        &mut self,
        event: PlaybackEvent,
        player: &mut PlayerService,
        queue: &mut QueueService,
    ) -> Result<()> {
        match event {
            PlaybackEvent::PlayPause => player.toggle_playback(),
            PlaybackEvent::Play => {
                if !is_playing(player) {
                    player.toggle_playback()?;
                }
                Ok(())
            }
            PlaybackEvent::Pause => {
                if is_playing(player) {
                    player.toggle_playback()?;
                }
                Ok(())
            }
            PlaybackEvent::Next => match queue.next()? {
                Some(details) => player.play(details),
                None => Ok(()),
            },
            PlaybackEvent::Previous => match queue.previous()? {
                Some(details) => player.play(details),
                None => Ok(()),
            },
        }
    }

    /// Pushes any change in playback state to the notification service.
    /// Cheap enough to call every frame.
    pub fn sync(&mut self, player: &PlayerService, queue: &QueueService) {
        let state = player.snapshot();
        let current = queue.current().map(|(index, _)| index);
        let current = Published {
            song_id: state.as_ref().map(|state| state.details.song_id),
            status: state.as_ref().map(|state| match state.playback_state {
                // The user asked for playback, it just hasn't started yet.
                PlaybackState::Loading | PlaybackState::Playing(_) => PlaybackStatus::Playing,
                PlaybackState::Paused(_) => PlaybackStatus::Paused,
                PlaybackState::Stopped => PlaybackStatus::Stopped,
            }),
            can_go_next: match current {
                Some(index) => index + 1 < queue.entries().len(),
                None => !queue.entries().is_empty(),
            },
            can_go_previous: current.is_some_and(|index| index > 0),
        };
        if current == self.published {
            return;
        }

        if current.song_id != self.published.song_id {
            match &state {
                Some(state) => self.publish_song(state.details),
                None => {
                    self.pending_art_album = None;
                    self.notification.set_metadata(Metadata::new());
                }
            }
        }
        if let Some(state) = &state {
            let position = match state.playback_state {
                PlaybackState::Playing(position) | PlaybackState::Paused(position) => position,
                PlaybackState::Loading | PlaybackState::Stopped => Duration::ZERO,
            };
            self.notification
                .set_position(Time::from_micros(position.as_micros() as i64));
        }
        if current.status != self.published.status {
            self.notification
                .set_playback_status(current.status.unwrap_or(PlaybackStatus::Stopped));
        }
        if current.can_go_next != self.published.can_go_next {
            self.notification.set_can_go_next(current.can_go_next);
        }
        if current.can_go_previous != self.published.can_go_previous {
            self.notification
                .set_can_go_previous(current.can_go_previous);
        }

        self.published = current;
    }

    /// Adds the cover to the current song's metadata once it's been fetched.
    pub fn on_cover_loaded(&mut self, album_id: i64, player: &PlayerService) {
        if self.pending_art_album != Some(album_id) {
            return;
        }
        self.pending_art_album = None;

        if let Some(state) = player.snapshot()
            && state.details.album_id == album_id
            && let Some(path) = self.cover_art.cached_path(album_id)
        {
            self.notification
                .set_metadata(metadata(state.details, Some(&path)));
        }
    }

    fn publish_song(&mut self, details: &PlayingSongDetails) {
        let art = self.cover_art.cached_path(details.album_id);
        // Not cached yet, e.g. the song was never opened in the UI.
        self.pending_art_album = if art.is_none() {
            self.cover_art.request(details.album_id);
            Some(details.album_id)
        } else {
            None
        };
        self.notification
            .set_metadata(metadata(details, art.as_deref()));
    }
}

fn is_playing(player: &PlayerService) -> bool {
    matches!(
        player.snapshot().map(|state| state.playback_state),
        Some(PlaybackState::Playing(_) | PlaybackState::Loading)
    )
}

fn metadata(details: &PlayingSongDetails, art: Option<&Path>) -> Metadata {
    let mut metadata = Metadata::builder()
        .title(details.song_title.clone())
        .artist([details.artist_name.clone()])
        .album(details.album_name.clone())
        .length(Time::from_secs(details.song_duration))
        .build();
    if let Ok(track_id) = TrackId::try_from(format!("/org/hipp/track/{}", details.song_id).as_str())
    {
        metadata.set_trackid(Some(track_id));
    }
    metadata.set_art_url(art.map(file_uri));

    metadata
}

fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }

    uri
}
