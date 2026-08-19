use crate::ui::color::ColorExt;
use arc_swap::{ArcSwap, Guard};
use once_cell::sync::Lazy;
use ratatui::style::Color;
use std::sync::Arc;

pub struct Theme {
    pub bg: Color,
    pub bg_darker: Color,
    pub fg: Color,
    pub fg_active: Color,
}

impl Default for Theme {
    fn default() -> Self {
        let bg = Color::Rgb(29, 37, 33);
        Self {
            bg,
            bg_darker: bg.darken(0.02),
            fg: Color::Rgb(255, 255, 255),
            fg_active: Color::Rgb(3, 253, 145),
        }
    }
}

static APP_THEME: Lazy<ArcSwap<Theme>> = Lazy::new(|| ArcSwap::from_pointee(Theme::default()));

pub fn get_app_theme() -> Guard<Arc<Theme>> {
    APP_THEME.load()
}
