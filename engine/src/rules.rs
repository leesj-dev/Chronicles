//! Published rates that the pinned tokscale-core does not apply. This file, `events.rs` and `report.rs`
//! are kept byte-identical in Chronicles and CrossBar, so both apps count and price the same way.
//!
//! Costs a provider recorded itself are never changed. Sources, read 2026-11-24:
//! - <https://developers.openai.com/api/docs/pricing>: Fast is 2.5x for GPT-5.5 and 2x for the rest;
//!   Ultrafast is 6x for GPT-6 Astra and GPT-6.1 Sol, and for GPT-6.2 Astra and Sol 3x through
//!   February 28, 2027 (Pacific) and 6x after. The long-context rates scale with them.
//! - <https://help.openai.com/en/articles/20001106-codex-rate-card>: Codex auto-review is free when signed
//!   in with ChatGPT. Announced October 6, 2026; logs do not say how the user signed in, and no model or
//!   API price is published for it. Earlier requests keep tokscale's own partial estimate.
//! - <https://platform.claude.com/docs/en/build-with-claude/fast-mode>: Claude fast mode, which prompt
//!   caching multipliers apply on top of. Opus 4.6 and 4.7 had it at $30/$150, six times their list price.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use memchr::memmem;
use serde_json::Value;
use tokscale_core::sessions::{CostSource, EstimateStatus};
use tokscale_core::UnifiedMessage;

/// 2026-10-06T00:00:00Z. The announcement gives a date, not a time.
const AUTO_REVIEW_FREE_SINCE: i64 = 1_791_244_800_000;
/// 2027-03-01T00:00:00-08:00.
const GPT_6_2_ULTRAFAST_PROMOTION_END: i64 = 1_803_888_000_000;

/// tokscale reports any priority or fast request at 2x and any other tier at 1x.
fn openai_factor(model: &str, tier: &str, timestamp: i64) -> f64 {
    match (tier, model) {
        ("priority" | "fast", "gpt-5.5") => 2.5 / 2.0,
        ("ultrafast", "gpt-6-astra" | "gpt-6.1-sol") => 6.0,
        ("ultrafast", "gpt-6.2-astra" | "gpt-6.2-sol") => {
            if timestamp < GPT_6_2_ULTRAFAST_PROMOTION_END {
                3.0
            } else {
                6.0
            }
        }
        _ => 1.0,
    }
}

fn claude_fast_factor(model: &str) -> Option<f64> {
    match model {
        "claude-fable-5-2" | "claude-mythos-5-2" | "claude-opus-5-5" | "claude-opus-5"
        | "claude-opus-4-8" => Some(2.0),
        "claude-opus-4-7" | "claude-opus-4-6" => Some(6.0),
        _ => None,
    }
}

/// Applies the rules to messages tokscale has already priced. `fast` holds the dedup keys of Claude Code's
/// fast-mode requests, from [`claude_fast_requests`].
pub fn apply(messages: &mut [UnifiedMessage], fast: &HashSet<String>) {
    for message in messages {
        if message.cost_source == CostSource::ProviderReported
            || message.model_attribution_conflicted
            || message.estimate_status == EstimateStatus::Authoritative
        {
            continue;
        }
        if &*message.client == "codex"
            && &*message.model_id == "codex-auto-review"
            && message.timestamp >= AUTO_REVIEW_FREE_SINCE
        {
            message.cost = 0.0;
            message.estimate_status = EstimateStatus::CurrentEstimate;
            continue;
        }
        if !matches!(
            message.estimate_status,
            EstimateStatus::CurrentEstimate | EstimateStatus::PartialEstimate
        ) {
            continue;
        }
        if &*message.client == "codex" && &*message.provider_id == "openai" {
            if let Some(tier) = &message.service_tier {
                message.cost *= openai_factor(&message.model_id, tier, message.timestamp);
            }
        } else if &*message.client == "claude"
            && message
                .dedup_key
                .as_ref()
                .is_some_and(|key| fast.contains(key))
        {
            match claude_fast_factor(&message.model_id) {
                Some(factor) => message.cost *= factor,
                // A fast request of a model without a published fast rate has no known price.
                None => message.estimate_status = EstimateStatus::Unpriced,
            }
        }
    }
}

/// The `message id:request id` of every Claude Code request that ran in fast mode. tokscale does not read the
/// log's `speed` field, so this looks for it; the files are only searched as bytes unless they contain it.
pub fn claude_fast_requests(home: Option<&Path>) -> HashSet<String> {
    // As in tokscale, naming a home ignores the environment.
    let config = match home {
        Some(home) => Some(home.join(".claude")),
        None => std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".claude"))),
    };
    let Some(root) = config.map(|config| config.join("projects")) else {
        return HashSet::new();
    };
    let mut files = Vec::new();
    collect_jsonl(&root, &mut files, &mut HashSet::new());
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut keys = HashSet::new();
                    while let Some(path) =
                        files.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                    {
                        if let Ok(bytes) = std::fs::read(path) {
                            fast_keys(&bytes, &mut keys);
                        }
                    }
                    keys
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

fn collect_jsonl(dir: &Path, files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    let Ok(real) = dir.canonicalize() else { return };
    if !seen.insert(real) {
        return;
    }
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, files, seen);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            files.push(path);
        }
    }
}

fn fast_keys(bytes: &[u8], keys: &mut HashSet<String>) {
    const SPEED: &[u8] = b"\"speed\":\"fast\"";
    if memmem::find(bytes, SPEED).is_none() {
        return;
    }
    for line in bytes.split(|byte| *byte == b'\n') {
        if memmem::find(line, SPEED).is_none() {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let message = &record["message"];
        if message["usage"]["speed"] != "fast" {
            continue;
        }
        if let Some(id) = message["id"].as_str() {
            keys.insert(match record["requestId"].as_str() {
                Some(request) => format!("{id}:{request}"),
                None => format!("message:{id}"),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(client: &str, model: &str, provider: &str, timestamp: i64) -> UnifiedMessage {
        let mut message = UnifiedMessage::new(
            client,
            model,
            provider,
            "session",
            timestamp,
            tokscale_core::TokenBreakdown::default(),
            1.0,
        );
        message.cost_source = CostSource::default();
        message.estimate_status = EstimateStatus::CurrentEstimate;
        message
    }

    fn cost(mut message: UnifiedMessage, fast: &[&str]) -> f64 {
        let fast = fast.iter().map(|key| key.to_string()).collect();
        apply(std::slice::from_mut(&mut message), &fast);
        message.cost
    }

    fn tier(model: &str, tier: &str, timestamp: i64) -> f64 {
        let mut message = message("codex", model, "openai", timestamp);
        message.service_tier = Some(tier.to_string().into());
        cost(message, &[])
    }

    #[test]
    fn openai_tiers_follow_the_published_multipliers() {
        // The priority 2x is already in tokscale's cost.
        assert_eq!(tier("gpt-5.5", "priority", 0), 1.25);
        assert_eq!(tier("gpt-6.1-sol", "priority", 0), 1.0);
        assert_eq!(tier("gpt-6.1-sol", "default", 0), 1.0);
        assert_eq!(tier("gpt-6-astra", "ultrafast", 0), 6.0);
        assert_eq!(tier("gpt-6.1-sol", "ultrafast", 0), 6.0);
        assert_eq!(tier("gpt-6-luna", "ultrafast", 0), 1.0);
        for model in ["gpt-6.2-sol", "gpt-6.2-astra"] {
            assert_eq!(
                tier(model, "ultrafast", GPT_6_2_ULTRAFAST_PROMOTION_END - 1),
                3.0
            );
            assert_eq!(
                tier(model, "ultrafast", GPT_6_2_ULTRAFAST_PROMOTION_END),
                6.0
            );
        }
    }

    #[test]
    fn the_dates_are_the_published_ones() {
        let millis = |text: &str| {
            chrono::DateTime::parse_from_rfc3339(text)
                .unwrap()
                .timestamp_millis()
        };
        assert_eq!(AUTO_REVIEW_FREE_SINCE, millis("2026-10-06T00:00:00Z"));
        assert_eq!(
            GPT_6_2_ULTRAFAST_PROMOTION_END,
            millis("2027-03-01T00:00:00-08:00")
        );
    }

    #[test]
    fn auto_review_is_free_from_its_announcement() {
        let at = |timestamp| message("codex", "codex-auto-review", "openai", timestamp);
        assert_eq!(cost(at(AUTO_REVIEW_FREE_SINCE), &[]), 0.0);
        assert_eq!(cost(at(AUTO_REVIEW_FREE_SINCE - 1), &[]), 1.0);
        let mut unpriced = at(AUTO_REVIEW_FREE_SINCE);
        unpriced.estimate_status = EstimateStatus::Unpriced;
        apply(std::slice::from_mut(&mut unpriced), &HashSet::new());
        assert_eq!(unpriced.estimate_status, EstimateStatus::CurrentEstimate);
    }

    #[test]
    fn claude_fast_requests_cost_the_fast_rate() {
        let with_key = |model: &str| {
            let mut message = message("claude", model, "anthropic", 0);
            message.dedup_key = Some("msg:req".into());
            message
        };
        assert_eq!(cost(with_key("claude-opus-5-5"), &["msg:req"]), 2.0);
        assert_eq!(cost(with_key("claude-opus-4-7"), &["msg:req"]), 6.0);
        assert_eq!(cost(with_key("claude-opus-5-5"), &["other:req"]), 1.0);
        let mut unknown = with_key("claude-sonnet-5-5");
        apply(
            std::slice::from_mut(&mut unknown),
            &HashSet::from(["msg:req".to_string()]),
        );
        assert_eq!(unknown.estimate_status, EstimateStatus::Unpriced);
    }

    #[test]
    fn recorded_and_unpriced_costs_are_left_alone() {
        let mut reported = message("codex", "gpt-6-astra", "openai", 0);
        reported.service_tier = Some("ultrafast".to_string().into());
        reported.cost_source = CostSource::ProviderReported;
        assert_eq!(cost(reported, &[]), 1.0);
        let mut unpriced = message("codex", "gpt-6-astra", "openai", 0);
        unpriced.service_tier = Some("ultrafast".to_string().into());
        unpriced.estimate_status = EstimateStatus::Unpriced;
        assert_eq!(cost(unpriced, &[]), 1.0);
    }

    #[test]
    fn fast_keys_need_the_speed_of_the_usage_itself() {
        let mut keys = HashSet::new();
        fast_keys(
            br#"{"requestId":"r1","message":{"id":"m1","usage":{"speed":"fast"}}}
{"requestId":"r2","message":{"id":"m2","usage":{"speed":"standard"}}}
{"requestId":"r3","message":{"id":"m3","usage":{},"note":{"speed":"fast"}}}
{"message":{"id":"m4","usage":{"speed":"fast"}}}
broken "speed":"fast""#,
            &mut keys,
        );
        assert_eq!(
            keys,
            HashSet::from(["m1:r1".to_string(), "message:m4".to_string()])
        );
    }
}
