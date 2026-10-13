use crate::domain::CoreEvent;
use crate::domain::media_controls::{MediaStatus, TrackMetadata};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub struct MediaControlsService;

impl MediaControlsService {
    pub fn new(_event_tx: UnboundedSender<CoreEvent>) -> Self {
        Self
    }

    pub fn set_playback_status(&self, _status: MediaStatus) {}

    pub fn set_metadata(&self, _metadata: Option<TrackMetadata>) {}

    pub fn set_position(&self, _position: Duration) {}

    pub fn set_can_go_next(&self, _can_go_next: bool) {}

    pub fn set_can_go_previous(&self, _can_go_previous: bool) {}
}
