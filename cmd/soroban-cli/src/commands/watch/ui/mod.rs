use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use crate::commands::watch::app::App;

mod detail_popup;
mod event_list;
mod filter_panel;
mod header;
mod help_popup;
mod tabs;

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(frame.area());

    header::render(frame, app, chunks[0]);
    tabs::render(frame, app, chunks[1]);
    event_list::render(frame, app, chunks[2]);

    if app.filter.panel_open {
        filter_panel::render(frame, app);
    }

    if app.detail_event.is_some() {
        detail_popup::render(frame, app);
    }

    if app.show_help {
        help_popup::render(frame, app);
    }
}
