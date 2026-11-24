pub use crate::events::*;
use chrono_tz::Tz;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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
                session: String::new(),
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
