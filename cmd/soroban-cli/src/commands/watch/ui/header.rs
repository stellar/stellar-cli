use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
    Frame,
};

use crate::commands::watch::app::App;
use crate::commands::watch::event::ConnectionStatus;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .title(format!(" {} ", app.network));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(12)])
        .split(inner);

    let rpc_span = status_span(&app.rpc_status);
    let filter_active = app.filter.is_active();
    let (filter_text, filter_style) = if filter_active {
        ("Filters: yes", Style::default().fg(Color::Yellow))
    } else {
        ("Filters: no", Style::default().fg(Color::DarkGray))
    };
    let left_line = Line::from(vec![
        rpc_span,
        Span::raw("  "),
        Span::styled(filter_text, filter_style),
    ]);
    frame.render_widget(Paragraph::new(left_line), chunks[0]);

    let right_line = Line::from(vec![
        Span::styled("?", Style::default().fg(Color::Cyan)),
        Span::styled("  Shortcuts", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(right_line), chunks[1]);
}

fn status_span(status: &ConnectionStatus) -> Span<'static> {
    match status {
        ConnectionStatus::Connected => Span::styled(
            "RPC: ● Connected".to_string(),
            Style::default().fg(Color::Green),
        ),
        ConnectionStatus::Connecting => Span::styled(
            "RPC: ◌ Connecting…".to_string(),
            Style::default().fg(Color::Yellow),
        ),
        ConnectionStatus::Error(e) => {
            Span::styled(format!("RPC: ✗ {e}"), Style::default().fg(Color::Red))
        }
    }
}
