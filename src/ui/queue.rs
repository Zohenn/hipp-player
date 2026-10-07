use crate::domain::player::PlayingSongDetails;
use crate::theme::{Theme, get_app_theme};
use crate::ui::common::{PLAYING_ALBUM_BAR, format_duration};
use crate::ui::keybinding::{format_keybinding, format_keybinding_hints};
use crate::ui::overlay::dim_area;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Padding, Row, Table, TableState};
use unicode_width::UnicodeWidthStr;

pub struct QueueView {
    /// Index into the queue, not into the rendered rows — group headers
    /// take up rows but can't be selected.
    selected: usize,
    table: TableState,
}

pub enum QueueViewAction {
    Close,
    Play(usize),
    Clear,
}

impl QueueView {
    pub fn new(current: Option<usize>) -> Self {
        Self {
            selected: current.unwrap_or(0),
            table: TableState::default(),
        }
    }

    pub fn handle_key(&mut self, key: KeyCode, len: usize) -> Option<QueueViewAction> {
        match key {
            KeyCode::Esc => return Some(QueueViewAction::Close),
            KeyCode::Char('c') => return Some(QueueViewAction::Clear),
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(len.saturating_sub(1)),
            KeyCode::Enter if self.selected < len => {
                return Some(QueueViewAction::Play(self.selected));
            }
            _ => {}
        }

        None
    }
}

/// Centered over `area` and anchored to its bottom edge, so it sits right
/// above the player bar.
pub fn render_queue(
    frame: &mut Frame,
    area: Rect,
    view: &mut QueueView,
    entries: &[PlayingSongDetails],
    current: Option<usize>,
) {
    let theme = get_app_theme();

    // The queue can shrink while the view is open (e.g. replaced from the
    // library), so the selection is clamped on every render.
    view.selected = view.selected.min(entries.len().saturating_sub(1));

    dim_area(area, frame.buffer_mut());

    let [area] = area.layout(&Layout::horizontal([Constraint::Percentage(50)]).flex(Flex::Center));
    let [area] = area.layout(&Layout::vertical([Constraint::Percentage(75)]).flex(Flex::End));

    let block = Block::new().bg(theme.bg).padding(Padding::uniform(1));
    let inner_area = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let [title_area, _, list_area, _, footer_area] = inner_area.layout(&Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ]));

    frame.render_widget(
        format_keybinding_hints(&[
            ("↑↓", "Move"),
            ("⏎", "Play"),
            ("c", "Clear"),
            ("esc", "Close"),
        ]),
        footer_area,
    );

    let title = Line::from_iter(format_keybinding("q", Some("Queue")));
    let count = Line::from(match entries.len() {
        1 => "1 song".to_owned(),
        len => format!("{len} songs"),
    })
    .fg(theme.fg_muted);
    let [title_left_area, title_right_area] = title_area.layout(
        &Layout::horizontal([
            Constraint::Length(title.width() as u16),
            Constraint::Length(count.width() as u16),
        ])
        .flex(Flex::SpaceBetween),
    );
    frame.render_widget(title, title_left_area);
    frame.render_widget(count, title_right_area);

    if entries.is_empty() {
        frame.render_widget(
            Line::from("Queue is empty").fg(theme.fg_muted).centered(),
            list_area,
        );
        return;
    }

    let mut rows = Vec::new();
    let mut selected_row = 0;
    for run in album_runs(entries) {
        let playing_in_run = current.is_some_and(|current| run.contains(&current));

        if run.len() == 1 {
            let index = run.start;
            let details = &entries[index];
            if index == view.selected {
                selected_row = rows.len();
            }
            rows.push(
                Row::new([
                    Text::from(vec![Line::from(gutter(playing_in_run, &theme)); 2]),
                    Text::from(vec![
                        Line::from(details.song_title.as_str())
                            .fg(title_fg(index == view.selected, &theme)),
                        Line::from(format!("{} • {}", details.artist_name, details.album_name))
                            .fg(theme.fg_muted),
                    ]),
                    Text::from(duration_line(details, &theme)),
                ])
                .height(2),
            );
            continue;
        }

        let first = &entries[run.start];
        rows.push(Row::new([
            Line::from(gutter(playing_in_run, &theme)),
            Line::from(vec![
                Span::from(first.album_name.as_str()).fg(theme.fg),
                Span::from(format!(" • {}", first.artist_name)).fg(theme.fg_muted),
            ]),
            Line::default(),
        ]));

        for index in run {
            let details = &entries[index];
            if index == view.selected {
                selected_row = rows.len();
            }
            rows.push(Row::new([
                Line::from(gutter(current == Some(index), &theme)),
                Line::from(format!("  {}", details.song_title))
                    .fg(title_fg(index == view.selected, &theme)),
                duration_line(details, &theme),
            ]));
        }
    }

    // Highlighting by hand, like the home screen lists: the row highlight
    // style would override the muted parts of the row.
    view.table.select(Some(selected_row));
    frame.render_stateful_widget(
        Table::new(
            rows,
            [
                Constraint::Length(PLAYING_ALBUM_BAR.width() as u16),
                Constraint::Fill(1),
                Constraint::Length(8),
            ],
        )
        .column_spacing(0),
        list_area,
        &mut view.table,
    );
}

/// Ranges of consecutive entries from the same album.
fn album_runs(entries: &[PlayingSongDetails]) -> Vec<std::ops::Range<usize>> {
    let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
    for (index, details) in entries.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if entries[run.start].album_id == details.album_id => run.end = index + 1,
            _ => runs.push(index..index + 1),
        }
    }
    runs
}

fn gutter(playing: bool, theme: &Theme) -> Span<'static> {
    if playing {
        Span::from(PLAYING_ALBUM_BAR).fg(theme.primary)
    } else {
        Span::from(" ".repeat(PLAYING_ALBUM_BAR.width()))
    }
}

fn title_fg(selected: bool, theme: &Theme) -> ratatui::style::Color {
    if selected { theme.primary } else { theme.fg }
}

fn duration_line(details: &PlayingSongDetails, theme: &Theme) -> Line<'static> {
    Line::from(format_duration(details.song_duration))
        .fg(theme.fg_muted)
        .right_aligned()
}
