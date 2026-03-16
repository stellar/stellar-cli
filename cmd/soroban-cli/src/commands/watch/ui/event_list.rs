use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

use crate::commands::watch::app::App;
use crate::commands::watch::event::{AppEvent, EventKind};

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let hint_height = u16::from(!app.auto_scroll);
    let chunks =
        Layout::vertical([Constraint::Min(0), Constraint::Length(hint_height)]).split(area);

    let list_area = chunks[0];
    let hint_area = chunks[1];

    if hint_height > 0 {
        render_hint(frame, app, hint_area);
    }

    let visible = app.visible_events();
    let total = visible.len();
    let row_num_width = total.to_string().len().max(2);
    let type_col_width = visible
        .iter()
        .map(|e| e.kind.type_label().len())
        .max()
        .unwrap_or(0);

    // row_num(row_num_width) + 2 + time(8) + 2 + badge(13) + 2 + type(type_col_width) + 2 + borders+padding(4) + highlight(2)
    let fixed: u16 = u16::try_from(row_num_width).unwrap_or(4)
        + 2
        + 8
        + 2
        + 13
        + 2
        + u16::try_from(type_col_width).unwrap_or(u16::MAX)
        + 2
        + 4
        + 2;
    let summary_max = list_area.width.saturating_sub(fixed) as usize;

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(idx, event)| {
            build_event_item(
                idx,
                total,
                row_num_width,
                event,
                type_col_width,
                summary_max,
            )
        })
        .collect();

    let count = items.len();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .padding(Padding::horizontal(1))
                .title(format!(" Events ({count}) ")),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, list_area, &mut app.list_state);
}

fn render_hint(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled(" ↑ L · Go live", Style::default().fg(Color::DarkGray)),
        Span::styled("    ", Style::default()),
    ];
    let (older_text, older_style) = if app.is_loading_older {
        (
            " ↓ Loading older…".to_string(),
            Style::default().fg(Color::Yellow),
        )
    } else if app.has_more_history {
        (
            " ↓ p · Load older".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            " ↓ no more history".to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        )
    };
    spans.push(Span::styled(older_text, older_style));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn build_event_item(
    idx: usize,
    total: usize,
    row_num_width: usize,
    event: &AppEvent,
    type_col_width: usize,
    summary_max: usize,
) -> ListItem<'static> {
    let row_num = Span::styled(
        format!("{:>row_num_width$}", total - idx),
        Style::default().fg(Color::DarkGray),
    );
    let time = event.timestamp.format("%H:%M:%S").to_string();
    let kind_span = match &event.kind {
        EventKind::Event(_) => Span::styled(
            format!("{:^13}", "Event"),
            Style::default().fg(Color::Black).bg(Color::Blue),
        ),
        EventKind::Transaction(_) => Span::styled(
            format!("{:^13}", "Transaction"),
            Style::default().fg(Color::Black).bg(Color::Green),
        ),
    };
    let type_label = event.kind.type_label().to_string();
    let summary = build_summary(&event.kind, summary_max);
    let line = Line::from(vec![
        row_num,
        Span::raw("  "),
        Span::styled(time, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        kind_span,
        Span::raw("  "),
        Span::styled(
            format!("{type_label:<type_col_width$}"),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(summary, Style::default().fg(Color::Gray)),
    ]);
    ListItem::new(line)
}

fn build_summary(kind: &EventKind, max_width: usize) -> String {
    match kind {
        EventKind::Transaction(d) => {
            let prefix = format!("{} {} op(s): ", d.status, d.operation_count);
            fit_list(&prefix, &d.operation_types, max_width)
        }
        EventKind::Event(_) => kind.summary(),
    }
}

fn fit_list(prefix: &str, items: &[String], max_width: usize) -> String {
    let total = items.len();
    if total == 0 || max_width == 0 {
        return prefix.to_string();
    }

    let mut result = prefix.to_string();
    let mut shown = 0;

    for item in items {
        let sep = if shown == 0 { "" } else { ", " };
        let next = format!("{result}{sep}{item}");
        let remaining = total - shown - 1;

        let fits = if remaining > 0 {
            format!("{next}, and {remaining} more").len() <= max_width
        } else {
            next.len() <= max_width
        };

        if fits {
            result = next;
            shown += 1;
        } else {
            if shown == 0 {
                return next; // always show at least one item
            }
            return format!("{result}, and {} more", total - shown);
        }
    }

    result
}
