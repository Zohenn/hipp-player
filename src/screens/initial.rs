use crate::domain::AppEvent;
use crate::domain::player::PlayingSongDetails;
use crate::screens::screen::{Action, Screen};
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

pub struct InitScreen {}

impl Screen for InitScreen {
    fn handle_input_event(&mut self, _event: Event) -> Option<Action> {
        None
    }

    fn handle_async_event(&mut self, _event: &AppEvent) {}

    fn render(
        &mut self,
        _frame: &mut Frame,
        _rect: Rect,
        _now_playing: Option<&PlayingSongDetails>,
    ) {
    }
}
