use std::io::Stderr;

use crossterm::event::{self, Event, KeyCode};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    cync::{file_viewer::FileKind, Cync, Mode},
    error::{Error, TuiErrorKind},
};
use ui::ui;

mod ui;

pub async fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    app: &mut Cync,
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
                Mode::Default => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('j') => app.next_file(),
                    KeyCode::Char('k') => app.prev_file(),
                    KeyCode::Enter => app.select_file(app.table_state.selected().unwrap()),
                    _ => {}
                },
                // TODO: Add some sort of loader while awaiting
                Mode::PendingAction(kind) => match kind {
                    FileKind::OnlyInRemote { .. } => match key.code {
                        KeyCode::Char('f') => {
                            app.pull_file_from_remote(app.selected_file.unwrap())?;
                            app.reload_files().await?;
                            app.selected_file = None;
                            app.mode = Mode::Default;
                        }
                        KeyCode::Char('q') => {
                            app.selected_file = None;
                            app.mode = Mode::Default;
                        }
                        _ => {}
                    },
                    FileKind::OnlyInLocal { .. } => match key.code {
                        KeyCode::Char('t') => {
                            app.push_file_to_remote(app.selected_file.unwrap()).await?;
                            app.reload_files().await?;
                            app.mode = Mode::Default;
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
