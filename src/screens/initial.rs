use crate::domain::AppEvent;
use crate::screens::login::LoginScreen;
use crate::screens::screen::{Action, Screen};
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;

pub struct InitScreen {}

impl Screen for InitScreen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action> {
        Some(Action::SwitchScreen(Box::new(LoginScreen::new())))
    }

    fn handle_async_event(&mut self, event: &AppEvent) {
        todo!()
    }

    fn render(&self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ]);
        let [_, message_area, _] = frame.area().layout(&layout);

        frame.render_widget(Paragraph::new("Initializing app").centered(), message_area);
    }
}
