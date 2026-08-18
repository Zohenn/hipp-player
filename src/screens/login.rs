use crate::screens::screen::{GlobalAction, Screen};
use crate::ui::control::{Control, ControlState};
use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::macros::vertical;
use ratatui::widgets::Block;
use std::ops::Rem;
use tui_input::Input;

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
            .map_or(if delta < 0 { 2 } else { 0 }, |(index, _)| index);

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
    fn handle_event(&mut self, event: Event) -> Option<GlobalAction> {
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

    fn render(&self, frame: &mut Frame) {
        let content_area = frame
            .area()
            .centered(Constraint::Min(1), Constraint::Min(1));

        let block = Block::bordered().title("Login");
        let inner_area = block.inner(content_area);

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(inner_area);

        frame.render_widget(block, content_area);
        frame.render_widget(Control::new(&self.url).label("URL"), chunks[0]);
        frame.render_widget(Control::new(&self.username).label("Username"), chunks[1]);
        frame.render_widget(Control::new(&self.password).label("Password"), chunks[2]);
    }
}
