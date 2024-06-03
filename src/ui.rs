use crate::app::App;

use ratatui::prelude::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Text;
use ratatui::widgets::{Cell, HighlightSpacing, Row, Table};
use ratatui::Frame;

pub fn ui(frame: &mut Frame, app: &mut App) {
    let rects = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).split(frame.size());
    frame.render_widget(Text::from("Cync".bold()), rects[0]);

    render_table(frame, app, rects[1]);
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
