use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, SecondsFormat, TimeDelta, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

const FIELDS: [&str; 6] = [
    "input",
    "output",
    "cacheRead",
    "cacheWrite",
    "cacheWrite1h",
    "reasoning",
];
const CORE_FIELDS: [&str; 4] = ["input", "output", "cacheRead", "cacheWrite"];

pub fn timezone() -> Result<Tz, String> {
    resolve_timezone(
        std::env::var("CHRONICLES_TIMEZONE").ok().as_deref(),
        std::env::var("TZ").ok().as_deref(),
        std::fs::canonicalize("/etc/localtime")
            .ok()
            .as_deref()
            .and_then(Path::to_str),
        std::fs::read_to_string("/etc/timezone").ok().as_deref(),
    )
}

fn resolve_timezone(
    explicit: Option<&str>,
    tz: Option<&str>,
    localtime: Option<&str>,
    file: Option<&str>,
) -> Result<Tz, String> {
    if let Some(name) = explicit.filter(|name| !name.is_empty()) {
        return name
            .parse()
            .map_err(|_| format!("invalid CHRONICLES_TIMEZONE: {name}"));
    }
    let env_zone = tz.map(|name| {
        name.trim_start_matches(':')
            .rsplit("zoneinfo/")
            .next()
            .unwrap_or("")
    });
    let system_zone = localtime.and_then(|path| path.split_once("zoneinfo/").map(|(_, name)| name));
    Ok([env_zone, system_zone, file.map(str::trim)]
        .into_iter()
        .flatten()
        .find_map(|name| name.parse().ok())
        .unwrap_or(chrono_tz::UTC))
}

pub fn event_id(key: &Value) -> String {
    format!("{:x}", Sha256::digest(key.to_string().as_bytes()))
}

fn timestamp(value: &Value, milliseconds: bool) -> Option<DateTime<FixedOffset>> {
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
struct Metrics {
    #[serde(flatten)]
    tokens: BTreeMap<&'static str, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priced: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost: Option<f64>,
}

impl Metrics {
    fn from_record(record: &Value, engine: bool) -> Option<Self> {
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

    fn merge_max(&mut self, other: Self) {
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
struct Event {
    id: String,
    provider: String,
    date: String,
    #[serde(serialize_with = "serialize_instant")]
    timestamp: Option<DateTime<FixedOffset>>,
    model: String,
    #[serde(flatten)]
    metrics: Metrics,
    #[serde(skip)]
    remote: bool,
}

fn serialize_instant<S: serde::Serializer>(
    time: &Option<DateTime<FixedOffset>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    time.map(iso).serialize(serializer)
}

#[derive(Default)]
pub struct Scan {
    events: Events,
    names: BTreeMap<String, Value>,
    engine: Value,
    skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Default)]
struct Events {
    ordered: Vec<Event>,
    indices: HashMap<String, usize>,
}

impl Events {
    fn insert(&mut self, event: Event) {
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
    })
}

pub fn build_import(
    stdout: &[u8],
    stderr: &[u8],
    zone: Tz,
    updated_at: &str,
) -> Result<Value, &'static str> {
    let output = serde_json::from_slice(stdout)
        .ok()
        .and_then(|report| convert(&report, zone).ok())
        .ok_or("remote engine output could not be read")?;
    Ok(json!({
        "schema": "usage-atlas.events.v1",
        "updatedAt": updated_at,
        "events": output.events.ordered,
        "errors": warnings(stderr),
        "files": 0,
        "skipped": output.skipped,
        "engine": output.engine,
    }))
}

fn merge_import(scan: &mut Scan, import: &Value, zone: Tz) -> Option<()> {
    if let Some(errors) = import.get("errors") {
        match errors {
            Value::Array(errors) => scan.errors.extend(
                errors
                    .iter()
                    .map(|error| format!("SSH import: {}", display(error))),
            ),
            _ => return None,
        }
    }
    for record in import["events"].as_array()? {
        let id = record["id"].as_str()?;
        let instant = truthy(&record["timestamp"])
            .then(|| timestamp(&record["timestamp"], false))
            .flatten();
        let metrics = Metrics::from_record(record, false)?;
        if let Some(&i) = scan.events.indices.get(id) {
            let existing = &mut scan.events.ordered[i];
            existing.timestamp = existing.timestamp.or(instant);
            existing.metrics.merge_max(metrics);
            existing.remote = true;
        } else {
            let date = if let Some(time) = instant {
                time.with_timezone(&zone).date_naive().to_string()
            } else {
                record["date"].as_str()?.to_owned()
            };
            scan.events.insert(Event {
                id: id.to_owned(),
                provider: record["provider"].as_str()?.to_owned(),
                date,
                timestamp: instant,
                model: record["model"].as_str()?.to_owned(),
                metrics,
                remote: true,
            });
        }
    }
    Some(())
}

#[derive(Default)]
struct Row {
    tokens: BTreeMap<&'static str, u64>,
    requests: u64,
    cost_requests: Option<u64>,
    cost: f64,
}

impl Row {
    fn add(&mut self, metrics: &Metrics) {
        for (metric, count) in &metrics.tokens {
            *self.tokens.entry(metric).or_default() += count;
        }
        self.requests += 1;
        if let Some(priced) = metrics.priced {
            *self.cost_requests.get_or_insert(0) += priced;
        }
        self.cost += metrics.cost.unwrap_or(0.0);
    }

    fn json(self, (provider, date, model, device): RowKey) -> Value {
        let mut row = json!(self.tokens);
        for field in CORE_FIELDS {
            row.as_object_mut()
                .unwrap()
                .entry(field)
                .or_insert(json!(0));
        }
        row["total"] = json!(CORE_FIELDS
            .iter()
            .map(|field| row[field].as_u64().unwrap_or(0))
            .sum::<u64>());
        row["provider"] = json!(provider);
        row["date"] = json!(date);
        row["model"] = json!(model);
        row["device"] = json!(device);
        row["requests"] = json!(self.requests);
        if let Some(cost_requests) = self.cost_requests {
            row["costRequests"] = json!(cost_requests);
            row["cost"] = json!(round_cost(self.cost));
        }
        row
    }
}

pub fn round_cost(cost: f64) -> f64 {
    // Decimal formatting rounds the original binary float, including ties,
    // like Python round(cost, 6). Scaling first can introduce a false tie.
    format!("{cost:.6}").parse().unwrap_or(cost)
}

type RowKey<'a> = (&'a str, &'a str, &'a str, &'static str);

pub fn aggregate(
    mut scan: Scan,
    import: Option<Result<Value, ()>>,
    zone: Tz,
    account_snapshots: Vec<Value>,
    updated_at: &str,
) -> Value {
    let local_events = scan.events.ordered.len();
    let mut imported_at = Value::Null;
    if let Some(import) = import {
        let imported = import.ok().and_then(|import| {
            imported_at = import["updatedAt"].clone();
            merge_import(&mut scan, &import, zone)
        });
        if imported.is_none() {
            scan.errors.push("SSH import could not be read".into());
        }
    }
    let mut rows = BTreeMap::<RowKey<'_>, Row>::new();
    let mut hourly = BTreeMap::<(RowKey<'_>, Option<String>), Row>::new();
    let mut remote_events = 0;
    for (index, event) in scan.events.ordered.iter_mut().enumerate() {
        let device = match (index < local_events, event.remote) {
            (true, true) => "shared",
            (false, true) => "macmini",
            (_, false) => "this-mac",
        };
        remote_events += usize::from(event.remote);
        let local = event.timestamp.map(|time| time.with_timezone(&zone));
        if let Some(time) = local {
            event.date = time.date_naive().to_string();
        }
        // Keep the original offset when truncating a repeated local hour.
        let hour = local.map(|time| time.format("%Y-%m-%dT%H:00:00%:z").to_string());
        let key = (
            event.provider.as_str(),
            event.date.as_str(),
            event.model.as_str(),
            device,
        );
        rows.entry(key).or_default().add(&event.metrics);
        hourly.entry((key, hour)).or_default().add(&event.metrics);
    }
    let rows: Vec<_> = rows.into_iter().map(|(key, row)| row.json(key)).collect();
    let hourly: Vec<_> = hourly
        .into_iter()
        .map(|((key, hour), row)| {
            let mut row = row.json(key);
            row["hour"] = json!(hour);
            row
        })
        .collect();
    json!({
        "accountSnapshots": account_snapshots,
        "rows": rows,
        "hourlyRows": hourly,
        "timezone": zone.name(),
        "updatedAt": updated_at,
        "files": 0,
        "importedAt": imported_at,
        "harnessNames": scan.names,
        "engine": scan.engine,
        "localEvents": local_events,
        "remoteEvents": remote_events,
        "events": scan.events.ordered.len(),
        "skipped": scan.skipped,
        "errors": scan.errors.into_iter().collect::<BTreeSet<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timezone_override_and_environment_have_priority() {
        assert_eq!(
            resolve_timezone(Some("UTC"), Some("Asia/Seoul"), None, None).unwrap(),
            chrono_tz::UTC
        );
        assert_eq!(
            resolve_timezone(None, Some(":/usr/share/zoneinfo/Asia/Seoul"), None, None).unwrap(),
            chrono_tz::Asia::Seoul
        );
        assert!(resolve_timezone(Some("invalid"), Some("UTC"), None, None).is_err());
    }

    #[test]
    fn system_timezone_sources_and_fallback() {
        let mac = resolve_timezone(
            None,
            None,
            Some("/var/db/timezone/zoneinfo/Asia/Seoul"),
            Some("UTC"),
        )
        .unwrap();
        for (text, date) in [
            ("2026-10-01T14:59:59Z", "2026-10-01"),
            ("2026-10-01T15:00:00Z", "2026-10-02"),
        ] {
            assert_eq!(
                timestamp(&json!(text), false)
                    .unwrap()
                    .with_timezone(&mac)
                    .date_naive()
                    .to_string(),
                date
            );
        }
        let linux = resolve_timezone(
            None,
            None,
            Some("/etc/localtime"),
            Some("America/New_York\n"),
        )
        .unwrap();
        for (text, offset) in [
            ("2026-01-01T00:00:00Z", "-05:00"),
            ("2026-07-01T00:00:00Z", "-04:00"),
        ] {
            assert_eq!(
                timestamp(&json!(text), false)
                    .unwrap()
                    .with_timezone(&linux)
                    .format("%:z")
                    .to_string(),
                offset
            );
        }
        assert_eq!(
            resolve_timezone(None, Some("invalid/timezone"), None, None).unwrap(),
            chrono_tz::UTC
        );
    }

    #[test]
    fn imported_timestamp_and_merge_use_the_local_timezone() {
        let zone = chrono_tz::Asia::Seoul;
        let report = json!({"clients":[], "messages":[{
            "client":"claude","dedup":"k","model":"m","timestamp":"2026-10-18T15:30:00Z",
            "input":10,"output":5,"cacheRead":2,"cacheWrite":0,"cacheWrite1h":0,"reasoning":0,
        }]});
        let original = report.clone();
        let scan = convert(&report, zone).unwrap();
        let import = json!({"events":[
            {"id":event_id(&json!(["engine","claude","k"])), "output":8},
            {"id":"old","provider":"claude","date":"2026-10-19","model":"m","input":10,"output":5,"cacheRead":2}
        ]});
        let result = aggregate(scan, Some(Ok(import)), zone, vec![], "fixed");
        assert_eq!(report, original);
        assert_eq!(result["localEvents"], 1);
        assert_eq!(result["remoteEvents"], 2);
        assert_eq!(result["events"], 2);
        assert_eq!(result["rows"][0]["total"], 17);
        assert_eq!(result["rows"][1]["total"], 20);
        assert_eq!(result["hourlyRows"][0]["hour"], Value::Null);
        assert_eq!(result["hourlyRows"][1]["hour"], "2026-10-19T00:00:00+09:00");
        assert!(result["rows"][0].get("costRequests").is_none());
        assert!(result.get("missing").is_none());
    }
}
