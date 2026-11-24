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
