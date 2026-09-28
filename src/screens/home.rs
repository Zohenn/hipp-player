use crate::domain::AppEvent;
use crate::domain::collection::{CollectionService, CollectionState, CollectionSyncState};
use crate::domain::cover_art::{CoverArtEvent, CoverArtService};
use crate::domain::song::Song;
use crate::screens::screen::{Action, Screen};
use crate::theme::{Theme, get_app_theme};
use chrono::Local;
use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Margin, Rect};
use ratatui::prelude::Layout;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    Block, Borders, List, ListState, Padding, Paragraph, Row, Table, TableState,
};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{FontSize, Resize, StatefulImage};

pub struct HomeScreen {
    collection: CollectionService,
    cover_art: CoverArtService,
    picker: Picker,
    album_list: ListState,
    open_album: Option<OpenAlbum>,
    focus: Focus,
}

struct OpenAlbum {
    album_id: i64,
    songs: Result<Vec<Song>, String>,
    song_table: TableState,
    cover: Cover,
}

enum Cover {
    Loading,
    Ready(StatefulProtocol),
    /// No cover, or it failed to load — either way nothing to show.
    Unavailable,
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Albums,
    Songs,
}

impl HomeScreen {
    pub fn new(collection: CollectionService, cover_art: CoverArtService, picker: Picker) -> Self {
        Self {
            collection,
            cover_art,
            picker,
            album_list: ListState::default(),
            open_album: None,
            focus: Focus::Albums,
        }
    }

    fn open_selected_album(&mut self) {
        let Some(selected) = self.album_list.selected() else {
            return;
        };
        // A past-the-end selection is only clamped at render time, and input
        // can arrive before the next render — treat it as the last album.
        let album_id = self.collection.with_state(|state| {
            state
                .albums
                .get(selected)
                .or(state.albums.last())
                .map(|album| album.id)
        });
        let Some(album_id) = album_id else {
            return;
        };

        self.open_album = Some(OpenAlbum {
            album_id,
            songs: self
                .collection
                .album_songs(album_id)
                .map_err(|err| format!("{err:#}")),
            song_table: TableState::default().with_selected(Some(0)),
            cover: Cover::Loading,
        });
        self.cover_art.request(album_id);
        self.focus = Focus::Songs;
    }
}

impl Screen for HomeScreen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action> {
        if let Some(key) = event.as_key_press_event() {
            match (self.focus, &mut self.open_album) {
                (Focus::Albums, _) => match key.code {
                    KeyCode::Up => self.album_list.select_previous(),
                    KeyCode::Down => self.album_list.select_next(),
                    KeyCode::Enter => self.open_selected_album(),
                    KeyCode::Right if self.open_album.is_some() => self.focus = Focus::Songs,
                    _ => {}
                },
                (Focus::Songs, Some(open_album)) => match key.code {
                    KeyCode::Up => open_album.song_table.select_previous(),
                    KeyCode::Down => open_album.song_table.select_next(),
                    KeyCode::Left => self.focus = Focus::Albums,
                    _ => {}
                },
                (Focus::Songs, None) => self.focus = Focus::Albums,
            }
        }

        None
    }

    fn handle_async_event(&mut self, event: &AppEvent) {
        // The user may have moved on to another album while this one loaded.
        if let AppEvent::CoverArt(CoverArtEvent::Loaded { album_id, result }) = event
            && let Some(open_album) = &mut self.open_album
            && open_album.album_id == *album_id
        {
            open_album.cover = match result {
                Ok(Some(image)) => {
                    Cover::Ready(self.picker.new_resize_protocol(image.as_ref().clone()))
                }
                Ok(None) | Err(_) => Cover::Unavailable,
            };
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let Self {
            collection,
            picker,
            album_list,
            open_album,
            focus,
            ..
        } = self;
        collection.with_state(|state| {
            render_state(
                frame,
                state,
                album_list,
                open_album.as_mut(),
                *focus,
                picker.font_size(),
            )
        });
    }
}

fn render_state(
    frame: &mut Frame,
    state: &CollectionState,
    album_list: &mut ListState,
    open_album: Option<&mut OpenAlbum>,
    focus: Focus,
    font_size: FontSize,
) {
    let theme = get_app_theme();
    let screen_area = frame.area().inner(Margin::new(1, 1));
    frame.render_widget(Block::new().bg(theme.bg), screen_area);

    let [main_area, sync_status_area] = screen_area.layout(&Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(2),
    ]));
    let [album_list_area, album_details_area] = main_area.layout(&Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ]));

    clamp_selection(
        album_list,
        ListState::selected,
        ListState::select,
        state.albums.len(),
    );

    // Not using List::highlight_style — it's applied over the whole rendered
    // row, which would also recolor the muted artist line.
    let selected = album_list.selected();
    frame.render_stateful_widget(
        List::new(state.albums.iter().enumerate().map(|(index, album)| {
            let name_fg = if selected == Some(index) {
                theme.fg_active
            } else {
                theme.fg
            };
            Text::from(vec![
                Line::from(album.name.as_str()).fg(name_fg),
                Line::from(artist_name(state, album.artist_id)).fg(theme.fg_muted),
            ])
        }))
        .style(Style::new().bg(theme.bg)),
        album_list_area,
        album_list,
    );

    render_album_details(
        frame,
        album_details_area,
        state,
        open_album,
        focus == Focus::Songs,
        font_size,
        &theme,
    );

    let paragraph = match &state.sync_state {
        Some(Ok(sync_state)) => match sync_state {
            CollectionSyncState::Started => Paragraph::new("Collection sync started"),
            CollectionSyncState::Fetching {
                artist,
                album,
                album_number,
                album_count,
            } => Paragraph::new(format!(
                "Album {album_number}/{album_count}: {artist} - {album}"
            )),
        },
        Some(Err(err)) => Paragraph::new(err.as_str()),
        None => Paragraph::new(match state.last_synced_at {
            Some(last_synced_at) => format!(
                "Last synced {}",
                last_synced_at
                    .with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M")
            ),
            None => "Never synced".to_owned(),
        }),
    };

    frame.render_widget(
        paragraph.block(
            Block::new()
                .borders(Borders::TOP)
                .border_style(Style::new().fg(theme.border)),
        ),
        sync_status_area,
    );
}

fn render_album_details(
    frame: &mut Frame,
    area: Rect,
    state: &CollectionState,
    open_album: Option<&mut OpenAlbum>,
    focused: bool,
    font_size: FontSize,
    theme: &Theme,
) {
    let block = Block::new()
        .borders(Borders::LEFT)
        .border_style(Style::new().fg(theme.border))
        .padding(Padding::horizontal(1));
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let Some(open_album) = open_album else {
        return;
    };
    // The album can disappear from state (e.g. pruned by a sync) while open.
    let Some(album) = state
        .albums
        .iter()
        .find(|album| album.id == open_album.album_id)
    else {
        return;
    };

    // Space stays reserved while loading so the layout doesn't jump once
    // the cover arrives.
    let (cover_height, cover_gap) = match open_album.cover {
        Cover::Unavailable => (0, 0),
        Cover::Loading | Cover::Ready(_) => (cover_height(inner_area, font_size), 1),
    };
    let [cover_area, _, header_area, songs_area] = inner_area.layout(&Layout::vertical([
        Constraint::Length(cover_height),
        Constraint::Length(cover_gap),
        Constraint::Length(3),
        Constraint::Fill(1),
    ]));

    render_cover(frame, cover_area, &mut open_album.cover, font_size, theme);

    frame.render_widget(
        Text::from(vec![
            Line::from(album.name.as_str()).fg(theme.fg),
            Line::from(artist_name(state, album.artist_id)).fg(theme.fg_muted),
        ])
        .centered(),
        header_area,
    );

    let songs = match &open_album.songs {
        Ok(songs) => songs,
        Err(err) => {
            frame.render_widget(Paragraph::new(err.as_str()), songs_area);
            return;
        }
    };

    // Unlike List, Table doesn't clamp the selection itself.
    let song_table = &mut open_album.song_table;
    clamp_selection(
        song_table,
        TableState::selected,
        TableState::select,
        songs.len(),
    );

    // Highlighting by hand for the same reason as the album list: the row
    // highlight style would override the muted track number and duration.
    let selected = song_table.selected().filter(|_| focused);
    frame.render_stateful_widget(
        Table::new(
            songs.iter().enumerate().map(|(index, song)| {
                let title_fg = if selected == Some(index) {
                    theme.fg_active
                } else {
                    theme.fg
                };
                Row::new([
                    Line::from(
                        song.track_number
                            .map(|track| track.to_string())
                            .unwrap_or_default(),
                    )
                    .fg(theme.fg_muted)
                    .right_aligned(),
                    Line::from(song.title.as_str()).fg(title_fg),
                    Line::from(format_duration(song.duration_seconds))
                        .fg(theme.fg_muted)
                        .right_aligned(),
                ])
            }),
            [
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Length(8),
            ],
        ),
        songs_area,
        song_table,
    );
}

/// Covers take up to half of the panel's height, like the now-playing view
/// of a mobile player, leaving the rest for the song list.
fn cover_height(area: Rect, font_size: FontSize) -> u16 {
    // A square in pixels is `font_height / font_width` times wider than it is
    // tall in cells.
    let height_for_width =
        u32::from(area.width) * u32::from(font_size.width) / u32::from(font_size.height);
    (area.height / 2).min(height_for_width as u16)
}

fn render_cover(
    frame: &mut Frame,
    area: Rect,
    cover: &mut Cover,
    font_size: FontSize,
    theme: &Theme,
) {
    let width = u32::from(area.height) * u32::from(font_size.height) / u32::from(font_size.width);
    let [area] =
        area.layout(&Layout::horizontal([Constraint::Length(width as u16)]).flex(Flex::Center));

    match cover {
        Cover::Loading => frame.render_widget(Block::new().bg(theme.bg_darker), area),
        Cover::Ready(protocol) => frame.render_stateful_widget(
            StatefulImage::default().resize(Resize::Fit(None)),
            area,
            protocol,
        ),
        Cover::Unavailable => {}
    }
}

/// `select_next`/`select_previous` can leave the selection past the end
/// (e.g. `usize::MAX` from Up with nothing selected), and highlighting is
/// computed before the widget renders, so it has to be clamped up front.
fn clamp_selection<S>(
    state: &mut S,
    selected: fn(&S) -> Option<usize>,
    select: fn(&mut S, Option<usize>),
    len: usize,
) {
    match selected(state) {
        Some(_) if len == 0 => select(state, None),
        Some(index) if index >= len => select(state, Some(len - 1)),
        _ => {}
    }
}

fn artist_name(state: &CollectionState, artist_id: i64) -> &str {
    state
        .artists
        .get(&artist_id)
        .map_or("", |artist| artist.name.as_str())
}

fn format_duration(seconds: i64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
