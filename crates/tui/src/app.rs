use crate::screens::home::HomeScreen;
use crate::screens::initial::InitScreen;
use crate::screens::login::LoginScreen;
use crate::screens::screen::{Action, Screen};
use crate::theme::get_app_theme;
use crate::ui::control::{ControlStyle, set_default_style};
use crate::ui::overlay::{AppOverlay, Overlay};
use crate::ui::player::render_player;
use crate::ui::queue::{QueueView, QueueViewAction, render_queue};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use hipp_core::domain::playback::PlaybackCommand;
use hipp_core::domain::{Core, CoreEvent, CoreNotice, StartOutcome};
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
    event_rx: UnboundedReceiver<CoreEvent>,
    core: Core,
    active_screen: Box<dyn Screen>,
    overlay: Option<AppOverlay>,
    queue_view: Option<QueueView>,
    picker: Picker,
    set_up: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let (event_tx, event_rx) = unbounded_channel::<CoreEvent>();
        let core = Core::new(event_tx)?;
        // Queries the terminal over stdin, so it has to happen before the
        // input EventStream starts reading from it.
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

        Ok(Self {
            should_quit: false,
            event_rx,
            core,
            active_screen: Box::new(InitScreen {}),
            overlay: None,
            queue_view: None,
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
                            let player_state = self.core.playback().player().snapshot();
                            self.active_screen.render(
                                frame,
                                screen_area.inner(Margin::new(1, 1)),
                                player_state.as_ref().map(|state| state.details),
                            );
                            let queue = self.core.playback().queue();
                            let queue_position = queue
                                .current()
                                .map(|(index, _)| (index + 1, queue.entries().len()));
                            frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), player_area);
                            render_player(
                                frame,
                                player_area.inner(Margin::new(1, 0)),
                                player_state.as_ref(),
                                queue_position,
                            );
                            if let Some(queue_view) = &mut self.queue_view {
                                render_queue(
                                    frame,
                                    screen_area,
                                    queue_view,
                                    queue.entries(),
                                    queue.current().map(|(index, _)| index),
                                );
                            }
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
        match self.core.start()? {
            StartOutcome::Ready => self.active_screen = Box::new(self.home_screen()),
            StartOutcome::LoginRequired => {
                self.active_screen = Box::new(LoginScreen::new());
                self.set_up = false;
            }
        }

        Ok(())
    }

    fn handle_async_event(&mut self, event: CoreEvent) {
        self.active_screen.handle_async_event(&event);

        match self.core.handle_event(event) {
            Some(CoreNotice::LoggedIn) => {
                self.overlay = None;
                self.active_screen = Box::new(self.home_screen());
                self.set_up = true;
            }
            Some(CoreNotice::Error(message)) => {
                self.overlay = Some(AppOverlay::error(Some(message)))
            }
            None => {}
        }
    }

    fn dispatch_playback_command(&mut self, event: PlaybackCommand) {
        if let Err(err) = self.core.dispatch(event) {
            self.overlay = Some(AppOverlay::error(Some(format!("{:#}", err))));
        }
    }

    fn home_screen(&self) -> HomeScreen {
        HomeScreen::new(
            self.core.collection().clone(),
            self.core.cover_art().clone(),
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
                    self.dispatch_playback_command(PlaybackCommand::PlayPause);

                    return;
                }
                KeyCode::Char('q') if self.set_up => {
                    self.queue_view = match self.queue_view {
                        Some(_) => None,
                        None => Some(QueueView::new(
                            self.core
                                .playback()
                                .queue()
                                .current()
                                .map(|(index, _)| index),
                        )),
                    };

                    return;
                }
                KeyCode::Char('<') | KeyCode::Char(',') if self.set_up => {
                    self.dispatch_playback_command(PlaybackCommand::Previous);

                    return;
                }
                KeyCode::Char('>') | KeyCode::Char('.') if self.set_up => {
                    self.dispatch_playback_command(PlaybackCommand::Next);

                    return;
                }
                KeyCode::Char('-') | KeyCode::Char('_') if self.set_up => {
                    self.dispatch_playback_command(PlaybackCommand::VolumeDown);

                    return;
                }
                KeyCode::Char('=') | KeyCode::Char('+') if self.set_up => {
                    self.dispatch_playback_command(PlaybackCommand::VolumeUp);

                    return;
                }
                _ => {}
            }
        }

        match &self.overlay {
            Some(overlay) => {
                if matches!(overlay, AppOverlay::Error(_)) {
                    if event.as_key_press_event().is_some() {
                        self.overlay = None;
                    }
                }

                return;
            }
            _ => {}
        }

        if let Some(queue_view) = &mut self.queue_view {
            if let Some(key) = event.as_key_press_event() {
                let len = self.core.playback().queue().entries().len();
                match queue_view.handle_key(key.code, len) {
                    Some(QueueViewAction::Close) => self.queue_view = None,
                    Some(QueueViewAction::Clear) => {
                        self.dispatch_playback_command(PlaybackCommand::Clear)
                    }
                    Some(QueueViewAction::Play(index)) => {
                        self.dispatch_playback_command(PlaybackCommand::Select(index))
                    }
                    None => {}
                }
            }

            return;
        }

        if let Some(action) = self.active_screen.handle_input_event(event) {
            match action {
                Action::Login(login_action) => {
                    self.overlay = Some(AppOverlay::loading(None));
                    self.core.handle_login_action(login_action)
                }
                Action::SwitchScreen(new_screen) => self.active_screen = new_screen,
                Action::PlayQueue { entries, start } => {
                    self.dispatch_playback_command(PlaybackCommand::Replace { entries, start })
                }
                Action::Enqueue(details) => {
                    self.dispatch_playback_command(PlaybackCommand::Enqueue(details))
                }
                Action::Quit => todo!(),
            }
        }
    }
}
