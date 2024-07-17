use crate::cync::{file_viewer::FileKind, Cync, Mode};
use ratatui::{
    prelude::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Cell, HighlightSpacing, Row, Table},
    Frame,
};

pub fn ui(frame: &mut Frame, app: &mut Cync) {
    let area = frame.size();
    let block = Block::default()
        .title_top("Cync".bold())
        .title_alignment(Alignment::Center)
        .borders(Borders::all());
    let block_inner = block.inner(area);
    frame.render_widget(block, area);

    render_table(frame, app, block_inner);
    render_footer(frame, app, block_inner);
}

fn render_footer(frame: &mut Frame, app: &mut Cync, area: Rect) {
    let text = match &app.mode {
        Mode::Default => String::from("Up/Down: j/k, Select: <Enter>, Quit: q"),
        Mode::PendingAction(kind) => match kind {
            FileKind::OnlyInRemote { .. } => {
                String::from("Select an action: Pull (f)rom remote, (Q)uit to previous screen")
            }
            FileKind::OnlyInLocal { .. } => {
                String::from("Select an action: Push (t)o remote, (q)uit to previous screen")
            }
            FileKind::ExistsInBoth {
                local_hash,
                remote_hash,
                ..
            } => {
                if local_hash != remote_hash {
                    String::from("Select an action: Push (t)o remote / Pull (f)rom remote")
                } else {
                    String::from("No actions availabble. Press (q) to quit")
                }
            }
        },
    };

    let block = Block::new()
        .title_bottom(text)
        .title_alignment(Alignment::Center);

    frame.render_widget(block, area);
}

fn render_table(frame: &mut Frame, app: &mut Cync, area: Rect) {
    let header_style = Style::default();
    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let header = ["Path", "Local Hash", "Remote Hash"]
        .into_iter()
        .map(Cell::from)
        .collect::<Row>()
        .style(header_style)
        .height(1);

    let files = app.view_files();
    let rows = files.iter().map(|(path, kind)| match kind {
        FileKind::OnlyInRemote { hash, .. } => Row::new(vec![
            Cell::from(path.to_owned()),
            String::new().into(),
            format!("{:?}", &hash).into(),
        ])
        .fg(Color::Yellow),
        FileKind::OnlyInLocal { hash, .. } => Row::new(vec![
            Cell::from(path.to_owned()),
            format!("{:?}", &hash).into(),
            String::new().into(),
        ])
        .fg(Color::Yellow),
        FileKind::ExistsInBoth {
            local_hash,
            remote_hash,
            ..
        } => Row::new(vec![
            Cell::from(path.to_owned()),
            Cell::from(format!("{:?}", &local_hash)),
            Cell::from(format!("{:?}", &remote_hash)),
        ])
        .fg(if local_hash == remote_hash {
            Color::Green
        } else {
            Color::Yellow
        }),
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
