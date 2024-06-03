use crate::error::Error;
use app::App;
use crossterm::event::{self, Event, KeyCode};
use error::TuiErrorKind;
use logging::initialize_logging;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stderr;
use ui::ui;
use util::{initialize_terminal, restore_terminal};

mod app;
mod config;
mod error;
mod logging;
mod ui;
mod util;

#[tokio::main]
async fn main() -> Result<(), Error> {
    initialize_logging()?;
    let mut terminal = initialize_terminal()?;
    let aws_config = &aws_config::load_from_env().await;

    let mut app = App::new(aws_config).await?;
    let res = run_app(&mut terminal, &mut app).await;

    restore_terminal(terminal)?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    app: &mut App,
) -> Result<(), Error> {
    loop {
        terminal
            .draw(|frame| {
                ui(frame, app);
            })
            .map_err(|_| Error::Tui(TuiErrorKind::Drawing))?;
        if let Event::Key(key) =
            event::read().map_err(|_| Error::Tui(TuiErrorKind::KeyboardEvent))?
        {
            if app.selected_file.is_none() {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('j') => app.next_file(),
                    KeyCode::Char('k') => app.prev_file(),
                    KeyCode::Enter => {
                        app.selected_file = app.table_state.selected();
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') => app.selected_file = None,
                    KeyCode::Char('t') => {
                        app.push_file_to_remote(app.selected_file.unwrap()).await?;
                        app.selected_file = None;
                        app.referesh_app_state().await?;
                    }
                    KeyCode::Char('f') => {
                        app.pull_file_from_remote(app.selected_file.unwrap())?;
                        app.selected_file = None;
                        app.referesh_app_state().await?;
                    }
                    _ => {}
                }
            }
        }
    }
}
