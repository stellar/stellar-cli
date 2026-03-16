use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::commands::watch::app::App;
use crate::commands::watch::event::{EventKind, WorkerMessage};
use crate::commands::watch::sources;
use crate::commands::watch::spec_cache;
use crate::commands::watch::ui;
use crate::config::{locator, network};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

enum Action {
    None,
    LoadOlder(u32),
    Quit,
}

pub fn setup_terminal() -> Result<Tui, std::io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore_terminal(terminal: &mut Tui) -> Result<(), std::io::Error> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn spawn_rpc(
    tx: mpsc::UnboundedSender<WorkerMessage>,
    rpc_url: String,
    poll_interval: u64,
    network_passphrase: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        sources::rpc::run_rpc_supervisor(rpc_url, poll_interval, tx, network_passphrase).await;
    })
}

#[allow(clippy::too_many_arguments)]
pub fn run_event_loop(
    mut terminal: Tui,
    mut app: App,
    rx: mpsc::UnboundedReceiver<WorkerMessage>,
    tx: mpsc::UnboundedSender<WorkerMessage>,
    rpc_url: String,
    poll_interval: u64,
    network_passphrase: String,
    locator: locator::Args,
    network_args: network::Args,
) -> Result<(), BoxError> {
    let mut rx = rx;

    let rpc_handle = spawn_rpc(
        tx.clone(),
        rpc_url.clone(),
        poll_interval,
        network_passphrase.clone(),
    );

    loop {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                WorkerMessage::NewEvent(event) => {
                    if let EventKind::Event(d) = &event.kind {
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            app.spec_cache.entry(d.contract_id.clone())
                        {
                            e.insert(None);
                            let contract_id = d.contract_id.clone();
                            let tx2 = tx.clone();
                            let loc = locator.clone();
                            let net = network_args.clone();
                            tokio::spawn(async move {
                                let spec_entries =
                                    spec_cache::fetch_spec_entries(&contract_id, &loc, &net).await;
                                let _ = tx2.send(WorkerMessage::SpecFetched {
                                    contract_id,
                                    spec_entries,
                                });
                            });
                        }
                    }
                    app.push_event(event);
                }
                WorkerMessage::RpcStatus(status) => app.rpc_status = status,
                WorkerMessage::OlderFetched {
                    events,
                    oldest_available,
                } => {
                    app.prepend_events(events, oldest_available);
                }
                WorkerMessage::SpecFetched {
                    contract_id,
                    spec_entries,
                } => {
                    let spec = spec_entries.map(|e| soroban_spec_tools::Spec::new(&e));
                    app.spec_cache.insert(contract_id, spec);
                }
            }
        }

        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match handle_key(&mut app, key) {
                    Action::Quit => {
                        app.should_quit = true;
                    }
                    Action::LoadOlder(before_ledger) => {
                        app.is_loading_older = true;
                        let tx2 = tx.clone();
                        let url = rpc_url.clone();
                        let passphrase = network_passphrase.clone();
                        tokio::spawn(async move {
                            sources::rpc::fetch_older(url, before_ledger, tx2, passphrase).await;
                        });
                    }
                    Action::None => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    rpc_handle.abort();

    restore_terminal(&mut terminal)?;
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    if app.show_help {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q' | '?')) {
            app.show_help = false;
        }
        return Action::None;
    }

    if app.detail_event.is_some() {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => app.close_popup(),
            KeyCode::Char('j') | KeyCode::Down => {
                app.detail_scroll = app.detail_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.detail_scroll = app.detail_scroll.saturating_sub(1);
            }
            _ => {}
        }
        return Action::None;
    }

    if app.filter.panel_open {
        handle_filter_key(app, key);
        return Action::None;
    }

    match key.code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Tab => app.switch_tab(true),
        KeyCode::BackTab => app.switch_tab(false),
        KeyCode::Char('j') | KeyCode::Down => app.scroll_down(),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
        KeyCode::Char('g') => app.jump_top(),
        KeyCode::Char('G') => app.jump_bottom(),
        KeyCode::Char('L') => app.see_latest(),
        KeyCode::Char('f') => app.toggle_filter_panel(),
        KeyCode::Enter => app.open_detail(),
        KeyCode::Esc => app.close_popup(),
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char('p') | KeyCode::PageUp => {
            if !app.is_loading_older && app.has_more_history {
                if let Some(before_ledger) = app.oldest_fetched_ledger() {
                    return Action::LoadOlder(before_ledger);
                }
            }
        }
        _ => {}
    }

    Action::None
}

fn handle_filter_key(app: &mut App, key: KeyEvent) {
    use crate::commands::watch::filter::PanelField;

    if app.filter.panel_focus == PanelField::RowTypes {
        match key.code {
            KeyCode::Esc => {
                app.filter.panel_open = false;
            }
            KeyCode::BackTab => {
                app.filter.cycle_focus_back();
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                app.filter.cycle_focus_back();
            }
            KeyCode::Tab => {
                app.filter.cycle_focus();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                app.filter.type_focus = app.filter.type_focus.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.filter.type_focus = (app.filter.type_focus + 1).min(1);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                app.filter.toggle_focused_type();
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.filter.panel_open = false;
        }
        KeyCode::BackTab => {
            app.filter.cycle_focus_back();
        }
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.filter.cycle_focus_back();
        }
        KeyCode::Tab => {
            app.filter.cycle_focus();
        }
        KeyCode::Up => {
            app.filter.select_prev();
        }
        KeyCode::Down => {
            app.filter.select_next();
        }
        KeyCode::Enter => {
            app.filter.add_to_focused();
        }
        KeyCode::Char('d')
            if key.modifiers == KeyModifiers::NONE && app.filter.input_buffer.is_empty() =>
        {
            app.filter.delete_selected_focused();
        }
        KeyCode::Char('D') if app.filter.input_buffer.is_empty() => {
            app.filter.clear_focused();
        }
        KeyCode::Char(c) => {
            app.filter.input_buffer.push(c);
        }
        KeyCode::Backspace => {
            app.filter.input_buffer.pop();
        }
        _ => {}
    }
}
