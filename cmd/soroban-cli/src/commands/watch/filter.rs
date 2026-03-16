use crate::commands::watch::event::{AppEvent, EventKind};
use crate::utils::contract_id_hash_from_asset;
use crate::xdr::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, PublicKey, Uint256,
};

#[derive(Debug, Clone, PartialEq)]
pub enum PanelField {
    RowTypes,
    Addresses,
    Tokens,
    EventTypes,
}

#[derive(Debug, Clone)]
pub struct FilterState {
    pub addresses: Vec<String>,
    pub tokens: Vec<String>,
    pub event_types: Vec<String>,
    pub show_transactions: bool,
    pub show_events: bool,
    pub type_focus: usize,
    pub panel_open: bool,
    pub panel_focus: PanelField,
    pub input_buffer: String,
    /// Selected item index within the currently focused list section.
    pub list_selection: Option<usize>,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            addresses: Vec::new(),
            tokens: Vec::new(),
            event_types: Vec::new(),
            show_transactions: true,
            show_events: true,
            type_focus: 0,
            panel_open: false,
            panel_focus: PanelField::RowTypes,
            input_buffer: String::new(),
            list_selection: None,
        }
    }
}

impl FilterState {
    pub fn is_active(&self) -> bool {
        !self.show_transactions
            || !self.show_events
            || !self.addresses.is_empty()
            || !self.tokens.is_empty()
            || !self.event_types.is_empty()
    }

    pub fn toggle_focused_type(&mut self) {
        match self.type_focus {
            0 => self.show_transactions = !self.show_transactions,
            _ => self.show_events = !self.show_events,
        }
    }

    pub fn matches(&self, event: &AppEvent, network_passphrase: &str) -> bool {
        let kind_visible = match &event.kind {
            EventKind::Transaction(_) => self.show_transactions,
            EventKind::Event(_) => self.show_events,
        };
        if !kind_visible {
            return false;
        }

        if self.addresses.is_empty() && self.tokens.is_empty() && self.event_types.is_empty() {
            return true;
        }

        let addr_match = filter_matches(&self.addresses, |addr| event_matches_address(event, addr));
        let token_match = filter_matches(&self.tokens, |token| {
            event_matches_token(event, token, network_passphrase)
        });
        let type_match = filter_matches(&self.event_types, |et| {
            let label = event.kind.type_label().to_lowercase();
            label.contains(&et.to_lowercase())
        });

        addr_match && token_match && type_match
    }

    fn focused_list_mut(&mut self) -> Option<&mut Vec<String>> {
        match self.panel_focus {
            PanelField::RowTypes => None,
            PanelField::Addresses => Some(&mut self.addresses),
            PanelField::Tokens => Some(&mut self.tokens),
            PanelField::EventTypes => Some(&mut self.event_types),
        }
    }

    pub fn add_to_focused(&mut self) {
        let input = self.input_buffer.trim().to_string();
        if self.panel_focus == PanelField::RowTypes {
            self.toggle_focused_type();
            return;
        }
        if input.is_empty() {
            return;
        }
        if let Some(list) = self.focused_list_mut() {
            if !list.contains(&input) {
                list.push(input);
            }
        }
        self.input_buffer.clear();
        self.list_selection = None;
    }

    fn focused_list_len(&self) -> usize {
        match self.panel_focus {
            PanelField::RowTypes => 0,
            PanelField::Addresses => self.addresses.len(),
            PanelField::Tokens => self.tokens.len(),
            PanelField::EventTypes => self.event_types.len(),
        }
    }

    pub fn select_prev(&mut self) {
        let len = self.focused_list_len();
        if len == 0 {
            return;
        }
        self.list_selection = Some(match self.list_selection {
            None | Some(0) => 0,
            Some(i) => i - 1,
        });
    }

    pub fn select_next(&mut self) {
        let len = self.focused_list_len();
        if len == 0 {
            return;
        }
        self.list_selection = Some(match self.list_selection {
            None => 0,
            Some(i) => (i + 1).min(len - 1),
        });
    }

    /// Remove the selected item (or the last item if nothing is selected).
    pub fn delete_selected_focused(&mut self) {
        let selection = self.list_selection;
        let (idx, new_len) = {
            let Some(list) = self.focused_list_mut() else {
                return;
            };
            if list.is_empty() {
                return;
            }
            let idx = selection.unwrap_or(list.len() - 1).min(list.len() - 1);
            list.remove(idx);
            (idx, list.len())
        };
        self.list_selection = if new_len == 0 {
            None
        } else {
            Some(idx.min(new_len - 1))
        };
    }

    /// Clear all items in the focused section.
    pub fn clear_focused(&mut self) {
        let Some(list) = self.focused_list_mut() else {
            return;
        };
        list.clear();
        self.list_selection = None;
    }

    pub fn cycle_focus(&mut self) {
        self.panel_focus = match self.panel_focus {
            PanelField::RowTypes => PanelField::Addresses,
            PanelField::Addresses => PanelField::Tokens,
            PanelField::Tokens => PanelField::EventTypes,
            PanelField::EventTypes => PanelField::RowTypes,
        };
        self.input_buffer.clear();
        self.list_selection = None;
    }

    pub fn cycle_focus_back(&mut self) {
        self.panel_focus = match self.panel_focus {
            PanelField::RowTypes => PanelField::EventTypes,
            PanelField::Addresses => PanelField::RowTypes,
            PanelField::Tokens => PanelField::Addresses,
            PanelField::EventTypes => PanelField::Tokens,
        };
        self.input_buffer.clear();
        self.list_selection = None;
    }
}

/// Evaluate a list of filter terms that may include negation (`-` prefix).
/// - Positive terms (no `-`): at least one must match (OR). If there are no positive terms, passes.
/// - Negative terms (`-` prefix): none may match (AND NOT).
/// - If all terms are negative: event passes as long as no negative matches.
fn filter_matches<F>(terms: &[String], mut matches: F) -> bool
where
    F: FnMut(&str) -> bool,
{
    if terms.is_empty() {
        return true;
    }
    let mut has_positive = false;
    let mut positive_hit = false;
    let mut negative_hit = false;
    for term in terms {
        if let Some(neg) = term.strip_prefix('-') {
            if matches(neg) {
                negative_hit = true;
            }
        } else {
            has_positive = true;
            if matches(term.as_str()) {
                positive_hit = true;
            }
        }
    }
    (!has_positive || positive_hit) && !negative_hit
}

fn event_matches_address(event: &AppEvent, addr: &str) -> bool {
    let addr_lower = addr.to_lowercase();
    match &event.kind {
        EventKind::Event(d) => {
            d.contract_id.to_lowercase().contains(&addr_lower)
                || d.topics
                    .iter()
                    .any(|t| t.display.to_lowercase().contains(&addr_lower))
        }
        EventKind::Transaction(d) => d.source_account.to_lowercase().contains(&addr_lower),
    }
}

fn event_matches_token(event: &AppEvent, token: &str, network_passphrase: &str) -> bool {
    let token_lower = token.to_lowercase();
    let resolved = token_to_contract_id(token, network_passphrase);
    // For "CODE:ISSUER" tokens, also try substring matching on just the code part.
    let code_lower: Option<String> = token.split_once(':').map(|(code, _)| code.to_lowercase());
    match &event.kind {
        EventKind::Event(d) => {
            let contract_lower = d.contract_id.to_lowercase();
            let topic_match = |needle: &str| {
                d.topics
                    .iter()
                    .any(|t| t.display.to_lowercase().contains(needle))
                    || d.value.display.to_lowercase().contains(needle)
            };
            resolved.as_deref() == Some(d.contract_id.as_str())
                || contract_lower.contains(&token_lower)
                || topic_match(&token_lower)
                || code_lower
                    .as_deref()
                    .is_some_and(|code| contract_lower.contains(code) || topic_match(code))
        }
        EventKind::Transaction(d) => d
            .operation_types
            .iter()
            .any(|op| op.to_lowercase().contains(&token_lower)),
    }
}

/// Attempt to resolve a `CODE:ISSUER` or `native` token string to the
/// corresponding SAC contract ID (as a strkey string). Returns `None` if the
/// token doesn't look like a classic asset.
fn token_to_contract_id(token: &str, network_passphrase: &str) -> Option<String> {
    let asset = if token.eq_ignore_ascii_case("native") {
        Asset::Native
    } else {
        let (code_str, issuer_str) = token.split_once(':')?;
        let issuer_key = stellar_strkey::ed25519::PublicKey::from_string(issuer_str).ok()?;
        let issuer = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(issuer_key.0)));
        let bytes = code_str.as_bytes();
        match bytes.len() {
            1..=4 => {
                let mut code = [0u8; 4];
                code[..bytes.len()].copy_from_slice(bytes);
                Asset::CreditAlphanum4(AlphaNum4 {
                    asset_code: AssetCode4(code),
                    issuer,
                })
            }
            5..=12 => {
                let mut code = [0u8; 12];
                code[..bytes.len()].copy_from_slice(bytes);
                Asset::CreditAlphanum12(AlphaNum12 {
                    asset_code: AssetCode12(code),
                    issuer,
                })
            }
            _ => return None,
        }
    };
    Some(contract_id_hash_from_asset(&asset, network_passphrase).to_string())
}
