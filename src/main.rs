use std::io::Stderr;

use crate::error::Error;
use app::App;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use error::TuiErrorKind;
use ratatui::prelude::CrosstermBackend;
use ratatui::{Frame, Terminal};

mod app;
mod config;
mod error;
mod util;

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    app: &mut App,
) -> Result<bool, Error> {
    loop {
        terminal.draw(|frame| {
            ui(frame, app);
        })?;
    }
}

fn ui(frame: &mut Frame, app: &App) {
    todo!()
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    enable_raw_mode().map_err(|_| Error::Tui(error::TuiErrorKind::Initilization))?;
    let mut stderr = std::io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|_| Error::Tui(error::TuiErrorKind::Initilization))?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal =
        Terminal::new(backend).map_err(|_| Error::Tui(TuiErrorKind::Initilization))?;
    let aws_config = &aws_config::load_from_env().await;

    let mut app = App::new(aws_config).await?;
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode().map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;

    terminal
        .show_cursor()
        .map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;

    if let Ok(do_print) = res {
        if do_print {
            app.view_files();
        }
    } else if let Err(err) = res {
        println!("{err:?}");
    }
    Ok(())
}
