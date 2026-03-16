use std::collections::{HashSet, VecDeque};

use ratatui::widgets::ListState;

use crate::commands::watch::event::{AppEvent, ConnectionStatus, EventKind};
use crate::commands::watch::filter::FilterState;
use crate::commands::watch::spec_cache::SpecCache;

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    All,
    Events,
    Transactions,
}

impl Tab {
    pub fn label(&self) -> &str {
        match self {
            Tab::All => "All Items",
            Tab::Events => "Events",
            Tab::Transactions => "Transactions",
        }
    }

    pub fn next(&self) -> Tab {
        match self {
            Tab::All => Tab::Events,
            Tab::Events => Tab::Transactions,
            Tab::Transactions => Tab::All,
        }
    }

    pub fn prev(&self) -> Tab {
        match self {
            Tab::All => Tab::Transactions,
            Tab::Events => Tab::All,
            Tab::Transactions => Tab::Events,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    pub events: VecDeque<AppEvent>,
    pub max_events: usize,
    pub filter: FilterState,
    pub active_tab: Tab,
    pub list_state: ListState,
    pub auto_scroll: bool,
    pub network: String,
    pub network_passphrase: String,
    pub is_loading_older: bool,
    pub has_more_history: bool,
    pub rpc_status: ConnectionStatus,
    pub detail_event: Option<AppEvent>,
    pub detail_scroll: u16,
    pub show_help: bool,
    pub should_quit: bool,
    pub spec_cache: SpecCache,
    next_id: u64,
    seen_ids: HashSet<String>,
}

impl App {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: VecDeque::new(),
            max_events,
            filter: FilterState::default(),
            active_tab: Tab::All,
            list_state: ListState::default(),
            network: String::new(),
            network_passphrase: String::new(),
            auto_scroll: true,
            is_loading_older: false,
            has_more_history: true,
            rpc_status: ConnectionStatus::Connecting,
            detail_event: None,
            detail_scroll: 0,
            show_help: false,
            should_quit: false,
            spec_cache: SpecCache::new(),
            next_id: 0,
            seen_ids: HashSet::new(),
        }
    }

    fn event_unique_key(event: &AppEvent) -> String {
        match &event.kind {
            EventKind::Event(d) => format!("evt:{}", d.event_id),
            EventKind::Transaction(d) => format!("tx:{}", d.tx_hash),
        }
    }

    /// Pop the oldest event and remove its ID from `seen_ids`.
    fn pop_evicting(&mut self) {
        if let Some(evicted) = self.events.pop_back() {
            self.seen_ids.remove(&Self::event_unique_key(&evicted));
        }
    }

    pub fn push_event(&mut self, mut event: AppEvent) {
        let key = Self::event_unique_key(&event);
        if !self.seen_ids.insert(key) {
            return;
        }

        event.id = self.next_id;
        self.next_id += 1;

        let will_be_visible =
            self.tab_matches(&event) && self.filter.matches(&event, &self.network_passphrase);

        self.events.push_front(event);
        // Only prune while in live view. When the user is browsing old entries
        // (auto_scroll == false) we keep everything so loaded history isn't evicted.
        if self.auto_scroll {
            while self.events.len() > self.max_events {
                self.pop_evicting();
            }
        }

        if self.auto_scroll {
            self.list_state.select(Some(0));
        } else if will_be_visible {
            if let Some(idx) = self.list_state.selected() {
                if idx > 0 {
                    self.list_state.select(Some(idx + 1));
                }
            }
        }
    }

    pub fn visible_events(&self) -> Vec<&AppEvent> {
        self.events
            .iter()
            .filter(|e| self.tab_matches(e) && self.filter.matches(e, &self.network_passphrase))
            .collect()
    }

    /// Counts (all, events, transactions) matching the current filter, ignoring the active tab.
    pub fn tab_counts(&self) -> (usize, usize, usize) {
        let mut all = 0;
        let mut events = 0;
        let mut transactions = 0;
        for e in &self.events {
            if self.filter.matches(e, &self.network_passphrase) {
                all += 1;
                match e.kind {
                    EventKind::Event(_) => events += 1,
                    EventKind::Transaction(_) => transactions += 1,
                }
            }
        }
        (all, events, transactions)
    }

    fn tab_matches(&self, event: &AppEvent) -> bool {
        match self.active_tab {
            Tab::All => true,
            Tab::Events => matches!(event.kind, EventKind::Event(_)),
            Tab::Transactions => matches!(event.kind, EventKind::Transaction(_)),
        }
    }

    pub fn scroll_down(&mut self) {
        let visible = self.visible_events();
        if visible.is_empty() {
            return;
        }
        let max = visible.len() - 1;
        let next = match self.list_state.selected() {
            Some(i) => (i + 1).min(max),
            None => 0,
        };
        self.list_state.select(Some(next));
        self.auto_scroll = false;
    }

    pub fn scroll_up(&mut self) {
        let next = match self.list_state.selected() {
            Some(0) | None => 0,
            Some(i) => i - 1,
        };
        self.list_state.select(Some(next));
        // Don't auto-enable live view when reaching the top — show a hint instead.
    }

    pub fn jump_top(&mut self) {
        self.list_state.select(Some(0));
    }

    fn enable_live_view(&mut self) {
        self.auto_scroll = true;
        while self.events.len() > self.max_events {
            self.pop_evicting();
        }
    }

    pub fn jump_bottom(&mut self) {
        self.auto_scroll = false;
        let visible = self.visible_events();
        if !visible.is_empty() {
            self.list_state.select(Some(visible.len() - 1));
        }
    }

    pub fn see_latest(&mut self) {
        self.enable_live_view();
        self.list_state.select(Some(0));
    }

    pub fn oldest_fetched_ledger(&self) -> Option<u32> {
        self.events.iter().map(|e| e.kind.ledger()).min()
    }

    pub fn prepend_events(&mut self, events: Vec<AppEvent>, oldest_available: u32) {
        let old_visible_len = self.visible_events().len();

        let mut events = events;
        events.sort_by(|a, b| b.kind.ledger().cmp(&a.kind.ledger()));

        for mut event in events {
            let key = Self::event_unique_key(&event);
            if !self.seen_ids.insert(key) {
                continue;
            }
            event.id = self.next_id;
            self.next_id += 1;
            self.events.push_back(event);
        }

        // Allow up to 4× max_events while browsing history; pruned back to
        // max_events when the user returns to live view (see enable_live_view).
        let limit = self.max_events * 4;
        while self.events.len() > limit {
            self.pop_evicting();
        }

        if let Some(oldest_in_deque) = self.oldest_fetched_ledger() {
            self.has_more_history = oldest_available < oldest_in_deque;
        }

        self.is_loading_older = false;
        self.auto_scroll = false;

        let new_visible_len = self.visible_events().len();
        if new_visible_len > old_visible_len {
            self.list_state.select(Some(old_visible_len));
        }
    }

    pub fn open_detail(&mut self) {
        if self.filter.panel_open {
            return;
        }
        let visible = self.visible_events();
        if let Some(idx) = self.list_state.selected() {
            if let Some(event) = visible.get(idx) {
                self.detail_event = Some((*event).clone());
                self.detail_scroll = 0;
            }
        }
    }

    pub fn close_popup(&mut self) {
        if self.detail_event.is_some() {
            self.detail_event = None;
            self.detail_scroll = 0;
        } else if self.filter.panel_open {
            self.filter.panel_open = false;
        }
    }

    pub fn toggle_filter_panel(&mut self) {
        self.filter.panel_open = !self.filter.panel_open;
    }

    pub fn switch_tab(&mut self, forward: bool) {
        self.active_tab = if forward {
            self.active_tab.next()
        } else {
            self.active_tab.prev()
        };
        let visible = self.visible_events();
        if visible.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }
}
