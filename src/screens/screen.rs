use crate::domain::AppEvent;
use crate::domain::login::LoginAction;
use crate::domain::player::PlayingSongDetails;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

pub trait Screen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action>;
    fn handle_async_event(&mut self, event: &AppEvent);
    fn render(
        &mut self,
        frame: &mut Frame,
        render_area: Rect,
        now_playing: Option<&PlayingSongDetails>,
    );
}

pub enum Action {
    Login(LoginAction),
    SwitchScreen(Box<dyn Screen>),
    /// Replaces the queue and plays `entries[start]`.
    PlayQueue {
        entries: Vec<PlayingSongDetails>,
        start: usize,
    },
    Enqueue(PlayingSongDetails),
    Quit,
}
