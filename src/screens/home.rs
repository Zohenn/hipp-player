use crate::domain::AppEvent;
use crate::screens::screen::{Action, Screen};
use crossterm::event::Event;
use ratatui::Frame;
use ratatui::widgets::Paragraph;

pub struct HomeScreen {}

impl HomeScreen {}

impl Screen for HomeScreen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action> {
        None
    }

    fn handle_async_event(&mut self, event: &AppEvent) {
        //
    }

    fn render(&self, frame: &mut Frame) {
        frame.render_widget(Paragraph::new("home screen"), frame.area());
    }
}
