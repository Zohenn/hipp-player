use crate::theme::get_app_theme;
use crate::ui::color::ColorExt;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub fn format_keybinding<'a>(
    key: &'a str,
    label: Option<&'a str>,
) -> impl Iterator<Item = Span<'a>> {
    let theme = get_app_theme();

    let style = Style::default().fg(theme.primary);
    let bracket_style = Style::default().fg(theme.bg.lighten(0.25));

    let key_span = Span::from(key).style(style);

    [
        Span::from("[").style(bracket_style),
        key_span,
        Span::from("]").style(bracket_style),
    ]
    .into_iter()
    .chain(
        label
            .into_iter()
            .flat_map(|label| [Span::from(" "), Span::from(label)]),
    )
}

/// A row of `(key, label)` hints, e.g. for the bottom of a panel.
pub fn format_keybinding_hints<'a>(hints: &[(&'a str, &'a str)]) -> Line<'a> {
    Line::from_iter(hints.iter().enumerate().flat_map(|(index, (key, label))| {
        (index > 0)
            .then(|| Span::from("  "))
            .into_iter()
            .chain(format_keybinding(key, Some(label)))
    }))
}
