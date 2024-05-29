use std::io::Stderr;

use crate::error::Error;
use app::{App, FileDetails};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use error::TuiErrorKind;
use ratatui::prelude::{Alignment, Constraint, CrosstermBackend, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::symbols::border;
use ratatui::text::Text;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Cell, HighlightSpacing, Row, Table};
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
        terminal
            .draw(|frame| {
                ui(frame, app);
            })
            .map_err(|_| Error::Tui(TuiErrorKind::Drawing))?;
    }
}

fn ui(frame: &mut Frame, app: &mut App) {
    let rects = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).split(frame.size());
    let title = Title::from("Cync".bold());
    render_table(frame, app, rects[0]);
}

fn render_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let header_style = Style::default();
    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let header = ["Path", "Local Hash", "Remote Hash"]
        .into_iter()
        .map(Cell::from)
        .collect::<Row>()
        .style(header_style)
        .height(1);

    let table_state = app.view_files();
    let widths = [
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Length(10),
    ];
    let files = app.view_files();
    let rows = files.0.iter().map(|(path, details)| {
        let mut file_row = vec![Cell::from(path.to_string())];
        if let Some(local) = details.local_hash() {
            file_row.push(Cell::from(format!("{:?}", local)))
        } else {
            file_row.push(Cell::from(String::new()))
        }
        if let Some(remote) = details.remote_hash() {
            file_row.push(Cell::from(format!("{:?}", remote)))
        } else {
            file_row.push(Cell::from(String::new()))
        }
        Row::new(file_row)
    });
    let longest_item_lens = app.constraint_len_calculator();
    let bar = " █ ";
    let t = Table::new(
        rows,
        [
            // + 1 is for padding.
            Constraint::Length(longest_item_lens.0 + 1),
            Constraint::Min(longest_item_lens.1 + 1),
            Constraint::Min(longest_item_lens.2),
        ],
    )
    .header(header)
    .highlight_style(selected_style)
    .highlight_symbol(Text::from(vec![
        "".into(),
        bar.into(),
        bar.into(),
        "".into(),
    ]))
    .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(t, area, &mut app.table_state);
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
