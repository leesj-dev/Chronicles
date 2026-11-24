//! Reads and prices local history with tokscale-core. Kept byte-identical in Chronicles and CrossBar.

use crate::rules;
use serde::Serialize;
use std::path::Path;
use tokscale_core::sessions::{CostSource, EstimateStatus};
use tokscale_core::{ClientId, LocalParseOptions};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Message {
    client: String,
    model: String,
    provider: String,
    session: String,
    dedup: Option<String>,
    timestamp: i64,
    input: i64,
    /// Includes reasoning, which tokscale counts apart from output.
    output: i64,
    cache_read: i64,
    cache_write: i64,
    cache_write_1h: i64,
    /// The part of `output` spent on reasoning.
    reasoning: i64,
    messages: i32,
    /// USD; null when the message could not be priced.
    cost: Option<f64>,
    cost_source: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub(crate) engine: &'static str,
    pub(crate) tokscale_core: &'static str,
    pub(crate) clients: Vec<Client>,
    pub(crate) messages: Vec<Message>,
}

#[derive(clap::Args, Clone, Default)]
pub struct Options {
    #[arg(long)]
    pub offline: bool,
    #[arg(long)]
    pub clients: Option<String>,
    #[arg(long)]
    exclude: Option<String>,
    #[arg(long)]
    home: Option<String>,
    #[arg(long)]
    pub list_clients: bool,
}

impl Options {
    pub fn collect(offline: bool) -> Self {
        Self {
            offline,
            ..Self::default()
        }
    }
}

fn list(value: Option<&str>) -> Option<Vec<&str>> {
    value.map(|v| {
        v.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    })
}

pub fn clients() -> Vec<Client> {
    ClientId::iter()
        .map(|c| Client {
            id: c.as_str(),
            name: c.display_name(),
        })
        .collect()
}

fn cost_of(message: &tokscale_core::UnifiedMessage) -> (Option<f64>, &'static str) {
    if message.cost_source == CostSource::ProviderReported {
        return (Some(message.cost), "reported");
    }
    match message.estimate_status {
        EstimateStatus::Unpriced | EstimateStatus::StaleEstimate => (None, "unpriced"),
        EstimateStatus::PartialEstimate => (Some(message.cost), "partial"),
        EstimateStatus::Authoritative => (Some(message.cost), "reported"),
        EstimateStatus::CurrentEstimate => (Some(message.cost), "estimated"),
    }
}

pub async fn scan(options: &Options) -> Result<Report, String> {
    if options.offline {
        tokscale_core::network::go_offline();
    }
    let all = clients();
    let selected = selected_clients(options, &all);
    let mut parsed = Vec::new();
    if !selected.is_empty() {
        parsed = tokscale_core::parse_local_unified_messages(LocalParseOptions {
            home_dir: options.home.clone(),
            clients: Some(selected),
            ..Default::default()
        })
        .await?;
        let home = options.home.as_deref().map(Path::new);
        rules::apply(&mut parsed, &rules::claude_fast_requests(home));
        // The synthetic client can discover a Cursor cache implicitly.
        parsed.retain(|m| m.client != "cursor");
    }
    let messages = parsed
        .iter()
        .map(|m| {
            let (cost, cost_source) = if m.model_attribution_conflicted {
                (None, "unpriced")
            } else {
                cost_of(m)
            };
            Message {
                client: m.client.to_string(),
                model: m.model_id.to_string(),
                provider: m.provider_id.to_string(),
                session: m.session_id.to_string(),
                dedup: m.dedup_key.clone(),
                timestamp: m.timestamp,
                input: m.tokens.input,
                output: m.tokens.output.saturating_add(m.tokens.reasoning),
                cache_read: m.tokens.cache_read,
                cache_write: m.tokens.cache_write,
                cache_write_1h: m.tokens.cache_write_1h,
                reasoning: m.tokens.reasoning,
                messages: m.message_count,
                cost,
                cost_source,
            }
        })
        .collect();
    Ok(Report {
        engine: env!("CARGO_PKG_VERSION"),
        tokscale_core: "4.23.1 (b265e83)",
        clients: all,
        messages,
    })
}

fn selected_clients(options: &Options, all: &[Client]) -> Vec<String> {
    let excluded = list(options.exclude.as_deref()).unwrap_or_default();
    // Cursor history is behind its private dashboard API, which tokscale
    // fetches in its CLI crate rather than in core.
    list(options.clients.as_deref())
        .unwrap_or_else(|| all.iter().map(|c| c.id).collect())
        .into_iter()
        .filter(|c| *c != "cursor" && !excluded.contains(c))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_clients_trim_and_exclude_cursor_and_explicit_exclusions() {
        let options = Options {
            clients: Some("claude, , cursor, codex, synthetic".into()),
            exclude: Some(" codex ".into()),
            ..Options::default()
        };
        assert_eq!(
            selected_clients(&options, &clients()),
            ["claude", "synthetic"]
        );
        assert!(!selected_clients(&Options::default(), &clients()).contains(&"cursor".into()));
    }
}
