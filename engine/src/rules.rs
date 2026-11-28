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
//! - <https://api-docs.deepseek.com/quick_start/pricing> and <https://api-docs.deepseek.com/updates/>,
//!   read 2026-11-25: DeepSeek's own API costs half outside 01:00-04:00 and 06:00-10:00 UTC, Monday to
//!   Friday, and all day on Chinese public holidays, since August 16, 2026 at 16:00 UTC. LiteLLM lists
//!   only the peak rate and models.dev's API only the off-peak one.
//! - <https://github.com/NateScarlet/holiday-cn> (MIT), which transcribes the State Council's notices:
//!   the holidays.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{Datelike, Timelike, Weekday};
use memchr::memmem;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokscale_core::sessions::{CostSource, EstimateStatus};
use tokscale_core::UnifiedMessage;

/// 2026-10-06T00:00:00Z. The announcement gives a date, not a time.
const AUTO_REVIEW_FREE_SINCE: i64 = 1_791_244_800_000;
/// 2027-03-01T00:00:00-08:00.
const GPT_6_2_ULTRAFAST_PROMOTION_END: i64 = 1_803_888_000_000;

/// 2026-08-16T16:00:00Z.
const DEEPSEEK_PEAK_PRICING_SINCE: i64 = 1_786_896_000_000;
/// The weekdays China has off from then through 2027, in holiday-cn's 2026 and 2027 data. A later
/// year's holidays count as ordinary days until they are added.
#[rustfmt::skip]
const CHINESE_WEEKDAY_HOLIDAYS: [(i32, u32, u32); 24] = [
    (2026, 9, 25), (2026, 10, 1), (2026, 10, 2), (2026, 10, 5), (2026, 10, 6), (2026, 10, 7),
    (2027, 1, 1), (2027, 2, 5), (2027, 2, 8), (2027, 2, 9), (2027, 2, 10), (2027, 2, 11),
    (2027, 2, 12), (2027, 4, 5), (2027, 5, 3), (2027, 5, 4), (2027, 5, 5), (2027, 6, 9),
    (2027, 9, 15), (2027, 10, 1), (2027, 10, 4), (2027, 10, 5), (2027, 10, 6), (2027, 10, 7),
];

/// Dollars per million input, cache-read and output tokens at peak; off-peak is half of each.
fn deepseek_peak_rates(model: &str) -> Option<[f64; 3]> {
    match model {
        "deepseek-flash" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp" => {
            Some([0.30, 0.006, 1.20])
        }
        "deepseek-v4-pro" => Some([1.32, 0.044, 3.96]),
        _ => None,
    }
}

fn deepseek_peak_hour(timestamp: i64) -> bool {
    chrono::DateTime::from_timestamp_millis(timestamp).is_some_and(|time| {
        matches!(time.hour(), 1..=3 | 6..=9)
            && !matches!(time.weekday(), Weekday::Sat | Weekday::Sun)
            // Both windows fall on the same date in China (UTC+8).
            && !CHINESE_WEEKDAY_HOLIDAYS.contains(&(time.year(), time.month(), time.day()))
    })
}

/// What to multiply tokscale's cost by for the hour of the request. Its price sources hold either the
/// peak or the off-peak rate, so the factor depends on which one the cost was made from. A cost that
/// matches neither is left as it is.
fn deepseek_factor(message: &UnifiedMessage) -> f64 {
    if message.timestamp < DEEPSEEK_PEAK_PRICING_SINCE
        || !matches!(
            &*message.provider_id,
            "deepseek" | "deepseek-account" | "deepseek-official"
        )
    {
        return 1.0;
    }
    let Some([input, cache_read, output]) = deepseek_peak_rates(&message.model_id) else {
        return 1.0;
    };
    let tokens = &message.tokens;
    let at_peak = (tokens.input as f64 * input
        + tokens.cache_read as f64 * cache_read
        + (tokens.output + tokens.reasoning) as f64 * output)
        / 1e6;
    let is = |cost: f64| (message.cost - cost).abs() <= cost * 1e-6;
    match deepseek_peak_hour(message.timestamp) {
        false if is(at_peak) => 0.5,
        true if is(at_peak / 2.0) => 2.0,
        _ => 1.0,
    }
}

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
        message.cost *= deepseek_factor(message);
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
/// log's `speed` field, so this looks for it. A file is searched again only when it changed.
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
    let path = tokscale_core::paths::get_cache_dir().join("claude-fast-requests.json");
    let known = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let found = search(&files, &known);
    if found != known {
        // Without the cache the next call only searches again.
        let _ = save(&path, &found);
    }
    found.into_values().flat_map(|file| file.keys).collect()
}

/// What an unchanged file held (rename the cache file when `fast_keys` changes) when it was last searched.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
struct Searched {
    size: u64,
    /// Modification time, nanoseconds since 1970.
    modified: u64,
    keys: Vec<String>,
}

type SearchedFiles = BTreeMap<PathBuf, Searched>;

fn save(path: &Path, files: &SearchedFiles) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, serde_json::to_vec(files)?)
        .and_then(|()| std::fs::rename(&temporary, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&temporary);
        })
}

/// Searches the files that changed since `known`.
fn search(files: &[PathBuf], known: &SearchedFiles) -> SearchedFiles {
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut found = Vec::new();
                    while let Some(path) =
                        files.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                    {
                        if let Some(file) = search_file(path, known.get(path)) {
                            found.push((path.clone(), file));
                        }
                    }
                    found
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

fn search_file(path: &Path, known: Option<&Searched>) -> Option<Searched> {
    // Read before the contents, so a later write is searched next time.
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    let (size, modified) = (metadata.len(), modified.as_nanos() as u64);
    if let Some(known) = known.filter(|known| (known.size, known.modified) == (size, modified)) {
        return Some(known.clone());
    }
    let mut keys = HashSet::new();
    fast_keys(&std::fs::read(path).ok()?, &mut keys);
    let mut keys: Vec<_> = keys.into_iter().collect();
    keys.sort();
    Some(Searched {
        size,
        modified,
        keys,
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
        assert_eq!(DEEPSEEK_PEAK_PRICING_SINCE, millis("2026-08-16T16:00:00Z"));
        assert_eq!(
            GPT_6_2_ULTRAFAST_PROMOTION_END,
            millis("2027-03-01T00:00:00-08:00")
        );
    }

    /// The cost after the rules of one million tokens of each kind, priced by tokscale at `rates`.
    fn deepseek(model: &str, provider: &str, at: &str, rates: [f64; 3]) -> f64 {
        let timestamp = chrono::DateTime::parse_from_rfc3339(at)
            .unwrap()
            .timestamp_millis();
        let mut message = message("dsh", model, provider, timestamp);
        message.tokens.input = 1_000_000;
        message.tokens.cache_read = 1_000_000;
        message.tokens.output = 1_000_000;
        message.cost = rates.iter().sum();
        cost(message, &[])
    }

    const FLASH_PEAK: [f64; 3] = [0.30, 0.006, 1.20];
    const FLASH_OFF_PEAK: [f64; 3] = [0.15, 0.003, 0.60];

    fn about(actual: f64, rates: [f64; 3]) -> bool {
        (actual - rates.iter().sum::<f64>()).abs() < 1e-9
    }

    #[test]
    fn deepseek_costs_half_outside_peak_hours() {
        let flash = |at: &str| deepseek("deepseek-flash", "deepseek-account", at, FLASH_PEAK);
        // Tuesday, November 24, 2026. Peak is 01:00-04:00 and 06:00-10:00 UTC.
        for peak in ["01:00:00", "03:59:59", "06:00:00", "09:59:59"] {
            assert!(
                about(flash(&format!("2026-11-24T{peak}Z")), FLASH_PEAK),
                "{peak}"
            );
        }
        for off in ["00:59:59", "04:00:00", "05:59:59", "10:00:00", "23:00:00"] {
            assert!(
                about(flash(&format!("2026-11-24T{off}Z")), FLASH_OFF_PEAK),
                "{off}"
            );
        }
        // A Saturday, and the weekdays China has off for Mid-Autumn, National Day and 2027's Spring Festival.
        for day in ["2026-11-28", "2026-09-25", "2026-10-07", "2027-02-12"] {
            assert!(
                about(flash(&format!("{day}T02:00:00Z")), FLASH_OFF_PEAK),
                "{day}"
            );
        }
        // A Saturday China works to make up for a holiday is still a weekend.
        assert!(about(flash("2026-10-10T02:00:00Z"), FLASH_OFF_PEAK));
        // The Thursday after the holiday is an ordinary day again.
        assert!(about(flash("2026-10-08T02:00:00Z"), FLASH_PEAK));
    }

    #[test]
    fn deepseek_peak_pricing_starts_when_it_was_announced_to() {
        let flash = |at: &str| deepseek("deepseek-v4-flash", "deepseek", at, FLASH_PEAK);
        assert!(about(flash("2026-08-16T15:59:59Z"), FLASH_PEAK));
        assert!(about(flash("2026-08-16T16:00:00Z"), FLASH_OFF_PEAK));
    }

    #[test]
    fn deepseek_costs_move_only_from_a_published_rate() {
        let at = |time: &str, rates| deepseek("deepseek-flash", "deepseek-official", time, rates);
        let (off, peak) = ("2026-11-24T12:00:00Z", "2026-11-24T02:00:00Z");
        // A source that already holds the off-peak rate is not halved again, and is doubled at peak.
        assert!(about(at(off, FLASH_OFF_PEAK), FLASH_OFF_PEAK));
        assert!(about(at(peak, FLASH_OFF_PEAK), FLASH_PEAK));
        // A cost made from some other rate is left alone.
        assert!(about(at(off, [1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]));
        let pro = deepseek(
            "deepseek-v4-pro",
            "deepseek-account",
            off,
            [1.32, 0.044, 3.96],
        );
        assert!(about(pro, [0.66, 0.022, 1.98]));
        // Another host's price of the same model has no peak hours.
        assert!(about(
            deepseek("deepseek-v4-pro", "openrouter", off, [1.32, 0.044, 3.96]),
            [1.32, 0.044, 3.96]
        ));
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

    #[test]
    fn a_file_is_searched_again_only_when_it_changed() {
        let folder = std::env::temp_dir().join(format!("fast-requests-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let log = folder.join("a.jsonl");
        let line = |id: &str| {
            format!("{{\"requestId\":\"r\",\"message\":{{\"id\":\"{id}\",\"usage\":{{\"speed\":\"fast\"}}}}}}\n")
        };
        std::fs::write(&log, line("m1")).unwrap();
        let files = [log.clone(), folder.join("missing.jsonl")];
        let first = search(&files, &SearchedFiles::new());
        assert_eq!(first.len(), 1);
        assert_eq!(first[&log].keys, ["m1:r"]);

        let path = folder.join("cache").join("known.json");
        save(&path, &first).unwrap();
        let mut known: SearchedFiles =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(known == first);
        known.get_mut(&log).unwrap().keys = vec!["kept".into()];
        assert_eq!(search(&files, &known)[&log].keys, ["kept"]);

        std::fs::write(&log, line("m1") + &line("m2")).unwrap();
        assert_eq!(search(&files, &known)[&log].keys, ["m1:r", "m2:r"]);
        assert!(search(&[], &known).is_empty());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
