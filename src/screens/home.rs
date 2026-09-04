use crate::domain::AppEvent;
use crate::domain::collection::{AlbumWithSongs, CollectionEvent, CollectionSyncState};
use crate::screens::screen::{Action, Screen};
use color_eyre::eyre::eyre;
use color_eyre::{Report, Result};
use crossterm::event::Event;
use ratatui::Frame;
use ratatui::text::Text;
use ratatui::widgets::{List, Paragraph};

#[derive(Default)]
pub struct HomeScreen {
    sync_state: Option<Result<CollectionSyncState>>,
    albums: Vec<AlbumWithSongs>,
}

impl HomeScreen {}

impl Screen for HomeScreen {
    fn handle_input_event(&mut self, event: Event) -> Option<Action> {
        None
    }

    fn handle_async_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Collection(event) => match event {
                CollectionEvent::SyncState(state) => {
                    self.sync_state = Some(Ok(state.clone()));
                }
                CollectionEvent::SyncResult(result) => match result {
                    Ok(result) => {
                        self.sync_state = None;
                        self.albums = result.albums.clone();
                        crate::dbg_file!(format!(
                            "{:#}\n{:#}\n",
                            result
                                .album_errors
                                .iter()
                                .map(|e| e.to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                            result
                                .artist_errors
                                .iter()
                                .map(|e| e.to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        ));
                    }
                    Err(err) => {
                        self.sync_state = Some(Err(eyre!(format!("{:#}", err))));
                    }
                },
            },
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame) {
        match &self.sync_state {
            Some(Ok(state)) => match state {
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
                frame.render_widget(Paragraph::new(err.to_string()), frame.area());
            }
            None => {
                frame.render_widget(
                    List::new(self.albums.iter().map(|album| {
                        Text::from(format!(
                            "{} - {}",
                            album.album.id,
                            album.album.name.as_str()
                        ))
                    })),
                    frame.area(),
                );
            }
        }
        // frame.render_widget(Paragraph::new("home screen"), frame.area());
    }
}
