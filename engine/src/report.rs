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
    pub(crate) client: String,
    pub(crate) model: String,
    pub(crate) provider: String,
    pub(crate) session: String,
    pub(crate) dedup: Option<String>,
    pub(crate) timestamp: i64,
    pub(crate) input: i64,
    /// Includes reasoning, which tokscale counts apart from output.
    pub(crate) output: i64,
    pub(crate) cache_read: i64,
    pub(crate) cache_write: i64,
    pub(crate) cache_write_1h: i64,
    /// The part of `output` spent on reasoning.
    pub(crate) reasoning: i64,
    pub(crate) messages: i32,
    /// USD; null when the message could not be priced.
    pub(crate) cost: Option<f64>,
    pub(crate) cost_source: &'static str,
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
        let runtime = tokio::runtime::Handle::current();
        let scans: Vec<_> = groups(selected)
            .into_iter()
            .map(|clients| {
                let runtime = runtime.clone();
                let home_dir = options.home.clone();
                tokio::task::spawn_blocking(move || {
                    runtime.block_on(tokscale_core::parse_local_unified_messages(
                        LocalParseOptions {
                            home_dir,
                            clients: Some(clients),
                            ..Default::default()
                        },
                    ))
                })
            })
            .collect();
        for scan in scans {
            parsed.extend(scan.await.map_err(|error| error.to_string())??);
        }
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

/// Splits the clients into scans that run at the same time. tokscale reads one client after another, and
/// matches bb's rows against Claude's and Codex's and OpenClaw's against Codex's, so those stay together.
/// `synthetic` relabels other clients' messages and reads everything itself.
fn groups(selected: Vec<String>) -> Vec<Vec<String>> {
    let has = |id: &str| selected.iter().any(|client| client == id);
    if has("synthetic") {
        return vec![selected];
    }
    let (claude_apart, codex_apart) = (!has("bb"), !has("bb") && !has("openclaw"));
    let mut groups = vec![Vec::new(); 5];
    for client in selected {
        let group = match client.as_str() {
            "claude" if claude_apart => 0,
            "codex" if codex_apart => 1,
            "antigravity" | "antigravity-cli" | "antigravity-extension" => 2,
            "copilot" => 3,
            _ => 4,
        };
        groups[group].push(client);
    }
    groups.retain(|group| !group.is_empty());
    groups
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

    #[test]
    fn clients_tokscale_matches_against_each_other_are_scanned_together() {
        let split = |text: &str| -> Vec<String> {
            groups(text.split(',').map(String::from).collect())
                .iter()
                .map(|group| group.join(","))
                .collect()
        };
        assert_eq!(
            split("codex,grok,claude,antigravity,antigravity-cli"),
            ["claude", "codex", "antigravity,antigravity-cli", "grok"]
        );
        assert_eq!(split("claude,codex,openclaw"), ["claude", "codex,openclaw"]);
        assert_eq!(
            split("claude,codex,bb,copilot"),
            ["copilot", "claude,codex,bb"]
        );
        assert_eq!(split("claude,copilot,synthetic").len(), 1);
    }
}
