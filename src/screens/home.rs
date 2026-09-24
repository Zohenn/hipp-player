use crate::domain::AppEvent;
use crate::domain::collection::{CollectionService, CollectionSyncState};
use crate::screens::screen::{Action, Screen};
use crossterm::event::Event;
use ratatui::Frame;
use ratatui::text::Text;
use ratatui::widgets::{List, Paragraph};

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

        match &state.sync_state {
            Some(Ok(sync_state)) => match sync_state {
                CollectionSyncState::Started => {
                    frame.render_widget(Paragraph::new("Collection sync started"), frame.area());
                }
                CollectionSyncState::Fetching {
                    artist,
                    artist_number,
                    artist_count,
                    artist_album_number,
                    artist_album_count,
                    album_number,
                    album_count,
                } => {
                    frame.render_widget(
                        Paragraph::new(format!("Artist ({artist_number}/{artist_count}): {artist}. Album {artist_album_number}/{artist_album_count} ({album_number}/{album_count} total)")),
                        frame.area(),
                    );
                }
            },
            Some(Err(err)) => {
                frame.render_widget(Paragraph::new(err.as_str()), frame.area());
            }
            None => {
                frame.render_widget(
                    List::new(
                        state
                            .albums
                            .iter()
                            .map(|album| Text::from(format!("{} - {}", album.id, album.name))),
                    ),
                    frame.area(),
                );
            }
        }
    }
}
