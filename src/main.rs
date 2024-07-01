use crate::error::Error;
use app::{App, Mode};
use crossterm::event::{self, Event, KeyCode};
use error::TuiErrorKind;
use file_viewer::FileKind;
use logging::initialize_logging;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stderr;
use ui::ui;
use util::{initialize_terminal, restore_terminal};

mod app;
mod config;
mod error;
mod file_viewer;
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
            match &app.mode {
                app::Mode::Default => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('j') => app.next_file(),
                    KeyCode::Char('k') => app.prev_file(),
                    KeyCode::Enter => app.select_file(app.table_state.selected().unwrap()),
                    _ => {}
                },
                app::Mode::PendingAction(kind) => match kind {
                    FileKind::OnlyInRemote { .. } => match key.code {
                        KeyCode::Char('t') => {
                            app.pull_file_from_remote(app.selected_file.unwrap())?;
                            app.reload_files().await?;
                            app.selected_file = None;
                        }
                        KeyCode::Char('q') => {
                            app.selected_file = None;
                            app.mode = Mode::Default;
                        }
                        _ => {}
                    },
                    FileKind::OnlyInLocal { .. } => match key.code {
                        KeyCode::Char('f') => {
                            app.pull_file_from_remote(app.selected_file.unwrap())?;
                            app.reload_files().await?;
                            app.selected_file = None;
                        }
                        KeyCode::Char('q') => {
                            app.selected_file = None;
                            app.mode = Mode::Default;
                        }
                        _ => {}
                    },
                    FileKind::ExistsInBoth {
                        local_hash,
                        remote_hash,
                        ..
                    } => match key.code {
                        KeyCode::Char('f') if local_hash != remote_hash => {
                            if local_hash != remote_hash {
                                app.pull_file_from_remote(app.selected_file.unwrap())?;
                                app.reload_files().await?;
                                app.selected_file = None;
                                app.mode = Mode::Default;
                            }
                        }
                        KeyCode::Char('t') if local_hash != remote_hash => {
                            if local_hash != remote_hash {
                                app.push_file_to_remote(app.selected_file.unwrap()).await?;
                                app.reload_files().await?;
                                app.selected_file = None;
                                app.mode = Mode::Default;
                            }
                        }
                        KeyCode::Char('q') => {
                            app.selected_file = None;
                            app.mode = Mode::Default;
                        }
                        _ => {}
                    },
                },
            }
        }
    }
}
