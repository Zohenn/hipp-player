use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Color, Modifier};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Paragraph, Widget, Wrap};

pub enum AppOverlay {
    Loading(OverlayOptions),
    Error(OverlayOptions),
}

#[derive(Default)]
pub struct OverlayOptions {
    text: Option<String>,
    style: Option<Style>,
}

impl AppOverlay {
    pub fn loading(text: Option<String>) -> Self {
        Self::Loading(OverlayOptions {
            text,
            ..Default::default()
        })
    }

    pub fn error(text: Option<String>) -> Self {
        Self::Error(OverlayOptions {
            text,
            ..Default::default()
        })
    }
}

pub struct Overlay<'a> {
    overlay: &'a AppOverlay,
}

impl<'a> Overlay<'a> {
    pub fn new(overlay: &'a AppOverlay) -> Self {
        Overlay { overlay }
    }
}

impl<'a> Widget for Overlay<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [shadow_area, view_area] =
            area.layout(&Layout::vertical([Constraint::Fill(1), Constraint::Max(3)]));

        match self.overlay {
            AppOverlay::Loading(options) => {
                render_overlay(shadow_area, buf);
                Paragraph::new(
                    options
                        .text
                        .as_ref()
                        .map(|s| s.as_str())
                        .unwrap_or("Loading..."),
                )
                .render(view_area, buf);
            }
            AppOverlay::Error(err) => {
                render_overlay(shadow_area, buf);
                Paragraph::new(Line::from(
                    &[
                        "Error: ",
                        err.text
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("Unexpected error has occurred"),
                    ][..],
                ))
                .block(Block::default().padding(Padding::uniform(1)))
                .wrap(Wrap { trim: true })
                .render(view_area, buf);
            }
        }
    }
}

fn render_overlay(area: Rect, buf: &mut Buffer) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Color::Rgb(r, g, b) = buf[(x, y)].fg {
                buf[(x, y)].fg = Color::Rgb(r / 2, g / 2, b / 2);
            } else {
                match buf[(x, y)].fg {
                    Color::Reset | Color::White => {
                        buf[(x, y)].fg = Color::Rgb(255 / 2, 255 / 2, 255 / 2);
                    }
                    _ => {
                        buf[(x, y)].modifier.insert(Modifier::DIM);
                    }
                }
            }

            if let Color::Rgb(r, g, b) = buf[(x, y)].bg {
                buf[(x, y)].bg = Color::Rgb(r / 2, g / 2, b / 2);
            } else {
                buf[(x, y)].bg = Color::Black;
            }
        }
    }
}
