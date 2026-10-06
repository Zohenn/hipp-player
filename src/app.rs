use crate::database::client_config::ClientConfigRepository;
use crate::database::core::backup::DatabaseBackup;
use crate::database::core::database::Database;
use crate::database::core::migration::{has_pending_migrations, migrate};
use crate::domain::login::LoginEvent;
use crate::domain::player::PlayerEvent;
use crate::domain::sync_run::SyncKind;
use crate::domain::{AppEvent, ServiceContainer};
use crate::open_subsonic::{OpenSubsonicClient, OpenSubsonicOptions};
use crate::screens::home::HomeScreen;
use crate::screens::initial::InitScreen;
use crate::screens::login::LoginScreen;
use crate::screens::screen::{Action, Screen};
use crate::theme::get_app_theme;
use crate::ui::control::{ControlStyle, set_default_style};
use crate::ui::overlay::{AppOverlay, Overlay};
use crate::ui::player::render_player;
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::EventStream;
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui_image::picker::Picker;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_stream::StreamExt;

pub struct App {
    should_quit: bool,
    event_rx: UnboundedReceiver<AppEvent>,
    service_container: ServiceContainer,
    active_screen: Box<dyn Screen>,
    overlay: Option<AppOverlay>,
    picker: Picker,
    set_up: bool,
}

impl App {
    pub fn new() -> Result<Self> {
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
        let (event_tx, event_rx) = unbounded_channel::<AppEvent>();
        // Queries the terminal over stdin, so it has to happen before the
        // input EventStream starts reading from it.
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

        Ok(Self {
            should_quit: false,
            event_rx,
            service_container: ServiceContainer::new(database, event_tx)?,
            active_screen: Box::new(InitScreen {}),
            overlay: None,
            picker,
            set_up: true,
            // overlay: Some(AppOverlay::error(Some("Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec ultricies mattis luctus. Maecenas interdum, purus et mollis finibus, nisl purus dapibus diam, pretium euismod justo lectus non enim. Mauris consectetur, felis a auctor pulvinar, enim purus porta nunc, laoreet tempus diam neque vitae tellus. Suspendisse potenti. Vestibulum lorem erat, accumsan ac magna sit amet, tincidunt tristique neque. Praesent fringilla tellus quis laoreet eleifend. Mauris non lorem a lorem malesuada elementum.".into()))),
        })
    }
}

impl App {
    const FRAMES_PER_SECOND: f32 = 60.0;

    pub async fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.initialize()?;

        let frame_time = Duration::from_secs_f32(1.0 / Self::FRAMES_PER_SECOND);
        let mut interval = tokio::time::interval(frame_time);
        let mut events = EventStream::new();

        let theme = get_app_theme();

        set_default_style(
            ControlStyle::default()
                .bg(theme.bg)
                .bg_darker(theme.bg_darker)
                .focused(theme.primary)
                .unfocused(theme.fg),
        );

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => {
                    terminal.draw(|frame| {
                        frame.render_widget(Block::default().style(Style::default().bg(theme.bg_darker)), frame.area());
                        if self.set_up {
                            let [screen_area, player_area] = frame.area().layout(&Layout::vertical([
                                Constraint::Fill(1),
                                Constraint::Length(3),
                            ]));
                            let player_state = self.service_container.player.snapshot();
                            self.active_screen.render(
                                frame,
                                screen_area.inner(Margin::new(1, 1)),
                                player_state.as_ref().map(|state| state.details),
                            );
                            render_player(frame, player_area.inner(Margin::new(1, 0)), player_state.as_ref());
                        } else {
                            self.active_screen.render(frame, frame.area(), None);
                        }

                        if let Some(overlay) = &self.overlay {
                            frame.render_widget(Overlay::new(overlay), frame.area());
                        }
                    })?;
                },
                Some(event) = self.event_rx.recv() => self.handle_async_event(event),
                Some(Ok(event)) = events.next() => self.handle_input_event(event),
            }
        }

        Ok(())
    }

    fn initialize(&mut self) -> Result<()> {
        let client_config =
            ClientConfigRepository::new(self.service_container.database.clone()).get()?;
        if let Some(client_config) = client_config {
            self.service_container
                .set_client(OpenSubsonicClient::new(OpenSubsonicOptions::new(
                    client_config.url,
                    client_config.username,
                    client_config.password,
                )));
            self.service_container.collection.load_from_db()?;
            if let Some(kind) = self.service_container.collection.due_sync()? {
                self.service_container.collection.sync(kind);
            }
            self.active_screen = Box::new(self.home_screen());
        } else {
            self.active_screen = Box::new(LoginScreen::new());
            self.set_up = false;
        }

        Ok(())
    }

    fn handle_async_event(&mut self, event: AppEvent) {
        self.active_screen.handle_async_event(&event);

        match event {
            AppEvent::Login(event) => match event {
                LoginEvent::LoginResult(result) => match result {
                    Ok(login_result) => {
                        self.service_container.set_client(login_result.client);
                        self.overlay = None;
                        self.active_screen = Box::new(self.home_screen());
                        self.service_container.collection.sync(SyncKind::Full);
                        self.set_up = true;
                    }
                    Err(err) => self.overlay = Some(AppOverlay::error(Some(format!("{:#}", err)))),
                },
            },
            AppEvent::CoverArt(_) => {}
            AppEvent::Player(event) => {
                if let PlayerEvent::Error(err) | PlayerEvent::StreamFailed { error: err, .. } =
                    &event
                {
                    self.overlay = Some(AppOverlay::error(Some(err.clone())));
                }
                if let Err(err) = self.service_container.player.handle_event(event) {
                    self.overlay = Some(AppOverlay::error(Some(format!("{:#}", err))));
                }
            }
        }
    }

    fn home_screen(&self) -> HomeScreen {
        HomeScreen::new(
            self.service_container.collection.clone(),
            self.service_container.cover_art.clone(),
            self.picker.clone(),
        )
    }

    fn handle_input_event(&mut self, event: Event) {
        if let Some(key) = event.as_key_press_event() {
            match key.code {
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                    self.should_quit = true;
                }
                KeyCode::Char(' ') if self.set_up => {
                    self.service_container.player.toggle_playback();

                    return;
                }
                KeyCode::Char('-') | KeyCode::Char('_') if self.set_up => {
                    self.service_container.player.decrease_volume();

                    return;
                }
                KeyCode::Char('=') | KeyCode::Char('+') if self.set_up => {
                    self.service_container.player.increase_volume();

                    return;
                }
                _ => {}
            }
        }

        match &self.overlay {
            Some(overlay) => {
                if matches!(overlay, AppOverlay::Error(_)) {
                    if let Some(key) = event.as_key_press_event() {
                        self.overlay = None;
                    }
                }

                return;
            }
            _ => {}
        }

        if let Some(action) = self.active_screen.handle_input_event(event) {
            match action {
                Action::Login(login_action) => {
                    self.overlay = Some(AppOverlay::loading(None));
                    self.service_container.login.handle_action(login_action)
                }
                Action::SwitchScreen(new_screen) => self.active_screen = new_screen,
                Action::PlaySong(details) => {
                    if let Err(err) = self.service_container.player.play(details) {
                        self.overlay = Some(AppOverlay::error(Some(format!("{:#}", err))));
                    }
                }
                Action::Quit => todo!(),
            }
        }
    }
}
