use crate::domain::login::LoginEvent;
use crate::domain::{AppEvent, ServiceContainer};
use crate::open_subsonic::OpenSubsonicClient;
use crate::screens::login::LoginScreen;
use crate::screens::screen::{Action, Screen};
use crate::theme::{Theme, get_app_theme};
use crate::ui::control::{ControlStyle, set_default_style};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::EventStream;
use ratatui::style::Style;
use ratatui::widgets::Block;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_stream::StreamExt;

pub struct App {
    should_quit: bool,
    event_rx: UnboundedReceiver<AppEvent>,
    service_container: ServiceContainer,
    active_screen: Box<dyn Screen>,
}

impl Default for App {
    fn default() -> Self {
        let (event_tx, event_rx) = unbounded_channel::<AppEvent>();

        Self {
            should_quit: false,
            event_rx,
            service_container: ServiceContainer::new(event_tx),
            active_screen: Box::new(LoginScreen::new()),
        }
    }
}

impl App {
    const FRAMES_PER_SECOND: f32 = 60.0;

    pub async fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let frame_time = Duration::from_secs_f32(1.0 / Self::FRAMES_PER_SECOND);
        let mut interval = tokio::time::interval(frame_time);
        let mut events = EventStream::new();

        let theme = get_app_theme();

        set_default_style(
            ControlStyle::default()
                .bg(theme.bg)
                .bg_darker(theme.bg_darker)
                .focused(theme.fg_active)
                .unfocused(theme.fg),
        );

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => {
                    terminal.draw(|frame| {
                        frame.render_widget(Block::default().style(Style::default().bg(theme.bg_darker)), frame.area());
                        self.active_screen.render(frame);
                    })?;
                },
                Some(event) = self.event_rx.recv() => self.handle_async_event(event),
                Some(Ok(event)) = events.next() => self.handle_input_event(event),
            }
        }

        Ok(())
    }

    fn handle_async_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Login(event) => {
                match event {
                    LoginEvent::LoginResult(result) => {
                        // todo: handle
                    }
                }
            }
        }
    }

    fn handle_input_event(&mut self, event: Event) {
        if let Some(key) = event.as_key_press_event() {
            match key.code {
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                    self.should_quit = true;
                }
                _ => {}
            }
        }

        if let Some(action) = self.active_screen.handle_input_event(event) {
            match action {
                Action::Login(login_action) => {
                    self.service_container.login.handle_action(login_action)
                }
                Action::SwitchScreen(new_screen) => self.active_screen = new_screen,
                Action::Quit => todo!(),
            }
        }
    }
}
