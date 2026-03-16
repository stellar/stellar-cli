use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
    Frame,
};

use crate::commands::watch::app::App;
use crate::commands::watch::event::EventKind;
use crate::commands::watch::spec_cache;

#[allow(clippy::too_many_lines)]
pub fn render(frame: &mut Frame, app: &App) {
    let Some(event) = &app.detail_event else {
        return;
    };

    let area = frame.area();
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .title(" Event Detail (Esc/q/Enter to close  j/k to scroll) ")
        .title_alignment(Alignment::Center);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    let val_width = inner.width.saturating_sub(21) as usize; // 20 key col + 1 space

    let label = |key: &str, val: &str| -> Vec<Line<'static>> {
        let key_span = Span::styled(format!("{key:<20} "), Style::default().fg(Color::Cyan));
        let indent = || Span::styled(format!("{:<21}", ""), Style::default());

        let mut result: Vec<Line<'static>> = Vec::new();
        let mut is_first = true;

        let source_lines: Vec<&str> = {
            let mut v: Vec<&str> = val.lines().collect();
            if v.is_empty() {
                v.push("");
            }
            v
        };

        for src in source_lines {
            let chunks: Vec<String> = if val_width == 0 || src.len() <= val_width {
                vec![src.to_string()]
            } else {
                src.chars()
                    .collect::<Vec<_>>()
                    .chunks(val_width)
                    .map(|c| c.iter().collect())
                    .collect()
            };
            for chunk in chunks {
                let val_span = Span::styled(chunk, Style::default().fg(Color::White));
                if is_first {
                    result.push(Line::from(vec![key_span.clone(), val_span]));
                    is_first = false;
                } else {
                    result.push(Line::from(vec![indent(), val_span]));
                }
            }
        }
        result
    };

    lines.extend(label(
        "Time",
        &event.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    ));

    match &event.kind {
        EventKind::Event(d) => {
            lines.extend(label("Type", "Event"));
            lines.extend(label("Event ID", &d.event_id));
            lines.extend(label("Contract ID", &d.contract_id));
            lines.extend(label("TX Hash", &d.tx_hash));
            lines.extend(label("Ledger", &d.ledger.to_string()));

            if let Some(decoded) = spec_cache::decode_event(
                &d.contract_id,
                &d.raw_topics,
                &d.raw_value,
                &app.spec_cache,
            ) {
                let event_label = if decoded.prefix_topics.is_empty() {
                    decoded.event_name.clone()
                } else {
                    format!(
                        "{} ({})",
                        decoded.event_name,
                        decoded.prefix_topics.join(", ")
                    )
                };
                lines.extend(label("Event", &event_label));
                for (name, value) in &decoded.params {
                    lines.extend(label(name, &json_display(value)));
                }
            } else {
                lines.extend(label("Event Type", &d.event_type));
                for (i, topic) in d.topics.iter().enumerate() {
                    lines.extend(label(&format!("Topic[{i}]"), &pretty_str(&topic.display)));
                }
                lines.extend(label("Value", &pretty_str(&d.value.display)));
            }

            if !d.tx_hash.is_empty() {
                if let Some(url) = explorer_tx_url(&app.network, &d.tx_hash) {
                    lines.extend(label("Transaction link", &url));
                }
            }
        }
        EventKind::Transaction(d) => {
            lines.extend(label("Type", "Transaction"));
            lines.extend(label("TX Hash", &d.tx_hash));
            lines.extend(label("Ledger", &d.ledger.to_string()));
            lines.extend(label("Status", &d.status));
            lines.extend(label("Source Account", d.source_account.as_str()));
            lines.extend(label("Fee Charged", &d.fee_charged.to_string()));
            lines.extend(label("Operation Count", &d.operation_count.to_string()));
            lines.extend(label("Operations", &d.operation_types.join(", ")));
            if let Some(url) = explorer_tx_url(&app.network, &d.tx_hash) {
                lines.extend(label("Transaction link", &url));
            }
        }
    }

    let paragraph = Paragraph::new(lines).scroll((app.detail_scroll, 0));

    frame.render_widget(paragraph, inner);
}

/// Format a `serde_json::Value` for display: strings without quotes, everything
/// else as pretty-printed JSON.
fn json_display(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

/// Try to parse a string as JSON and pretty-print it; strings are unquoted.
/// Falls back to the original string if it's not valid JSON.
fn pretty_str(s: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(v) => json_display(&v),
        Err(_) => s.to_string(),
    }
}

fn explorer_tx_url(network: &str, tx_hash: &str) -> Option<String> {
    match network {
        "mainnet" => Some(format!("https://stellar.expert/explorer/public/{tx_hash}")),
        "testnet" => Some(format!(
            "https://stellar.expert/explorer/testnet/tx/{tx_hash}"
        )),
        _ => None,
    }
}
