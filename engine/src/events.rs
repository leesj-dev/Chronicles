//! Turns a report into events with the ids both apps use. Kept byte-identical in Chronicles and CrossBar.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, SecondsFormat, TimeDelta, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const FIELDS: [&str; 6] = [
    "input",
    "output",
    "cacheRead",
    "cacheWrite",
    "cacheWrite1h",
    "reasoning",
];
pub fn event_id(key: &Value) -> String {
    format!("{:x}", Sha256::digest(key.to_string().as_bytes()))
}

pub fn timestamp(value: &Value, milliseconds: bool) -> Option<DateTime<FixedOffset>> {
    if let Some(number) = value.as_f64().or_else(|| (value == true).then_some(1.0)) {
        let seconds = if milliseconds {
            number / 1000.0
        } else {
            number
        };
        if (0.0..253_402_300_800.0).contains(&seconds) && seconds != 0.0 {
            return DateTime::from_timestamp_micros(
                (seconds * 1_000_000.0).round_ties_even() as i64
            )
            .map(|time| time.fixed_offset());
        }
    }
    let text = value.as_str()?.replace('Z', "+00:00");
    let text = text.replace(' ', "T");
    let time = DateTime::parse_from_rfc3339(&text).ok().or_else(|| {
        NaiveDateTime::parse_from_str(&text, "%Y-%m-%dT%H:%M:%S%.f")
            .ok()
            .or_else(|| {
                NaiveDate::parse_from_str(&text, "%Y-%m-%d")
                    .ok()?
                    .and_hms_opt(0, 0, 0)
            })
            .map(|time| time.and_utc().fixed_offset())
    })?;
    // Python datetime keeps microseconds.
    let nanos = time.timestamp_subsec_nanos() % 1000;
    Some(time - TimeDelta::nanoseconds(i64::from(nanos)))
}

fn iso(time: DateTime<FixedOffset>) -> String {
    let precision = if time.timestamp_subsec_micros() == 0 {
        SecondsFormat::Secs
    } else {
        SecondsFormat::Micros
    };
    time.to_rfc3339_opts(precision, false)
}

pub fn now() -> String {
    iso(Utc::now().fixed_offset())
}

pub fn warnings(stderr: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    for line in String::from_utf8_lossy(stderr)
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("Warning:"))
    {
        let warning = format!("engine: {}", line.chars().take(500).collect::<String>());
        if !found.contains(&warning) {
            found.push(warning);
            if found.len() == 10 {
                break;
            }
        }
    }
    found
}

fn alias(client: &str) -> &str {
    match client {
        "antigravity-extension" | "antigravity-cli" => "antigravity",
        _ => client,
    }
}

pub fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        _ => value.to_string(),
    }
}

fn nonnegative_count(value: &Value, coerce: bool) -> Option<u64> {
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    if let Some(number) = value.as_i64() {
        return Some(number.max(0) as u64);
    }
    if let Some(flag) = value.as_bool() {
        return Some(u64::from(flag));
    }
    if coerce {
        if let Some(number) = value.as_f64() {
            return Some(number.max(0.0) as u64);
        }
        if let Some(number) = value.as_str() {
            return number
                .trim()
                .parse::<i64>()
                .ok()
                .map(|number| number.max(0) as u64);
        }
    }
    None
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(_) => value.as_f64() != Some(0.0),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

#[derive(Clone, Default, Serialize)]
pub struct Metrics {
    #[serde(flatten)]
    pub tokens: BTreeMap<&'static str, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priced: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

impl Metrics {
    pub fn from_record(record: &Value, engine: bool) -> Option<Self> {
        let mut tokens = BTreeMap::new();
        for field in FIELDS {
            let count = nonnegative_count(&record[field], engine);
            tokens.insert(field, if engine { count? } else { count.unwrap_or(0) });
        }
        let cost = (engine || truthy(&record["priced"]))
            .then(|| record["cost"].as_f64().map(|cost| cost.max(0.0)))
            .flatten();
        let priced =
            (engine || record.get("priced").is_some()).then_some(u64::from(cost.is_some()));
        Some(Self {
            tokens,
            priced,
            cost,
        })
    }

    pub fn merge_max(&mut self, other: Self) {
        for (metric, value) in other.tokens {
            let current = self.tokens.entry(metric).or_default();
            *current = (*current).max(value);
        }
        if let Some(value) = other.priced {
            self.priced = Some(self.priced.unwrap_or(0).max(value));
        }
        if let Some(value) = other.cost {
            self.cost = Some(self.cost.unwrap_or(0.0).max(value));
        }
    }
}

#[derive(Serialize)]
pub struct Event {
    pub id: String,
    pub provider: String,
    pub date: String,
    #[serde(serialize_with = "serialize_instant")]
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub model: String,
    #[serde(flatten)]
    pub metrics: Metrics,
    #[serde(skip)]
    pub remote: bool,
    /// Session id as the source names it; never part of an export.
    #[serde(skip)]
    pub session: String,
}

fn serialize_instant<S: serde::Serializer>(
    time: &Option<DateTime<FixedOffset>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    time.map(iso).serialize(serializer)
}

#[derive(Default)]
pub struct Scan {
    pub events: Events,
    pub names: BTreeMap<String, Value>,
    pub engine: Value,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct Events {
    pub ordered: Vec<Event>,
    pub indices: HashMap<String, usize>,
}

impl Events {
    pub fn insert(&mut self, event: Event) {
        if let Some(&i) = self.indices.get(&event.id) {
            let existing = &mut self.ordered[i];
            existing.metrics.merge_max(event.metrics);
            existing.timestamp = existing.timestamp.or(event.timestamp);
            existing.remote |= event.remote;
        } else {
            self.indices.insert(event.id.clone(), self.ordered.len());
            self.ordered.push(event);
        }
    }
}

pub fn convert(report: &Value, zone: Tz) -> Result<Scan, &'static str> {
    const INVALID: &str = "engine: output could not be read";
    let mut scan = Scan::default();
    for client in report["clients"].as_array().ok_or(INVALID)? {
        let id = client["id"].as_str().ok_or(INVALID)?;
        let name = client.get("name").ok_or(INVALID)?;
        if id == alias(id) {
            scan.names.insert(id.to_owned(), name.clone());
        }
    }
    scan.engine = json!({
        "version": report.get("engine").map(display).unwrap_or_default(),
        "tokscaleCore": report.get("tokscaleCore").map(display).unwrap_or_default(),
    });
    collect_events(
        &mut scan,
        report["messages"].as_array().ok_or(INVALID)?.iter(),
        zone,
    );
    Ok(scan)
}

pub fn convert_local(report: crate::report::Report, zone: Tz) -> Scan {
    let mut scan = Scan {
        names: report
            .clients
            .into_iter()
            .filter(|client| client.id == alias(client.id))
            .map(|client| (client.id.into(), json!(client.name)))
            .collect(),
        engine: json!({"version": report.engine, "tokscaleCore": report.tokscale_core}),
        ..Scan::default()
    };
    // Convert just one message at a time so large local reports never need a
    // second, fully materialized JSON representation in memory.
    collect_events(
        &mut scan,
        report.messages.into_iter().map(|message| json!(message)),
        zone,
    );
    scan
}

fn collect_events(
    scan: &mut Scan,
    messages: impl Iterator<Item = impl std::borrow::Borrow<Value>>,
    zone: Tz,
) {
    let mut seen = HashMap::new();
    for message in messages {
        if let Some(event) = local_event(message.borrow(), zone, &mut seen) {
            scan.events.insert(event);
        } else {
            scan.skipped += 1;
        }
    }
}

fn local_event(message: &Value, zone: Tz, seen: &mut HashMap<String, u64>) -> Option<Event> {
    let client = message["client"].as_str()?;
    let instant = timestamp(&message["timestamp"], true)?;
    let key = if message["dedup"].is_null() {
        let identity = [
            message.get("session")?,
            message.get("timestamp")?,
            message.get("model")?,
        ];
        let occurrence = seen
            .entry(serde_json::to_string(&identity).ok()?)
            .or_default();
        *occurrence += 1;
        json!([identity[0], identity[1], identity[2], occurrence])
    } else {
        message["dedup"].clone()
    };
    Some(Event {
        id: event_id(&json!(["engine", client, key])),
        provider: alias(client).to_owned(),
        date: instant.with_timezone(&zone).date_naive().to_string(),
        timestamp: Some(instant),
        model: message["model"].as_str()?.to_owned(),
        metrics: Metrics::from_record(message, true)?,
        remote: false,
        session: message["session"].as_str().unwrap_or("").to_owned(),
    })
}
