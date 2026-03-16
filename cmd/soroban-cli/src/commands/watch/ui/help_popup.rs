use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding},
    Frame,
};

use crate::commands::watch::app::App;

const SHORTCUTS: &[(&str, &str)] = &[
    ("?", "Toggle this help"),
    ("q / Ctrl-C", "Quit"),
    ("Tab / BackTab", "Switch tab"),
    ("j / ↓", "Scroll down (older)"),
    ("k / ↑", "Scroll up (newer)"),
    ("g", "Jump to top"),
    ("G", "Jump to bottom"),
    ("L", "Go live (resume auto-scroll)"),
    ("p / PgUp", "Load older events"),
    ("f", "Open filter panel"),
    ("Enter", "Open event detail"),
    ("Esc", "Close popup / panel"),
];

pub fn render(frame: &mut Frame, _app: &App) {
    let area = centered_rect(
        50,
        u16::try_from(SHORTCUTS.len()).unwrap_or(u16::MAX) + 4,
        frame.area(),
    );
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .title(" Shortcuts ")
        .title_alignment(Alignment::Center);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = SHORTCUTS
        .iter()
        .map(|(key, desc)| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{key:<18}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(y.saturating_sub(area.y)),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area);

    let mid = chunks[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(x.saturating_sub(area.x)),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(mid)[1]
}
