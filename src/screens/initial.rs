use crate::screens::login::LoginScreen;
use crate::screens::screen::{GlobalAction, Screen};
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;

pub struct InitScreen {}

impl Screen for InitScreen {
    fn handle_event(&mut self, event: Event) -> Option<GlobalAction> {
        Some(GlobalAction::SwitchScreen(Box::new(LoginScreen::new())))
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
