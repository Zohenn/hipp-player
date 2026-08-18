use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{StatefulWidget, Widget};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use std::sync::RwLock;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

#[derive(Default)]
pub struct ControlStyle {
    unfocused: Option<Color>,
    focused: Option<Color>,
}

impl ControlStyle {
    pub fn unfocused(mut self, unfocused: Color) -> Self {
        self.unfocused = Some(unfocused);

        self
    }

    pub fn focused(mut self, focused: Color) -> Self {
        self.focused = Some(focused);

        self
    }
}

static DEFAULT_STYLE: RwLock<ControlStyle> = RwLock::new(ControlStyle {
    unfocused: None,
    focused: None,
});

pub fn set_default_style(style: ControlStyle) {
    if let Ok(mut default_style) = DEFAULT_STYLE.write() {
        default_style.unfocused = style.unfocused;
        default_style.focused = style.focused;
    }
}

#[derive(Default, PartialEq)]
pub enum ControlMode {
    #[default]
    Normal,
    Focused,
}

pub struct ControlState {
    input: Input,
    mode: ControlMode,
}

impl ControlState {
    pub fn auto_focused() -> Self {
        Self {
            mode: ControlMode::Focused,
            ..Default::default()
        }
    }

    pub const fn focused(&self) -> bool {
        matches!(self.mode, ControlMode::Focused)
    }

    pub const fn focus(&mut self) {
        self.mode = ControlMode::Focused;
    }

    pub const fn blur(&mut self) {
        self.mode = ControlMode::Normal;
    }

    pub fn handle_input(&mut self, event: &Event) {
        if self.mode != ControlMode::Focused {
            return;
        }

        if let Some(key) = event.as_key_press_event() {
            match key.code {
                KeyCode::Esc => self.mode = ControlMode::Normal,
                _ => {
                    self.input.handle_event(event);
                }
            }
        }
    }
}

impl Default for ControlState {
    fn default() -> Self {
        Self {
            input: Input::default(),
            mode: Default::default(),
        }
    }
}

impl From<&str> for ControlState {
    fn from(s: &str) -> Self {
        Self {
            input: s.into(),
            mode: Default::default(),
        }
    }
}

pub struct Control<'a> {
    state: &'a ControlState,
    label: Line<'a>,
}

impl<'a> Control<'a> {
    pub fn new(state: &'a ControlState) -> Control<'a> {
        Self {
            state,
            label: Line::default(),
        }
    }

    pub fn label(mut self, label: impl Into<Line<'a>>) -> Self {
        self.label = label.into();

        self
    }
}

impl Widget for Control<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let default_style = DEFAULT_STYLE.read().unwrap();
        let focused_color = default_style.focused.unwrap_or(Color::Cyan);

        let width = area.width.max(3) - 3;
        let scroll = self.state.input.visual_scroll(width as usize);
        let input_style = match self.state.mode {
            ControlMode::Normal => default_style
                .unfocused
                .map(|c| Style::default().fg(c))
                .unwrap_or(Style::default()),
            ControlMode::Focused => focused_color.into(),
        };
        let input = Paragraph::new(self.state.input.value())
            .style(input_style)
            .scroll((0, scroll as u16))
            .block(Block::bordered().title(self.label));

        input.render(area, buf);

        if self.state.mode == ControlMode::Focused {
            let text_x = self.state.input.visual_cursor().max(scroll) - scroll;
            let x = text_x + 1;
            Line::from(self.state.input.value().get(text_x..=text_x).unwrap_or(" "))
                .style(Style::default().bg(focused_color).fg(Color::Black))
                .render(Rect::new(area.x + x as u16, area.y + 1, 1, 1), buf);
        }
    }
}

// impl StatefulWidget for Control {
//     type State = ControlState;
//
//     fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
//         let width = area.width.max(3) - 3;
//         let scroll = state.input.visual_scroll(width as usize);
//         let style = match state.mode {
//             ControlMode::Normal => Style::default(),
//             ControlMode::Focused => Color::Yellow.into(),
//         };
//         let input = Paragraph::new(state.input.value())
//             .style(style)
//             .scroll((0, scroll as u16))
//             .block(Block::bordered().title("Input"));
//
//         input.render(area, buf);
//
//         if state.mode == ControlMode::Focused {
//             let x = state.input.visual_cursor().max(scroll) - scroll + 1;
//         }
//     }
// }
