use crate::domain::AppEvent;
use crate::domain::login::{LoginAction, LoginParams};
use crate::domain::player::PlayingSongDetails;
use crate::screens::screen::{Action, Screen};
use crate::theme::get_app_theme;
use crate::ui::control::{Control, ControlState};
use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph};

#[derive(Default)]
pub struct LoginScreen {
    url: ControlState,
    username: ControlState,
    password: ControlState,
}

impl LoginScreen {
    pub fn new() -> Self {
        Self {
            url: "https://bandcamp.com/api/subsonic".into(),
            username: ControlState::auto_focused(),
            ..Default::default()
        }
    }

    fn focus_input(&mut self, delta: isize) {
        let mut inputs = [&mut self.url, &mut self.username, &mut self.password];

        let focused_index = inputs
            .iter_mut()
            .enumerate()
            .find(|input| input.1.focused())
            .map_or(if delta < 0 { 0 } else { 2 }, |(index, _)| index);

        let new_index = if delta > 0 {
            (focused_index + delta as usize) % inputs.len()
        } else {
            (focused_index as isize + delta).rem_euclid(inputs.len() as isize) as usize
        };

        for input in inputs.iter_mut() {
            input.blur();
        }

        inputs[new_index].focus();
    }
}

impl Screen for LoginScreen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action> {
        if let Some(key) = event.as_key_press_event() {
            match key.code {
                KeyCode::Up => {
                    self.focus_input(-1);

                    return None;
                }
                KeyCode::Down => {
                    self.focus_input(1);

                    return None;
                }
                KeyCode::Enter => {
                    return Some(Action::Login(LoginAction::Login(LoginParams {
                        url: self.url.value().to_owned(),
                        username: self.username.value().to_owned(),
                        password: self.password.value().to_owned(),
                    })));
                }
                _ => {}
            }
        }

        for control in [&mut self.url, &mut self.username, &mut self.password].iter_mut() {
            if control.focused() {
                control.handle_input(&event);
            }
        }

        None
    }

    fn handle_async_event(&mut self, _event: &AppEvent) {}

    fn render(
        &mut self,
        frame: &mut Frame,
        render_area: Rect,
        _now_playing: Option<&PlayingSongDetails>,
    ) {
        let theme = get_app_theme();

        let content_area = render_area.centered(Constraint::Max(50), Constraint::Length(14));
        let [dialog, hint] = content_area.layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));

        let block = Block::default()
            .style(Style::default().bg(theme.bg))
            .padding(Padding::uniform(1));
        let inner_area = block.inner(dialog);

        let [title, url, username, password] = inner_area.layout(&Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ]));

        frame.render_widget(block, dialog);
        frame.render_widget(Paragraph::new("API configuration").centered(), title);
        frame.render_widget(Control::new(&self.url).label("URL"), url);
        frame.render_widget(Control::new(&self.username).label("Username"), username);
        frame.render_widget(Control::new(&self.password).label("Password"), password);

        // match &self.login_state {
        //     AsyncJobState::Pending => {
        //         frame.render_widget(Paragraph::new("Loading...").centered(), status)
        //     }
        //     AsyncJobState::Error(err) => {
        //         frame.render_widget(Paragraph::new(err.as_str()).centered(), status)
        //     }
        //     _ => {}
        // }

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("↑↓", Style::default().fg(theme.primary)),
                " Change focus  ".into(),
                Span::styled("Enter", Style::default().fg(theme.primary)),
                " Submit".into(),
            ]))
            .block(Block::default().padding(Padding::horizontal(1))),
            hint,
        );

        // frame.render_widget(Overlay::new(&self.login_state), frame.area());
    }
}
