use crate::data::log_error;
use crate::domain::AppEvent;
use crate::domain::playback::PlaybackCommand;
use mpris_server::{Metadata, PlaybackStatus, Player, Time, zbus};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::LocalSet;

pub struct NotificationService {
    command_tx: UnboundedSender<MprisCommand>,
}

enum MprisCommand {
    PlaybackStatus(PlaybackStatus),
    Metadata(Metadata),
    Position(Time),
    CanGoNext(bool),
    CanGoPrevious(bool),
}

impl NotificationService {
    pub fn new(event_tx: UnboundedSender<AppEvent>) -> Self {
        let (command_tx, command_rx) = unbounded_channel();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => return log_error(err),
            };

            let result = LocalSet::new().block_on(&runtime, run_server(event_tx, command_rx));
            if let Err(err) = result {
                log_error(err);
            }
        });

        Self { command_tx }
    }

    pub fn set_playback_status(&self, status: PlaybackStatus) {
        self.send(MprisCommand::PlaybackStatus(status));
    }

    pub fn set_metadata(&self, metadata: Metadata) {
        self.send(MprisCommand::Metadata(metadata));
    }

    pub fn set_position(&self, position: Time) {
        self.send(MprisCommand::Position(position));
    }

    pub fn set_can_go_next(&self, can_go_next: bool) {
        self.send(MprisCommand::CanGoNext(can_go_next));
    }

    pub fn set_can_go_previous(&self, can_go_previous: bool) {
        self.send(MprisCommand::CanGoPrevious(can_go_previous));
    }

    fn send(&self, command: MprisCommand) {
        // Fails once the server thread has exited, e.g. no session bus was
        // available; MPRIS is optional, so that's already been logged.
        let _ = self.command_tx.send(command);
    }
}

async fn run_server(
    event_tx: UnboundedSender<AppEvent>,
    mut command_rx: UnboundedReceiver<MprisCommand>,
) -> zbus::Result<()> {
    let player = Player::builder("hipp_player")
        .identity("Hipp Player")
        .can_play(true)
        .can_pause(true)
        .build()
        .await?;

    let forward = |event: fn() -> PlaybackCommand| {
        let event_tx = event_tx.clone();
        move |_: &Player| {
            if let Err(err) = event_tx.send(AppEvent::PlaybackCommand(event())) {
                log_error(err);
            }
        }
    };
    player.connect_play(forward(|| PlaybackCommand::Play));
    player.connect_pause(forward(|| PlaybackCommand::Pause));
    // There's no stopped state to go to, so stopping pauses.
    player.connect_stop(forward(|| PlaybackCommand::Pause));
    player.connect_play_pause(forward(|| PlaybackCommand::PlayPause));
    player.connect_next(forward(|| PlaybackCommand::Next));
    player.connect_previous(forward(|| PlaybackCommand::Previous));

    tokio::task::spawn_local(player.run());

    while let Some(command) = command_rx.recv().await {
        let result = match command {
            MprisCommand::PlaybackStatus(status) => player.set_playback_status(status).await,
            MprisCommand::Metadata(metadata) => player.set_metadata(metadata).await,
            // Clients poll `Position`, so there's no signal to emit.
            MprisCommand::Position(position) => {
                player.set_position(position);
                Ok(())
            }
            MprisCommand::CanGoNext(value) => player.set_can_go_next(value).await,
            MprisCommand::CanGoPrevious(value) => player.set_can_go_previous(value).await,
        };
        if let Err(err) = result {
            log_error(err);
        }
    }

    Ok(())
}
