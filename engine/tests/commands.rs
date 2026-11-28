use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_chronicles"))
}

#[test]
fn demo_export_embeds_assets_and_payload_from_any_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let result = cli()
        .current_dir(dir.path())
        .args(["export", "nested/snapshot.html", "--demo"])
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result);
    let page = fs::read_to_string(dir.path().join("nested/snapshot.html")).unwrap();
    for present in [
        "window.ChroniclesCombobox",
        "window.ChroniclesCalendar",
        "--primary-foreground:",
        "<style>",
        "data:image/svg+xml,",
        "data:font/woff2;base64,",
        "function formatMoney",
    ] {
        assert!(page.contains(present), "{present}");
    }
    for absent in [
        "src=\"/app.js\"",
        "src=\"/combobox.js\"",
        "src=\"/calendar.js\"",
        "href=\"/style.css\"",
        "href=\"/shadcn.css\"",
        "from \"./analytics.mjs\"",
        "pricing.mjs",
        "/Geist.woff2",
    ] {
        assert!(!page.contains(absent), "{absent}");
    }
    let payload = page
        .split_once("window.CHRONICLES_DATA=")
        .unwrap()
        .1
        .split_once(";</script>")
        .unwrap()
        .0;
    let report: Value = serde_json::from_str(payload).unwrap();
    assert_eq!(report["demo"], true);
    assert_eq!(report, chronicles::web::demo_report());
    assert!(report.get("pricing").is_none());
}

#[test]
fn raw_report_and_list_clients_work_with_an_isolated_home() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli()
        .current_dir(dir.path())
        .env("TOKSCALE_CONFIG_DIR", dir.path().join("engine"))
        .args(["report", "--offline", "--home"])
        .arg(dir.path())
        .args(["--clients", "cursor,claude", "--exclude", "claude"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["messages"], json!([]));
    assert!(report["tokscaleCore"].as_str().unwrap().contains("b265e83"));
    let output = cli().arg("--list-clients").output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let clients: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["clients"], clients);
    assert!(clients
        .as_array()
        .unwrap()
        .iter()
        .any(|client| client["id"] == "cursor"));
}

fn copilot_home(home: &Path) {
    let user = home.join("Library/Application Support/Code/User");
    let workspace = user.join("workspaceStorage/hash/chatSessions");
    let empty = user.join("globalStorage/emptyWindowChatSessions");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&empty).unwrap();
    // The engine already emits the first request. The second is filled in by later journal writes.
    fs::write(workspace.join("copied.jsonl"), r#"
{"kind":0,"v":{"sessionId":"ignored","requests":[{"timestamp":1773000000001,"modelId":"copilot/gpt-4.1","promptTokens":10,"result":{"usage":{"promptTokens":99}}},{"timestamp":1773000000002,"modelId":"copilot/auto"}]}}
{"kind":1,"k":["requests",1,"result"],"v":{"metadata":{"agentId":"github.copilot-chat","resolvedModel":"gpt-4.1"}}}
{"kind":1,"k":["requests",1,"result","usage","promptTokens"],"v":12}
{"kind":1,"k":["requests",1,"result","usage","completionTokens"],"v":4}
{"kind":2,"k":["requests"],"v":[{"timestamp":1773000000003,"modelId":"copilot/legacy","result":{"metadata":{}}},{"timestamp":1773000000004},{"timestamp":1773000000005,"result":{}},{"timestamp":1773000000006,"result":{"metadata":{"agentId":"another.extension"}}},{"timestamp":0,"result":{"metadata":{}}}]}
{"kind":2,"k":["requests",2,"response"],"v":[{"timestamp":1773000000007,"result":{"metadata":{}}}]}
{"kind":
"#).unwrap();
    fs::write(empty.join("copied.json"), r#"{
        "requests": [
            {"timestamp":1773000000002,"modelId":"copilot/stale","result":{"usage":{"promptTokens":200}}},
            {"timestamp":{"start":1773000000000},"modelId":"copilot/original","result":{"metadata":{}}}
        ]
    }"#).unwrap();

    let windows = home.join("AppData/Roaming/Code/User/globalStorage/emptyWindowChatSessions");
    fs::create_dir_all(&windows).unwrap();
    fs::write(windows.join("windows.jsonl"), r#"
{"kind":0,"v":{"requests":[]}}
{"kind":1,"k":["requests"],"v":[{"timestamp":1773000000010,"modelId":"copilot/windows","result":{"metadata":{}}}]}
"#).unwrap();

    let linux = home.join(".config/Code/User/workspaceStorage/hash/chatSessions");
    fs::create_dir_all(&linux).unwrap();
    fs::write(linux.join("linux.json"), r#"{
        "requests": [{"timestamp":1773000000011,"modelId":" github.copilot-chat/legacy ","result":{"details":"fixture"}}]
    }"#).unwrap();
}

fn copilot_command(root: &Path) -> Command {
    let cache = root.join("engine");
    fs::create_dir_all(&cache).unwrap();
    fs::write(
        cache.join("custom-pricing.json"),
        r#"{"models":{
        "gpt-4.1":{"input_cost_per_million_tokens":2,"output_cost_per_million_tokens":8}
    }}"#,
    )
    .unwrap();
    let mut cmd = cli();
    cmd.current_dir(root)
        .env("TOKSCALE_CONFIG_DIR", cache)
        .env("CHRONICLES_TIMEZONE", "UTC");
    cmd
}

#[test]
fn report_recovers_copilot_usage_and_request_only_history() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("fixture-home");
    copilot_home(&home);
    let run = |filters: &[&str]| {
        let output = copilot_command(root.path())
            .args(["report", "--offline", "--home"])
            .arg(&home)
            .args(filters)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let report = run(&["--clients", " copilot "]);
    let mut messages: Vec<_> = report["messages"].as_array().unwrap().iter().collect();
    messages.sort_by_key(|message| message["timestamp"].as_i64().unwrap());
    let actual: Vec<_> = messages
        .iter()
        .map(|m| {
            json!([
                m["client"],
                m["session"],
                m["dedup"],
                m["model"],
                m["input"],
                m["output"],
                m["messages"],
                m["costSource"]
            ])
        })
        .collect();
    assert_eq!(
        actual,
        json!([
            [
                "copilot",
                "copied",
                "copilot-vscode:copied:1773000000000",
                "original",
                0,
                0,
                1,
                "unpriced"
            ],
            [
                "copilot",
                "copied",
                "copilot-vscode:copied:1773000000001",
                "gpt-4.1",
                10,
                0,
                1,
                "estimated"
            ],
            [
                "copilot",
                "copied",
                "copilot-vscode:copied:1773000000002",
                "gpt-4.1",
                12,
                4,
                1,
                "estimated"
            ],
            [
                "copilot",
                "copied",
                "copilot-vscode:copied:1773000000003",
                "legacy",
                0,
                0,
                1,
                "unpriced"
            ],
            [
                "copilot",
                "windows",
                "copilot-vscode:windows:1773000000010",
                "windows",
                0,
                0,
                1,
                "unpriced"
            ],
            [
                "copilot",
                "linux",
                "copilot-vscode:linux:1773000000011",
                "legacy",
                0,
                0,
                1,
                "unpriced"
            ]
        ])
        .as_array()
        .unwrap()
        .clone()
    );
    assert!(messages[0]["cost"].is_null());
    assert_eq!(messages[2]["provider"], messages[1]["provider"]);
    assert!((messages[2]["cost"].as_f64().unwrap() - 0.000056).abs() < 1e-12);
    for filters in [
        &["--clients", "codex"][..],
        &["--clients", "copilot", "--exclude", " copilot "][..],
    ] {
        assert_eq!(run(filters)["messages"], json!([]));
    }
}

#[test]
fn offline_export_includes_recovered_copilot_request_counts() {
    let root = tempfile::tempdir().unwrap();
    copilot_home(root.path());
    let output = copilot_command(root.path())
        .env("HOME", root.path())
        .env("USERPROFILE", root.path())
        .args(["export", "snapshot.html", "--offline"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let page = fs::read_to_string(root.path().join("snapshot.html")).unwrap();
    let payload = page
        .split_once("window.CHRONICLES_DATA=")
        .unwrap()
        .1
        .split_once(";</script>")
        .unwrap()
        .0;
    let report: Value = serde_json::from_str(payload).unwrap();
    let rows = report["rows"].as_array().unwrap();
    let legacy = rows
        .iter()
        .find(|r| r["provider"] == "copilot" && r["model"] == "legacy")
        .unwrap();
    assert_eq!(legacy["requests"], 2);
    assert_eq!(legacy["total"], 0);
    assert_eq!(legacy["costRequests"], 0);
}

#[cfg(unix)]
mod sync {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;

    struct Fake {
        root: tempfile::TempDir,
    }

    impl Fake {
        fn new() -> Self {
            let fake = Self {
                root: tempfile::tempdir().unwrap(),
            };
            fs::create_dir_all(fake.path().join("engine")).unwrap();
            fs::create_dir_all(fake.path().join("src/web")).unwrap();
            fs::write(fake.path().join("engine/Cargo.toml"), "").unwrap();
            fs::write(fake.path().join("src/web/index.html"), "").unwrap();
            fs::write(
                fake.path().join("report.json"),
                include_str!("fixtures/engine-report.json"),
            )
            .unwrap();
            fake.script(
                "uname",
                "test \"$1\" = '-sm' || exit 1\nprintf 'Darwin arm64\\n'",
            );
            fake.script(
                "ssh",
                r#"
test "$1 $2 $3 $4 $5" = "-o BatchMode=yes -o ConnectTimeout=10 fixture.invalid" || exit 99
printf '%s\n' "$6" >> "$FAKE_DIR/calls"
case "$6" in
  'uname -sm') printf '%s\n' "${REMOTE_PLATFORM:-Darwin arm64}" ;;
  'shasum -a 256 '*) printf '%s' "$DIGEST" ;;
  'mkdir -p '*|'chmod +x '*) ;;
  'TOKSCALE_CONFIG_DIR='*)
    if [ "$FAILURE" = 'status' ]; then printf 'noise\nboom\n' >&2; exit 3; fi
    if [ "$FAILURE" = 'json' ]; then printf 'not json'; exit 0; fi
    /bin/cat "$FAKE_DIR/report.json"
    printf 'Warning: sqlite busy\n' >&2 ;;
  *) exit 99 ;;
esac"#,
            );
            fake.script(
                "scp",
                r#"
test "$1 $2 $3 $4 $5" = "-q -o BatchMode=yes -o ConnectTimeout=10" || exit 99
printf 'scp %s\n' "$7" >> "$FAKE_DIR/calls"
case "$7" in fixture.invalid:.cache/chronicles/chronicles.upload-*.tmp) ;; *) exit 99 ;; esac
"#,
            );
            fake
        }

        fn path(&self) -> &Path {
            self.root.path()
        }

        fn script(&self, name: &str, body: &str) {
            let path = self.path().join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }

        fn sync(&self) -> Command {
            let mut command = cli();
            command
                .current_dir(self.path())
                .env("PATH", self.path())
                .env("FAKE_DIR", self.path())
                .env("CHRONICLES_TIMEZONE", "America/New_York")
                .args(["sync", "--host", "fixture.invalid"]);
            command
        }

        fn calls(&self) -> String {
            fs::read_to_string(self.path().join("calls")).unwrap()
        }
    }

    #[test]
    fn sync_verifies_platform_hash_and_runs_remote_report() {
        let fake = Fake::new();
        for (digest, copy, offline) in [
            (
                format!(
                    "{:x}  path\n",
                    Sha256::digest(fs::read(env!("CARGO_BIN_EXE_chronicles")).unwrap())
                ),
                false,
                false,
            ),
            (String::new(), true, true),
            ("0000  path\n".into(), true, false),
        ] {
            fs::write(fake.path().join("calls"), "").unwrap();
            let mut command = fake.sync();
            if offline {
                command.arg("--offline");
            }
            let output = command.env("DIGEST", digest).output().unwrap();
            assert!(output.status.success(), "{:?}", output);
            let calls = fake.calls();
            assert!(calls.starts_with("uname -sm\nshasum -a 256 "));
            assert_eq!(
                calls.contains("scp fixture.invalid:.cache/chronicles/chronicles.upload-"),
                copy
            );
            assert_eq!(calls.contains("chmod +x "), copy);
            assert_eq!(calls.contains(" && mv "), copy);
            let suffix = if offline { " --offline" } else { "" };
            assert!(calls.ends_with(&format!("TOKSCALE_CONFIG_DIR=\"$HOME/.cache/chronicles/engine\" \"$HOME/.cache/chronicles/chronicles\" report{suffix}\n")));
            let path = fake.path().join(".local/imports/macmini.json");
            let mut report: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            report["updatedAt"] = json!("2026-11-02T00:00:00+00:00");
            let expected: Value =
                serde_json::from_str(include_str!("fixtures/sync-import.json")).unwrap();
            assert_eq!(report, expected);
            assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        }
    }

    #[test]
    fn sync_rejects_nonmatching_platform_before_copy() {
        for platform in ["Darwin x86_64", "Linux x86_64"] {
            let fake = Fake::new();
            let output = fake
                .sync()
                .env("REMOTE_PLATFORM", platform)
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("same architecture"));
            assert_eq!(fake.calls(), "uname -sm\n");
        }
    }

    #[test]
    fn sync_remote_errors_leave_existing_import_intact() {
        for (failure, expected) in [
            ("status", "status 3 (boom)"),
            ("json", "remote engine output could not be read"),
        ] {
            let fake = Fake::new();
            let path = fake.path().join(".local/imports/macmini.json");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "existing import").unwrap();
            let output = fake.sync().env("FAILURE", failure).output().unwrap();
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
            assert_eq!(fs::read_to_string(path).unwrap(), "existing import");
        }
    }
}
