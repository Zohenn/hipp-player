use crate::screens::screen::{Action, Screen};
use hipp_core::domain::CoreEvent;
use hipp_core::domain::player::PlayingSongDetails;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

pub struct InitScreen {}

impl Screen for InitScreen {
    fn handle_input_event(&mut self, _event: Event) -> Option<Action> {
        None
    }

    fn handle_async_event(&mut self, _event: &CoreEvent) {}

    fn render(
        &mut self,
        _frame: &mut Frame,
        _rect: Rect,
        _now_playing: Option<&PlayingSongDetails>,
    ) {
    }
}
