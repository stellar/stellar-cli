use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Tabs},
    Frame,
};

use crate::commands::watch::app::{App, Tab};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let (all, events, transactions) = app.tab_counts();
    let tabs = vec![
        Line::from(format!(" {} ({}) ", Tab::All.label(), all)),
        Line::from(format!(" {} ({}) ", Tab::Events.label(), events)),
        Line::from(format!(
            " {} ({}) ",
            Tab::Transactions.label(),
            transactions
        )),
    ];

    let selected = match app.active_tab {
        Tab::All => 0,
        Tab::Events => 1,
        Tab::Transactions => 2,
    };

    let tab_widget = Tabs::new(tabs)
        .block(Block::default().borders(Borders::ALL))
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tab_widget, area);
}
