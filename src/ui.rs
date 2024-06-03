use crate::app::{App, FileDetails};

use ratatui::prelude::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Cell, Clear, HighlightSpacing, Row, Table};
use ratatui::Frame;

pub fn ui(frame: &mut Frame, app: &mut App) {
    let area = frame.size();
    let block = Block::default()
        .title_top("Cync".bold())
        .title_alignment(Alignment::Center)
        .borders(Borders::all());
    let block_inner = block.inner(area);
    frame.render_widget(block, area);
    if let Some(idx) = app.selected_file {
        // Not entirely sure this works
        // Based on assumption that BTreeMaps are always ordered deterministically
        // However, underlying data could change (?)
        // What happens if external service manipulates folder in-between renders?
        let (
            _,
            FileDetails {
                are_hashes_identical,
                ..
            },
        ) = app.view_files().0.into_iter().nth(idx).unwrap();
        if !are_hashes_identical {
            render_popup(frame, app, block_inner)
        } else {
            app.selected_file = None;
        };
    }
    render_table(frame, app, block_inner);
}

fn render_popup(frame: &mut Frame, _app: &mut App, area: Rect) {
    frame.render_widget(Clear, area);
    let block = Block::new()
        .title_bottom("Select an action: Pull (f)rom remote, Push (t)o remote")
        .title_alignment(Alignment::Center);

    frame.render_widget(block, area);
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

    let files = app.view_files().0;
    let rows = files.iter().map(|(path, details)| {
        let mut file_row = vec![Cell::from(path.to_string())];

        if let Some(local) = details.local_hash() {
            file_row.push(
                Cell::from(format!("{:?}", local)).fg(if details.are_hashes_identical {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            )
        } else {
            file_row.push(Cell::from(String::new()))
        }
        if let Some(remote) = details.remote_hash() {
            file_row.push(
                Cell::from(format!("{:?}", remote)).fg(if details.are_hashes_identical {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            )
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
