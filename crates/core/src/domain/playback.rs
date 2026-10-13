use crate::domain::CoreEvent;
use crate::domain::cover_art::CoverArtService;
use crate::domain::media_controls::{MediaControlsService, MediaStatus, TrackMetadata};
use crate::domain::player::{PlaybackState, PlayerEvent, PlayerService, PlayingSongDetails};
use crate::domain::queue::QueueService;
use color_eyre::Result;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub struct PlaybackService {
    player: PlayerService,
    queue: QueueService,
    media_controls: MediaControlsService,
    cover_art: CoverArtService,
    published: Published,
    pending_art_album: Option<i64>,
}

pub enum PlaybackCommand {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    Select(usize),
    Clear,
    Enqueue(PlayingSongDetails),
    Replace {
        entries: Vec<PlayingSongDetails>,
        start: usize,
    },
}

/// What the media controls were last told, so it's only updated on
/// change.
#[derive(Default, PartialEq)]
struct Published {
    song_id: Option<i64>,
    status: Option<MediaStatus>,
    can_go_next: bool,
    can_go_previous: bool,
}

impl PlaybackService {
    pub fn new(
        player: PlayerService,
        queue: QueueService,
        event_tx: UnboundedSender<CoreEvent>,
        cover_art: CoverArtService,
    ) -> Self {
        Self {
            player,
            queue,
            media_controls: MediaControlsService::new(event_tx),
            cover_art,
            published: Published::default(),
            pending_art_album: None,
        }
    }

    pub fn player(&self) -> &PlayerService {
        &self.player
    }

    pub fn queue(&self) -> &QueueService {
        &self.queue
    }

    /// Loads the persisted queue and shows its current song without playing
    /// it.
    pub fn load_queue(&mut self) -> Result<()> {
        let result = self.queue.load_from_db().map(|()| {
            if let Some((_, details)) = self.queue.current() {
                self.player.restore(details.clone());
            }
        });
        self.sync();

        result
    }

    pub fn handle_event(&mut self, event: PlaybackCommand) -> Result<()> {
        let result = self.apply(event);
        self.sync();

        result
    }

    fn apply(&mut self, event: PlaybackCommand) -> Result<()> {
        match event {
            PlaybackCommand::PlayPause => self.player.toggle_playback(),
            PlaybackCommand::Play => {
                if !is_playing(&self.player) {
                    self.player.toggle_playback()?;
                }
                Ok(())
            }
            PlaybackCommand::Pause => {
                if is_playing(&self.player) {
                    self.player.toggle_playback()?;
                }
                Ok(())
            }
            PlaybackCommand::Next => {
                let next = self.queue.next()?;
                self.play(next)
            }
            PlaybackCommand::Previous => {
                let previous = self.queue.previous()?;
                self.play(previous)
            }
            PlaybackCommand::VolumeUp => {
                self.player.increase_volume();
                Ok(())
            }
            PlaybackCommand::VolumeDown => {
                self.player.decrease_volume();
                Ok(())
            }
            PlaybackCommand::Select(index) => {
                let selected = self.queue.select(index)?;
                self.play(selected)
            }
            PlaybackCommand::Clear => self.queue.clear(),
            PlaybackCommand::Enqueue(details) => self.queue.append(details),
            PlaybackCommand::Replace { entries, start } => {
                let current = self.queue.replace(entries, start)?;
                self.play(current)
            }
        }
    }

    /// Moves on to the next song when the current one finishes.
    pub fn handle_player_event(&mut self, event: PlayerEvent) -> Result<()> {
        let result = self.apply_player_event(event);
        self.sync();

        result
    }

    fn apply_player_event(&mut self, event: PlayerEvent) -> Result<()> {
        let advance = matches!(
            &event,
            PlayerEvent::Finished { request_id } if self.player.is_current_request(*request_id)
        );
        let advanced = if advance {
            self.queue.next().and_then(|next| self.play(next))
        } else {
            Ok(())
        };
        self.player.handle_event(event)?;

        advanced
    }

    /// `None` means there was nothing to move to, so the current song is
    /// left alone.
    fn play(&mut self, details: Option<PlayingSongDetails>) -> Result<()> {
        match details {
            Some(details) => self.player.play(details),
            None => Ok(()),
        }
    }

    /// Pushes any change in playback state to the media controls.
    /// Playback state only changes through this service, so it's called after
    /// each of the entry points above.
    fn sync(&mut self) {
        let state = self.player.snapshot();
        let details = state.as_ref().map(|state| state.details.clone());
        let (status, position) = match state.as_ref().map(|state| &state.playback_state) {
            Some(PlaybackState::Loading) => {
                // The user asked for playback, it just hasn't started yet.
                (Some(MediaStatus::Playing), Duration::ZERO)
            }
            Some(PlaybackState::Playing(position)) => (Some(MediaStatus::Playing), *position),
            Some(PlaybackState::Paused(position)) => (Some(MediaStatus::Paused), *position),
            Some(PlaybackState::Stopped) => (Some(MediaStatus::Stopped), Duration::ZERO),
            None => (None, Duration::ZERO),
        };

        let current = self.queue.current().map(|(index, _)| index);
        let published = Published {
            song_id: details.as_ref().map(|details| details.song_id),
            status,
            can_go_next: match current {
                Some(index) => index + 1 < self.queue.entries().len(),
                None => !self.queue.entries().is_empty(),
            },
            can_go_previous: current.is_some_and(|index| index > 0),
        };
        if published == self.published {
            return;
        }

        if published.song_id != self.published.song_id {
            match &details {
                Some(details) => self.publish_song(details),
                None => {
                    self.pending_art_album = None;
                    self.media_controls.set_metadata(None);
                }
            }
        }
        if details.is_some() {
            self.media_controls.set_position(position);
        }
        if published.status != self.published.status {
            self.media_controls
                .set_playback_status(published.status.unwrap_or(MediaStatus::Stopped));
        }
        if published.can_go_next != self.published.can_go_next {
            self.media_controls.set_can_go_next(published.can_go_next);
        }
        if published.can_go_previous != self.published.can_go_previous {
            self.media_controls
                .set_can_go_previous(published.can_go_previous);
        }

        self.published = published;
    }

    /// Adds the cover to the current song's metadata once it's been fetched.
    pub fn on_cover_loaded(&mut self, album_id: i64) {
        if self.pending_art_album != Some(album_id) {
            return;
        }
        self.pending_art_album = None;

        if let Some(state) = self.player.snapshot()
            && state.details.album_id == album_id
            && let Some(path) = self.cover_art.cached_path(album_id)
        {
            self.media_controls
                .set_metadata(Some(track_metadata(state.details, Some(path))));
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
        self.media_controls
            .set_metadata(Some(track_metadata(details, art)));
    }
}

fn is_playing(player: &PlayerService) -> bool {
    matches!(
        player.snapshot().map(|state| state.playback_state),
        Some(PlaybackState::Playing(_) | PlaybackState::Loading)
    )
}

fn track_metadata(details: &PlayingSongDetails, art: Option<PathBuf>) -> TrackMetadata {
    TrackMetadata {
        song_id: details.song_id,
        title: details.song_title.clone(),
        artist: details.artist_name.clone(),
        album: details.album_name.clone(),
        length: Duration::from_secs(details.song_duration.max(0) as u64),
        art,
    }
}
