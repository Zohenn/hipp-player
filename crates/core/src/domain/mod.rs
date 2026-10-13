use crate::database::client_config::ClientConfigRepository;
use crate::database::core::backup::DatabaseBackup;
use crate::database::core::database::Database;
use crate::database::core::migration::{has_pending_migrations, migrate};
use crate::domain::collection::CollectionService;
use crate::domain::cover_art::{CoverArtEvent, CoverArtService};
use crate::domain::login::{LoginAction, LoginEvent, LoginService};
use crate::domain::open_subsonic_streaming::OpenSubsonicStreamingService;
use crate::domain::playback::{PlaybackCommand, PlaybackService};
use crate::domain::player::{PlayerEvent, PlayerService};
use crate::domain::queue::QueueService;
use crate::domain::sync_run::SyncKind;
use crate::open_subsonic::{OpenSubsonicClient, OpenSubsonicOptions};
use color_eyre::Result;
use tokio::sync::mpsc::UnboundedSender;

pub mod album;
pub mod artist;
pub mod client_config;
pub mod collection;
pub mod cover_art;
pub mod login;
pub mod media_controls;
pub mod open_subsonic_streaming;
pub mod playback;
pub mod player;
pub mod queue;
pub mod song;
pub mod source;
pub mod sync_run;

/// Asynchronous results from the services, delivered to the frontend over the
/// channel passed to [`Core::new`]. The frontend hands each one back through
/// [`Core::handle_event`].
pub enum CoreEvent {
    Login(LoginEvent),
    CoverArt(CoverArtEvent),
    Player(PlayerEvent),
    PlaybackCommand(PlaybackCommand),
}

/// What a frontend has to react to after [`Core::handle_event`].
pub enum CoreNotice {
    /// Logging in succeeded and the new credentials are in use.
    LoggedIn,
    Error(String),
}

/// Whether [`Core::start`] found a configured server to talk to.
pub enum StartOutcome {
    Ready,
    LoginRequired,
}

/// Entry point for frontends: owns the database and every service.
pub struct Core {
    database: Database,
    client: OpenSubsonicClient,
    login: LoginService,
    collection: CollectionService,
    cover_art: CoverArtService,
    playback: PlaybackService,
}

impl Core {
    /// Opens and migrates the database, then builds the services. Must be
    /// called from within a tokio runtime.
    pub fn new(event_tx: UnboundedSender<CoreEvent>) -> Result<Self> {
        let database = Database::new()?;
        let backup = DatabaseBackup::new(database.clone())?;
        // A backup from earlier today may predate data written since, so a
        // migration always gets a fresh one.
        if has_pending_migrations(&database)? {
            backup.create()?;
        } else {
            backup.create_if_due()?;
        }
        migrate(&database)?;
        backup.spawn_periodic();

        let client = OpenSubsonicClient::new(OpenSubsonicOptions::default());
        let cover_art = CoverArtService::new(database.clone(), client.clone(), event_tx.clone())?;
        Ok(Self {
            database: database.clone(),
            client: client.clone(),
            login: LoginService::new(database.clone(), event_tx.clone()),
            collection: CollectionService::new(database.clone(), client.clone()),
            cover_art: cover_art.clone(),
            playback: PlaybackService::new(
                PlayerService::new(
                    database.clone(),
                    OpenSubsonicStreamingService::new(client)?,
                    event_tx.clone(),
                ),
                QueueService::new(database),
                event_tx,
                cover_art,
            ),
        })
    }

    /// Restores the saved session, if there is one: loads the collection and
    /// queue and starts a sync if one is due.
    pub fn start(&mut self) -> Result<StartOutcome> {
        let client_config = ClientConfigRepository::new(self.database.clone()).get()?;
        let Some(client_config) = client_config else {
            return Ok(StartOutcome::LoginRequired);
        };

        self.client.set_options(OpenSubsonicOptions::new(
            client_config.url,
            client_config.username,
            client_config.password,
        ));
        self.collection.load_from_db()?;
        self.playback.load_queue()?;
        if let Some(kind) = self.collection.due_sync()? {
            self.collection.sync(kind);
        }

        Ok(StartOutcome::Ready)
    }

    pub fn collection(&self) -> &CollectionService {
        &self.collection
    }

    pub fn cover_art(&self) -> &CoverArtService {
        &self.cover_art
    }

    pub fn playback(&self) -> &PlaybackService {
        &self.playback
    }

    /// The result arrives as a [`LoginEvent`].
    pub fn handle_login_action(&self, action: LoginAction) {
        self.login.handle_action(action);
    }

    pub fn dispatch(&mut self, command: PlaybackCommand) -> Result<()> {
        self.playback.handle_event(command)
    }

    pub fn handle_event(&mut self, event: CoreEvent) -> Option<CoreNotice> {
        match event {
            CoreEvent::Login(LoginEvent::LoginResult(result)) => match result {
                Ok(login_result) => {
                    self.client.set_options(login_result.options);
                    self.collection.sync(SyncKind::Full);

                    Some(CoreNotice::LoggedIn)
                }
                Err(err) => Some(CoreNotice::Error(format!("{:#}", err))),
            },
            CoreEvent::CoverArt(CoverArtEvent::Loaded { album_id, .. }) => {
                self.playback.on_cover_loaded(album_id);

                None
            }
            CoreEvent::Player(event) => {
                let mut notice = None;
                if let PlayerEvent::Error(err) | PlayerEvent::StreamFailed { error: err, .. } =
                    &event
                {
                    notice = Some(CoreNotice::Error(err.clone()));
                }
                if let Err(err) = self.playback.handle_player_event(event) {
                    notice = Some(CoreNotice::Error(format!("{:#}", err)));
                }

                notice
            }
            CoreEvent::PlaybackCommand(command) => self
                .dispatch(command)
                .err()
                .map(|err| CoreNotice::Error(format!("{:#}", err))),
        }
    }
}
