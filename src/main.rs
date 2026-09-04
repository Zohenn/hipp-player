use crate::app::App;
use color_eyre::Result;

mod app;
pub mod database;
pub mod debug;
pub mod domain;
mod open_subsonic;
pub mod screens;
pub mod theme;
pub mod types;
pub mod ui;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::new()?.run(terminal).await;
    ratatui::restore();

    app_result
}
