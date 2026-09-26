use crate::domain::AppEvent;
use crate::domain::collection::{CollectionService, CollectionSyncState};
use crate::screens::screen::{Action, Screen};
use crate::theme::get_app_theme;
use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::prelude::Layout;
use ratatui::style::{Style, Stylize};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, List, Paragraph};

pub struct HomeScreen {
    collection: CollectionService,
}

impl HomeScreen {
    pub fn new(collection: CollectionService) -> Self {
        Self { collection }
    }
}

impl Screen for HomeScreen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action> {
        None
    }

    fn handle_async_event(&mut self, event: &AppEvent) {}

    fn render(&self, frame: &mut Frame) {
        let state = self.collection.state();
        let theme = get_app_theme();
        let [main_area, gap_area, sync_status_area] = frame.area().layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]));

        frame.render_widget(
            List::new(
                state
                    .albums
                    .iter()
                    .map(|album| Text::from(format!("{} - {}", album.id, album.name))),
            )
            .style(Style::new().bg(theme.bg)),
            main_area,
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
            None => Paragraph::new("No sync happening"),
        };

        frame.render_widget(paragraph.block(Block::new().bg(theme.bg)), sync_status_area);
    }
}
