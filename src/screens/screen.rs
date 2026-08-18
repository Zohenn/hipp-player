use ratatui::Frame;
use ratatui::crossterm::event::Event;

pub trait Screen {
    fn handle_event(&mut self, event: Event) -> Option<GlobalAction>;
    fn render(&self, frame: &mut Frame);
}

pub enum GlobalAction {
    SwitchScreen(Box<dyn Screen>),
    Quit,
}
