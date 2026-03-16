use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use crate::commands::watch::app::App;
use crate::commands::watch::filter::PanelField;

pub fn render(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .title(" Filters (Tab: next field, ↑/↓: select, Enter: add, d: delete, D: clear all, -term: exclude, Esc: close) ")
        .title_alignment(Alignment::Center);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(inner);

    render_type_section(frame, app, chunks[0]);
    render_section(
        frame,
        app,
        chunks[1],
        "Addresses",
        PanelField::Addresses,
        &app.filter.addresses,
    );
    render_section(
        frame,
        app,
        chunks[2],
        "Tokens",
        PanelField::Tokens,
        &app.filter.tokens,
    );
    render_section(
        frame,
        app,
        chunks[3],
        "Event Types",
        PanelField::EventTypes,
        &app.filter.event_types,
    );
}

fn render_type_section(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.filter.panel_focus == PanelField::RowTypes;
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .border_style(border_style)
        .title(" Row Types ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let checkboxes = [
        (app.filter.show_transactions, "Transaction", 0usize),
        (app.filter.show_events, "Events", 1),
    ];

    let mut spans = Vec::new();
    for (i, (checked, label, idx)) in checkboxes.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        let focused = is_focused && app.filter.type_focus == *idx;
        let check = if *checked { "✓" } else { " " };
        let style = if focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if *checked {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!("[{check}] {label}"), style));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn render_section(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    title: &str,
    field: PanelField,
    items: &[String],
) {
    let is_focused = app.filter.panel_focus == field;
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|s| {
            let style = if s.starts_with('-') {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(s.clone(), style)))
        })
        .collect();

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .padding(Padding::horizontal(1))
                .border_style(border_style)
                .title(format!(" {title} ")),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let selection = if is_focused {
        app.filter.list_selection
    } else {
        None
    };
    let mut list_state = ListState::default().with_selected(selection);
    frame.render_stateful_widget(list, layout[0], &mut list_state);

    if is_focused {
        let input_text = format!("> {}_", app.filter.input_buffer);
        let input = Paragraph::new(input_text).style(Style::default().fg(Color::Cyan));
        frame.render_widget(input, layout[1]);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
