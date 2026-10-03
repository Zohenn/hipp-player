use crate::domain::player::{PlaybackState, PlayerState};
use crate::theme::get_app_theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, HorizontalAlignment, Layout, Rect};
use ratatui::prelude::{Line, Span};
use ratatui::style::Stylize;
use std::time::Duration;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn render_player(frame: &mut Frame, render_area: Rect, player_state: Option<&PlayerState>) {
    let theme = get_app_theme();

    let [progress_area, playback_details_area] = render_area.layout(&Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
    ]));
    match player_state {
        Some(player_state) => {
            let current_time = match &player_state.playback_state {
                PlaybackState::Playing(time) | PlaybackState::Paused(time) => time,
                PlaybackState::Loading | PlaybackState::Stopped => &Duration::ZERO,
            };

            let line_width = render_area.width as usize;
            let padded = format!("{:─^line_width$}", player_state.details.song_title);
            let (colored_part, non_colored_part) = split_at_percent(
                &padded,
                (current_time.as_secs() as f64 / player_state.details.song_duration as f64)
                    * 100f64,
            );

            frame.render_widget(
                Line::from(vec![
                    Span::from(colored_part).fg(
                        if matches!(player_state.playback_state, PlaybackState::Playing(_)) {
                            theme.fg_active
                        } else {
                            theme.fg
                        },
                    ),
                    Span::from(non_colored_part).fg(theme.fg_muted),
                ]),
                progress_area,
            );

            let details = Line::from(format!(
                "{} • {}",
                player_state.details.artist_name, player_state.details.album_name
            ))
            .fg(theme.fg_muted);
            let time = Line::from(format!(
                "{} / {}",
                format_duration(current_time),
                format_duration(&Duration::from_secs(
                    player_state.details.song_duration as u64
                ))
            ));
            let keybindings = Line::from(vec![
                Span::from("P").fg(theme.fg_active),
                Span::from("ause"),
            ]);
            let [artist_album_area, time_area, keybindings_area] = playback_details_area.layout(
                &Layout::horizontal([
                    // Constraint::Length(details.width() as u16),
                    Constraint::Fill(1),
                    Constraint::Length(time.width() as u16),
                    // Constraint::Length(keybindings.width() as u16),
                    Constraint::Fill(1),
                ])
                .flex(Flex::SpaceBetween),
            );

            frame.render_widget(details, artist_album_area);
            frame.render_widget(time, time_area);
            frame.render_widget(
                keybindings.alignment(HorizontalAlignment::Right),
                keybindings_area,
            );
        }
        None => {
            let line_width = render_area.width as usize;
            let padded = format!("{:─^line_width$}", "Nothing is playing");

            frame.render_widget(Line::from(padded).fg(theme.fg_muted), progress_area);
        }
    }
}

fn split_at_percent(s: &str, percent: f64) -> (&str, &str) {
    let total = s.width();
    let target = (total as f64 * percent.clamp(0.0, 100.0) / 100.0).round() as usize;

    let mut cols = 0;
    for (i, c) in s.char_indices() {
        let w = c.width().unwrap_or(0);
        if cols + w > target {
            return s.split_at(i);
        }
        cols += w;
    }
    (s, "")
}

fn format_duration(duration: &Duration) -> String {
    let minutes = duration.as_secs() / 60;
    let seconds = duration.as_secs() % 60;

    format!("{:01}:{:02}", minutes, seconds)
}
