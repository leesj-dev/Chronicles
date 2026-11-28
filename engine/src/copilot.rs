//! Recover VS Code chat requests missing from the pinned tokscale parser.

use crate::report::{self, Message, Report};
use serde_json::Value;
use std::{
    cmp::Reverse,
    collections::HashSet,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use tokscale_core::{pricing::PricingService, ClientId, TokenBreakdown, UnifiedMessage};

pub async fn scan(
    options: &report::Options,
    home: Option<String>,
    exclude: Option<&str>,
) -> Result<Report, String> {
    let report = report::scan(options).await?;
    let client = ClientId::Copilot.as_str();
    let contains_copilot = |list: &str| list.split(',').any(|id| id.trim() == client);
    if !options.clients.as_deref().is_none_or(contains_copilot)
        || exclude.is_some_and(contains_copilot)
    {
        return Ok(report);
    }

    let home = tokscale_core::get_home_dir_string(&home)?;
    let (mut report, mut recovered) = tokio::task::spawn_blocking(move || {
        let recovered = recover(&report, Path::new(&home));
        (report, recovered)
    })
    .await
    .map_err(|error| error.to_string())?;

    if recovered.iter().any(has_tokens) {
        let pricing = load_pricing().await;
        for message in recovered.iter_mut().filter(|message| has_tokens(message)) {
            let Some(pricing) = pricing.as_ref() else {
                tokscale_core::pricing::warn_incomplete_pricing();
                break;
            };
            let usage = UnifiedMessage::new(
                &message.client,
                &message.model,
                &message.provider,
                &message.session,
                message.timestamp,
                TokenBreakdown {
                    input: message.input,
                    output: message.output,
                    ..Default::default()
                },
                0.0,
            );
            let estimate = pricing.estimate_message(&usage);
            if estimate.coverage {
                message.cost = Some(estimate.cost);
                message.cost_source = "estimated";
            } else if estimate.selected.is_some() {
                message.cost = Some(estimate.cost);
                message.cost_source = "partial";
            }
        }
    }
    report.messages.append(&mut recovered);
    Ok(report)
}

fn has_tokens(message: &Message) -> bool {
    message.input > 0 || message.output > 0
}

// Match tokscale-core's local pricing policy. report::scan sets offline mode
// before either scan uses the shared pricing service.
async fn load_pricing() -> Option<std::sync::Arc<PricingService>> {
    if !tokscale_core::network::is_offline()
        && std::env::var("TOKSCALE_PRICING_CACHE_ONLY")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
    {
        return PricingService::load_cached_any_age().map(std::sync::Arc::new);
    }
    PricingService::get_or_init()
        .await
        .ok()
        .or_else(|| PricingService::load_cached_any_age().map(std::sync::Arc::new))
}

fn recover(report: &Report, home: &Path) -> Vec<Message> {
    let client = ClientId::Copilot.as_str();
    let mut seen: HashSet<_> = report
        .messages
        .iter()
        .filter_map(|message| message.dedup.clone())
        .collect();
    // tokscale also suppresses VS Code observations already seen in another
    // Copilot source (for example, the OTel exporter).
    let existing: HashSet<_> = report
        .messages
        .iter()
        .filter(|message| message.client == client)
        .map(|message| (message.session.as_str(), message.timestamp))
        .collect();
    let mut recovered = Vec::new();
    for path in session_files(home) {
        let Some(session) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        recovered.extend(
            read_requests(&path)
                .iter()
                .filter_map(|request| recover_request(request, session, &existing, &mut seen)),
        );
    }
    recovered
}

fn entries(path: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
}

fn session_files(home: &Path) -> Vec<PathBuf> {
    // Same Code/User roots as tokscale's local report (use_env_roots=false).
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for empty_window in [false, true] {
        for root in [
            "Library/Application Support/Code/User",
            ".config/Code/User",
            "AppData/Roaming/Code/User",
        ] {
            let root = home.join(root);
            let directories: Vec<_> = if empty_window {
                vec![root.join("globalStorage/emptyWindowChatSessions")]
            } else {
                entries(&root.join("workspaceStorage"))
                    .map(|workspace| workspace.join("chatSessions"))
                    .collect()
            };
            for path in directories.iter().flat_map(|dir| entries(dir)) {
                if !matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("json" | "jsonl")
                ) || !path.is_file()
                {
                    continue;
                }
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen.insert(canonical) {
                    let modified = path.metadata().and_then(|metadata| metadata.modified());
                    files.push((empty_window, Reverse(modified.unwrap_or(UNIX_EPOCH)), path));
                }
            }
        }
    }
    // Within a session, workspace copies win, newest first. The original
    // empty-window file can still contribute requests missing from the copy.
    files.sort_unstable();
    files.into_iter().map(|(_, _, path)| path).collect()
}

fn read_requests(path: &Path) -> Vec<Value> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        let Ok(mut snapshot) = serde_json::from_reader::<_, Value>(reader) else {
            return Vec::new();
        };
        return snapshot
            .get_mut("requests")
            .map(take_requests)
            .unwrap_or_default();
    }

    let mut requests = Vec::new();
    for line in reader.split(b'\n').map_while(Result::ok) {
        let Ok(mut record) = serde_json::from_str::<Value>(&String::from_utf8_lossy(&line)) else {
            // A journal may have a partially written final line.
            continue;
        };
        if record["kind"].as_u64() == Some(0) {
            if let Some(snapshot) = record.get_mut("v").filter(|value| value.is_object()) {
                requests = snapshot
                    .get_mut("requests")
                    .map(take_requests)
                    .unwrap_or_default();
            }
            continue;
        }
        let value = record.get_mut("v").map(Value::take);
        let (Some(mut value), Some(path)) = (value, record["k"].as_array()) else {
            continue;
        };
        match (record["kind"].as_u64(), path.as_slice()) {
            (Some(1), [root]) if root == "requests" => requests = take_requests(&mut value),
            (Some(2), [root]) if root == "requests" => requests.extend(take_requests(&mut value)),
            (Some(1), [root, index, fields @ ..]) if root == "requests" => {
                let index = index.as_u64().and_then(|index| usize::try_from(index).ok());
                if let Some(request) = index.and_then(|index| requests.get_mut(index)) {
                    update_request(request, fields, value);
                }
            }
            _ => {}
        }
    }
    requests
}

fn take_requests(value: &mut Value) -> Vec<Value> {
    if let Value::Array(mut requests) = value.take() {
        requests.iter_mut().for_each(project_request);
        requests
    } else {
        Vec::new()
    }
}

fn token_field(field: &str) -> bool {
    matches!(field, "promptTokens" | "completionTokens")
}

fn metadata_field(field: &str) -> bool {
    matches!(
        field,
        "agentId" | "resolvedModel" | "promptTokens" | "outputTokens"
    )
}

fn request_field(field: &str) -> bool {
    matches!(field, "timestamp" | "modelId" | "result") || token_field(field)
}

// Retain only fields needed for counts, model and cost. Keep the keys of
// other result fields so their presence still distinguishes a nonempty result.
fn project_request(request: &mut Value) {
    let Some(fields) = request.as_object_mut() else {
        *request = Value::Null;
        return;
    };
    fields.retain(|key, _| request_field(key));
    if let Some(result) = fields.get_mut("result").and_then(Value::as_object_mut) {
        for (key, value) in result.iter_mut() {
            let retain = match key.as_str() {
                "metadata" => metadata_field,
                "usage" => token_field,
                _ => {
                    *value = Value::Null;
                    continue;
                }
            };
            if let Some(fields) = value.as_object_mut() {
                fields.retain(|key, _| retain(key));
            }
        }
    }
}

fn update_request(request: &mut Value, fields: &[Value], value: Value) {
    let (fields, value) = match fields {
        [] => (fields, value),
        [field] if field.as_str().is_some_and(request_field) => (fields, value),
        [field, key] if field == "timestamp" && key == "start" => (fields, value),
        [field, key] if field == "result" && key.is_string() => (fields, value),
        [field, section, key]
            if field == "result"
                && ((section == "metadata" && key.as_str().is_some_and(metadata_field))
                    || (section == "usage" && key.as_str().is_some_and(token_field))) =>
        {
            (fields, value)
        }
        // A write within discarded metadata still establishes a result.
        [field, section, ..] if field == "result" && section.is_string() => {
            let section = section.as_str().unwrap();
            if request["result"].get(section).is_some() {
                return;
            }
            (&fields[..2], Value::Null)
        }
        _ => return,
    };
    let mut target = &mut *request;
    for key in fields {
        if !target.is_object() {
            *target = Value::Object(Default::default());
        }
        target = target
            .as_object_mut()
            .unwrap()
            .entry(key.as_str().unwrap())
            .or_insert(Value::Null);
    }
    *target = value;
    project_request(request);
}

fn recover_request(
    request: &Value,
    session: &str,
    existing: &HashSet<(&str, i64)>,
    seen: &mut HashSet<String>,
) -> Option<Message> {
    let result = &request["result"];
    if result.as_object().is_none_or(|fields| fields.is_empty()) {
        return None;
    }
    let metadata = &result["metadata"];
    if let Some(agent) = metadata.get("agentId").filter(|agent| !agent.is_null()) {
        if !agent
            .as_str()
            .is_some_and(|id| id.starts_with("github.copilot"))
        {
            return None;
        }
    }
    let timestamp = request["timestamp"]
        .as_i64()
        .or_else(|| request["timestamp"]["start"].as_i64())?;
    if timestamp <= 0 || existing.contains(&(session, timestamp)) {
        return None;
    }
    let dedup = format!("copilot-vscode:{session}:{timestamp}");
    if !seen.insert(dedup.clone()) {
        return None;
    }

    let model = metadata["resolvedModel"]
        .as_str()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .or_else(|| {
            let model = request["modelId"].as_str()?.trim();
            (!model.is_empty()).then(|| {
                model
                    .strip_prefix("copilot/")
                    .or_else(|| model.strip_prefix("github.copilot-chat/"))
                    .unwrap_or(model)
            })
        })
        .unwrap_or("auto");
    let tokens = |field: &str, metadata_field: &str| {
        request[field]
            .as_i64()
            .or_else(|| metadata[metadata_field].as_i64())
            .filter(|count| *count > 0)
            .or_else(|| result["usage"][field].as_i64())
            .unwrap_or(0)
            .max(0)
    };
    Some(Message {
        client: ClientId::Copilot.as_str().into(),
        model: model.into(),
        provider: provider_hint(model).into(),
        session: session.into(),
        dedup: Some(dedup),
        timestamp,
        input: tokens("promptTokens", "promptTokens"),
        output: tokens("completionTokens", "outputTokens"),
        cache_read: 0,
        cache_write: 0,
        cache_write_1h: 0,
        reasoning: 0,
        messages: 1,
        cost: None,
        cost_source: "unpriced",
    })
}

// Mirror tokscale's private provider inference so price lookup gets the
// same upstream hint as a VS Code request emitted by its native parser.
fn provider_hint(model: &str) -> &'static str {
    let model = model.to_lowercase();
    let model = model.trim_start_matches("ollama/");
    let contains = |families: &[&str]| families.iter().any(|family| model.contains(family));
    let delimited = |families: &[&str], allow_version: bool| {
        families.iter().any(|family| {
            model.match_indices(family).any(|(start, matched)| {
                let end = start + matched.len();
                model[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| !ch.is_alphanumeric())
                    && model[end..].chars().next().is_none_or(|ch| {
                        !ch.is_alphanumeric() || (allow_version && ch.is_ascii_digit())
                    })
            })
        })
    };
    let providers = [
        (
            "anthropic",
            contains(&["claude", "anthropic"])
                || delimited(&["opus", "sonnet", "haiku", "fable"], true),
        ),
        (
            "openai",
            contains(&["gpt", "openai"]) || delimited(&["o1", "o3", "o4"], false),
        ),
        ("google", contains(&["gemini", "google"])),
        ("xai", contains(&["grok"])),
        (
            "deepseek",
            contains(&["deepseek"]) || delimited(&["dfmodel", "df_model", "df-model"], false),
        ),
        ("minimax", contains(&["minimax"])),
        (
            "mistral",
            contains(&["mistral", "mixtral", "codestral", "devstral"]),
        ),
        (
            "meta",
            contains(&["llama"]) || delimited(&["meta", "muse-spark"], true),
        ),
        (
            "qwen",
            contains(&["qwen"])
                || delimited(&["qmodel"], true)
                || delimited(
                    &[
                        "qmodelpreview",
                        "qmodellatest",
                        "qmodel38max",
                        "qfmodel",
                        "qf_model",
                        "qf-model",
                    ],
                    false,
                )
                || (model.starts_with("qmodel_") || model.starts_with("qmodel-")),
        ),
        ("sakana", contains(&["fugu"])),
        (
            "moonshotai",
            delimited(&["kimi"], true) || delimited(&["k2", "k3"], false),
        ),
        ("xiaomi", delimited(&["mimo"], true)),
        (
            "zai",
            delimited(&["glm"], true) || delimited(&["gfmodel", "gf_model", "gf-model"], false),
        ),
    ];
    providers
        .into_iter()
        .find_map(|(provider, matched)| matched.then_some(provider))
        .unwrap_or("github-copilot")
}
