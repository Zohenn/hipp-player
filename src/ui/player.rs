use crate::domain::player::{PlaybackState, PlayerState};
use crate::theme::get_app_theme;
use crate::ui::keybinding::format_keybinding;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, HorizontalAlignment, Layout, Margin, Rect};
use ratatui::prelude::{Line, Span};
use ratatui::style::Stylize;
use std::iter;
use std::time::Duration;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn render_player(frame: &mut Frame, render_area: Rect, player_state: Option<&PlayerState>) {
    let theme = get_app_theme();

    let [title_area, progress_area, playback_details_area] =
        render_area.layout(&Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]));

    frame.render_widget(
        Line::from_iter(format_keybinding("0", Some("Player"))),
        title_area,
    );

    match player_state {
        Some(player_state) => {
            let current_time = match &player_state.playback_state {
                PlaybackState::Playing(time) | PlaybackState::Paused(time) => time,
                PlaybackState::Loading | PlaybackState::Stopped => &Duration::ZERO,
            };

            let current_time_formatted = format_duration(current_time);
            let duration_formatted = format_duration(&Duration::from_secs(
                player_state.details.song_duration as u64,
            ));

            let time_len = current_time_formatted.len().max(duration_formatted.len()) as u16;

            let [current_time_area, progress_area, duration_area] =
                progress_area.layout(&Layout::horizontal([
                    Constraint::Length(time_len),
                    Constraint::Fill(1),
                    Constraint::Length(time_len),
                ]));

            let progress_area = progress_area.inner(Margin::new(1, 0));
            let line_width = progress_area.width as usize;
            let padded = format!("{:━^line_width$}", player_state.details.song_title);
            let (colored_part, non_colored_part) = split_at_percent(
                &padded,
                (current_time.as_secs() as f64 / player_state.details.song_duration as f64)
                    * 100f64,
            );

            let time_color = if matches!(player_state.playback_state, PlaybackState::Playing(_)) {
                theme.fg
            } else {
                theme.fg_muted
            };
            frame.render_widget(
                Line::from(current_time_formatted).fg(time_color),
                current_time_area,
            );

            frame.render_widget(
                Line::from(vec![
                    Span::from(colored_part).fg(
                        if matches!(player_state.playback_state, PlaybackState::Playing(_)) {
                            theme.primary
                        } else {
                            theme.fg
                        },
                    ),
                    Span::from(non_colored_part).fg(theme.fg_muted),
                ]),
                progress_area,
            );

            frame.render_widget(Line::from(duration_formatted).fg(time_color), duration_area);

            let details = Line::from(format!(
                "{} • {}",
                player_state.details.artist_name, player_state.details.album_name
            ))
            .fg(theme.fg_muted);
            let playback_label = if matches!(player_state.playback_state, PlaybackState::Playing(_))
            {
                "Pause"
            } else {
                // Padded to the width of "Pause" so the line doesn't shift on toggle.
                "Play "
            };
            let volume = Line::from_iter(
                format_keybinding("␣", Some(playback_label))
                    .chain(iter::once(Span::from("  ")))
                    .chain(format_keybinding("-", None))
                    .chain(iter::once(Span::from(format!(
                        " {:3}% ",
                        player_state.volume
                    ))))
                    .chain(format_keybinding("+", None)),
            );
            let [artist_album_area, volume_area] = playback_details_area.layout(
                &Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
                    .flex(Flex::SpaceBetween),
            );

            frame.render_widget(details, artist_album_area);
            frame.render_widget(volume.alignment(HorizontalAlignment::Right), volume_area);
        }
        None => {
            let line_width = render_area.width as usize;
            let padded = format!("{:━^line_width$}", "Nothing is playing");

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
