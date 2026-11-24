use chronicles::{usage, web};
use chrono_tz::America::New_York;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const RAW: &str = include_str!("fixtures/engine-report.json");
const WARNINGS: &[u8] = b"noise\n  Warning: sqlite busy  \nWarning: sqlite busy\n";

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn usage_matches_python_with_legacy_import_and_dst() {
    let mut scan = usage::convert(&fixture(RAW), New_York).unwrap();
    scan.errors = usage::warnings(WARNINGS);
    let import = fixture(include_str!("fixtures/macmini.json"));
    let report = usage::aggregate(scan, Some(Ok(import)), New_York, vec![], "normalized");
    assert_eq!(report, fixture(include_str!("fixtures/usage.json")));
    let daily = report["rows"].as_array().unwrap();
    let hourly = report["hourlyRows"].as_array().unwrap();
    for metric in ["requests", "total", "costRequests"] {
        let sum = |rows: &[Value]| {
            rows.iter()
                .filter_map(|row| row[metric].as_u64())
                .sum::<u64>()
        };
        assert_eq!(sum(daily), sum(hourly), "{metric}");
    }
}

#[test]
fn ids_match_python_for_unicode_and_nested_keys() {
    for case in fixture(include_str!("fixtures/event-ids.json"))
        .as_array()
        .unwrap()
    {
        assert_eq!(usage::event_id(&case["key"]), case["id"].as_str().unwrap());
    }
}

#[test]
fn sync_import_matches_python_and_preserves_local_ids() {
    let data = usage::build_import(
        RAW.as_bytes(),
        WARNINGS,
        New_York,
        "2026-11-02T00:00:00+00:00",
    )
    .unwrap();
    assert_eq!(data, fixture(include_str!("fixtures/sync-import.json")));
    let original = fixture(RAW);
    let scan = usage::convert(&original, New_York).unwrap();
    let report = usage::aggregate(scan, Some(Ok(data)), New_York, vec![], "normalized");
    assert_eq!(report["events"], 6);
    assert_eq!(report["localEvents"], 6);
    assert_eq!(report["remoteEvents"], 6);
    assert!(report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["device"] == "shared"));
}

#[test]
fn demo_matches_python_and_hourly_values_sum_to_daily() {
    let report = web::demo_report();
    assert_eq!(report, fixture(include_str!("fixtures/demo.json")));
    fn sums(rows: &Value) -> BTreeMap<[String; 4], [i64; 8]> {
        let mut group = BTreeMap::new();
        for row in rows.as_array().unwrap() {
            let key = ["date", "provider", "model", "device"]
                .map(|field| row[field].as_str().unwrap().to_owned());
            let values = group.entry(key).or_insert([0; 8]);
            for (index, field) in [
                "requests",
                "input",
                "output",
                "cacheRead",
                "cacheWrite",
                "total",
                "costRequests",
            ]
            .into_iter()
            .enumerate()
            {
                values[index] += row[field].as_i64().unwrap();
            }
            values[7] += (row["cost"].as_f64().unwrap() * 1e6).round() as i64;
        }
        group
    }
    assert_eq!(sums(&report["rows"]), sums(&report["hourlyRows"]));
    assert!(report["rows"].as_array().unwrap().iter().all(|row| {
        row["requests"] == row["costRequests"] && row["cost"].as_f64().unwrap() > 0.0
    }));
}

#[test]
fn report_conversion_rejects_bad_shape_and_counts_bad_messages() {
    assert!(usage::convert(&json!([]), New_York).is_err());
    assert!(usage::convert(&json!({"clients":[{}], "messages":[]}), New_York).is_err());
    assert!(usage::build_import(b"not json", b"", New_York, "fixed").is_err());
    let scan = usage::convert(&fixture(RAW), New_York).unwrap();
    let report = usage::aggregate(scan, None, New_York, vec![], "normalized");
    assert_eq!(report["skipped"], 2);
    assert_eq!(report["files"], 0);
    assert_eq!(
        report["harnessNames"],
        json!({"aside":"Aside","claude":"Claude Code","antigravity":"Antigravity"})
    );
    let rows = report["rows"].as_array().unwrap();
    let claude = rows
        .iter()
        .find(|row| {
            row["provider"] == "claude" && row["model"] == "claude-x" && row["output"] == 12
        })
        .unwrap();
    assert_eq!(claude["requests"], 1);
    assert_eq!(claude["cost"], 0.5);
    assert_eq!(claude["cacheWrite1h"], 2);
    assert_eq!(claude["reasoning"], 3);
    assert_eq!(claude["total"], 124);
    let unpriced = rows.iter().find(|row| row["provider"] == "aside").unwrap();
    assert_eq!(unpriced["requests"], 2);
    assert_eq!(unpriced["costRequests"], 0);
    assert_eq!(unpriced["cost"], 0.0);
}

#[test]
fn warnings_are_trimmed_deduplicated_and_limited() {
    let long = "가".repeat(500);
    let stderr = format!(
        "noise\n Warning: busy \nWarning: busy\nWarning: {long}\n{}",
        (0..15)
            .map(|i| format!("Warning: {i}\n"))
            .collect::<String>()
    );
    let results = usage::warnings(stderr.as_bytes());
    assert_eq!(results.len(), 10);
    assert_eq!(results[0], "engine: Warning: busy");
    assert_eq!(results[1].chars().count(), 508);
}
