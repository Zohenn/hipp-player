//! Publishes what's playing to the OS and receives its media-key commands.
//!
//! The service is platform-specific: MPRIS over D-Bus on Linux, and a no-op
//! everywhere else.
// TODO: Windows (System Media Transport Controls) and macOS
// (`MPNowPlayingInfoCenter` / `MPRemoteCommandCenter`) implementations, each
// with the same `MediaControlsService` interface as `mpris`.

#[cfg(target_os = "linux")]
mod mpris;
#[cfg(not(target_os = "linux"))]
mod noop;

#[cfg(target_os = "linux")]
pub use mpris::MediaControlsService;
#[cfg(not(target_os = "linux"))]
pub use noop::MediaControlsService;

use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
pub enum MediaStatus {
    Playing,
    Paused,
    Stopped,
}

pub struct TrackMetadata {
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: Duration,
    /// Cached cover art file.
    pub art: Option<PathBuf>,
}
