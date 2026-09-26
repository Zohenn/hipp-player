use crate::domain::AppEvent;
use crate::domain::login::LoginAction;
use ratatui::Frame;
use ratatui::crossterm::event::Event;

pub trait Screen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action>;
    fn handle_async_event(&mut self, event: &AppEvent);
    fn render(&mut self, frame: &mut Frame);
}

pub enum Action {
    Login(LoginAction),
    SwitchScreen(Box<dyn Screen>),
    Quit,
}
