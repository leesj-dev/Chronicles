use crate::usage::{display, truthy};
use futures_util::future::join_all;
use serde_json::{json, Map, Value};
use std::time::{Duration, Instant};

const PROVIDERS: [&str; 11] = [
    "claude",
    "codex",
    "cursor",
    "antigravity",
    "copilot",
    "devin",
    "grok",
    "ollama",
    "opencode",
    "openrouter",
    "zai",
];
const MAX_BYTES: usize = 2_000_001;

pub struct Bridge {
    client: reqwest::Client,
    port: u16,
    cached: Option<(Instant, Vec<Value>)>,
}

impl Bridge {
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(2))
                .build()?,
            port: 6736,
            cached: None,
        })
    }

    async fn fetch(&self, provider: &str) -> Option<Vec<Value>> {
        let mut response = self
            .client
            .get(format!(
                "http://127.0.0.1:{}/v1/usage/{provider}",
                self.port
            ))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;
        let mut body = Vec::new();
        while body.len() < MAX_BYTES {
            let Some(chunk) = response.chunk().await.ok()? else {
                break;
            };
            body.extend_from_slice(&chunk[..chunk.len().min(MAX_BYTES - body.len())]);
        }
        Some(snapshots(serde_json::from_slice(&body).ok()?, provider))
    }

    pub async fn collect(&mut self) -> Vec<Value> {
        if let Some((updated, snapshots)) = &self.cached {
            if updated.elapsed() < Duration::from_secs(30) {
                return snapshots.clone();
            }
        }
        let snapshots: Vec<_> = join_all(PROVIDERS.map(|provider| self.fetch(provider)))
            .await
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        self.cached = Some((Instant::now(), snapshots.clone()));
        snapshots
    }
}

fn text(value: &Value, length: usize) -> String {
    display(value).chars().take(length).collect()
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_bool().map(|flag| f64::from(u8::from(flag))))
}

fn snapshots(raw: Value, provider: &str) -> Vec<Value> {
    let snapshots = match raw {
        Value::Array(values) => values,
        value => vec![value],
    };
    let mut result = Vec::new();
    for snapshot in snapshots {
        if !display(&snapshot["providerId"]).starts_with(provider) {
            continue;
        }
        let lines: Vec<_> = match snapshot.get("lines") {
            Some(Value::Array(lines)) => lines.iter().filter_map(safe_line).collect(),
            None => Vec::new(),
            _ => return Vec::new(),
        };
        result.push(json!({
            "provider": provider,
            "fetchedAt": snapshot.get("fetchedAt").map(|value| text(value, 40)).unwrap_or_default(),
            "plan": snapshot.get("plan").filter(|value| truthy(value)).map(|value| text(value, 80)).unwrap_or_default(),
            "lines": lines,
        }));
    }
    result
}

fn safe_line(line: &Value) -> Option<Value> {
    let kind = line["type"].as_str()?;
    if !matches!(kind, "text" | "progress" | "badge" | "barChart") || line["label"] == "Error" {
        return None;
    }
    let content = [line.get("value"), line.get("text")]
        .into_iter()
        .flatten()
        .find(|value| truthy(value))
        .map(display)
        .unwrap_or_default();
    if matches!(
        content.trim().to_lowercase().as_str(),
        "no data" | "no usage data" | "—"
    ) {
        return None;
    }
    let mut safe: Map<String, Value> = [
        "type", "label", "value", "text", "subtitle", "note", "resetsAt",
    ]
    .into_iter()
    .filter_map(|key| {
        Some((
            key.into(),
            json!(text(line.get(key).filter(|v| !v.is_null())?, 160)),
        ))
    })
    .collect();
    for key in ["used", "limit"] {
        if number(&line[key]).is_some() {
            safe.insert(key.into(), line[key].clone());
        }
    }
    if let Some(format) = line["format"].as_object() {
        let format: Map<String, Value> = format
            .iter()
            .filter(|(key, _)| matches!(key.as_str(), "kind" | "suffix"))
            .map(|(key, value)| (key.clone(), json!(text(value, 40))))
            .collect();
        safe.insert("format".into(), json!(format));
    }
    if let Some(points) = line["points"].as_array().filter(|_| kind == "barChart") {
        let points: Vec<_> = points.iter().take(366).filter(|point| number(&point["value"]).is_some_and(|v| v >= 0.0))
            .map(|point| json!({
                "label": point.get("label").map(|value| text(value, 40)).unwrap_or_default(),
                "value": point["value"],
                "valueLabel": point.get("valueLabel").map(|value| text(value, 80)).unwrap_or_default(),
            })).collect();
        safe.insert("points".into(), json!(points));
    }
    Some(Value::Object(safe))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Value {
        json!({"providerId":"ollama","plan":"Pro","fetchedAt":"2026-01-01T00:00:00Z",
            "lines": [
                {"type":"progress","label":"Weekly","used":30,"limit":100,"format":{"kind":"percent"}},
                {"type":"text","label":"Plan","value":"Pro"},
                {"type":"badge","label":"State","text":"Active"},
                {"type":"barChart","label":"Trend","points":[{"label":"Mon","value":10,"valueLabel":"10 tokens"}]},
                {"type":"secret","label":"ignored","value":"ignore"},
                {"type":"text","label":"Error","value":"ignored"},
                {"type":"badge","text":"No usage data"}
            ], "apiKey":"must-not-be-forwarded"})
    }

    #[test]
    fn legacy_and_array_snapshots_are_sanitized() {
        let legacy = snapshots(snapshot(), "ollama");
        assert_eq!(
            legacy,
            snapshots(json!([snapshot(), {"providerId":"other"}]), "ollama")
        );
        assert_eq!(legacy[0]["lines"].as_array().unwrap().len(), 4);
        assert!(!json!(legacy).to_string().contains("apiKey"));
        let report = crate::usage::aggregate(
            Default::default(),
            None,
            chrono_tz::UTC,
            legacy.clone(),
            "fixed",
        );
        assert_eq!(report["accountSnapshots"], json!(legacy));
        assert_eq!(report["rows"], json!([]));
    }

    #[test]
    fn bridge_uses_provider_path_and_caches_results() {
        use std::io::{BufRead, BufReader, Write};
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut bridge = Bridge::new().unwrap();
        bridge.port = server.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            for _ in PROVIDERS {
                let (mut stream, _) = server.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = BufReader::new(&stream);
                let mut line = String::new();
                request.read_line(&mut line).unwrap();
                let provider = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .strip_prefix("/v1/usage/")
                    .unwrap();
                assert!(PROVIDERS.contains(&provider));
                let body = if provider == "ollama" {
                    snapshot().to_string()
                } else {
                    "[]".into()
                };
                loop {
                    line.clear();
                    if request.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let first = runtime.block_on(bridge.collect());
        worker.join().unwrap();
        assert_eq!(first, snapshots(snapshot(), "ollama"));
        assert_eq!(runtime.block_on(bridge.collect()), first);
    }
}
