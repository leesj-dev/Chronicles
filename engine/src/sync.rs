use crate::{paths, usage};
use chrono_tz::Tz;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    path::Path,
    process::Output,
    time::Duration,
};
use tokio::{process::Command, time::timeout};

const REMOTE_DIR: &str = ".cache/chronicles";
const REMOTE_BINARY: &str = ".cache/chronicles/chronicles";
const SSH_ARGS: [&str; 4] = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"];
const COMMAND_TIMEOUT: u64 = 60;

async fn run(command: &mut Command, seconds: u64) -> Result<Output, String> {
    let name = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    command.kill_on_drop(true);
    timeout(Duration::from_secs(seconds), command.output())
        .await
        .map_err(|_| format!("{name} failed: timed out"))?
        .map_err(|error| format!("{name} failed: {error}"))
}

fn tail(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim()
        .lines()
        .last()
        .unwrap_or("")
        .to_owned()
}

async fn remote(host: &str, script: &str, seconds: u64, check: bool) -> Result<Output, String> {
    let result = run(
        Command::new("ssh").args(SSH_ARGS).args([host, script]),
        seconds,
    )
    .await?;
    if check && !result.status.success() {
        let message = tail(&result.stderr);
        return Err(if message.is_empty() {
            "ssh command failed".into()
        } else {
            message
        });
    }
    Ok(result)
}

async fn check_platform(host: &str) -> Result<(), String> {
    let local = run(Command::new("uname").arg("-sm"), COMMAND_TIMEOUT).await?;
    let there = remote(host, "uname -sm", COMMAND_TIMEOUT, true).await?;
    let local = String::from_utf8_lossy(&local.stdout);
    let there = String::from_utf8_lossy(&there.stdout);
    let (local, there) = (local.trim(), there.trim());
    if !local.starts_with("Darwin") || there != local {
        let there = if there.is_empty() { "unknown" } else { there };
        return Err(format!("remote is \"{there}\" but this Mac is \"{local}\": sync copies the engine binary, so both must be macOS on the same architecture"));
    }
    Ok(())
}

fn sha256(binary: &Path) -> io::Result<String> {
    let mut file = std::fs::File::open(binary)?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}

async fn ensure_binary(host: &str, binary: &Path, digest: &str) -> Result<bool, String> {
    let found = remote(
        host,
        &format!("shasum -a 256 \"$HOME/{REMOTE_BINARY}\" 2>/dev/null"),
        COMMAND_TIMEOUT,
        false,
    )
    .await?;
    if String::from_utf8_lossy(&found.stdout)
        .split_whitespace()
        .next()
        == Some(digest)
    {
        return Ok(false);
    }
    remote(
        host,
        &format!("mkdir -p \"$HOME/{REMOTE_DIR}/engine\""),
        COMMAND_TIMEOUT,
        true,
    )
    .await?;
    // Distinct staging names let simultaneous uploads finish independently.
    let upload = tempfile::Builder::new()
        .prefix("upload-")
        .tempfile()
        .map_err(|e| e.to_string())?;
    let suffix = upload.path().file_name().unwrap().to_string_lossy();
    let temporary = format!("{REMOTE_BINARY}.{suffix}.tmp");
    let copied = run(
        Command::new("scp")
            .arg("-q")
            .args(SSH_ARGS)
            .arg(binary)
            .arg(format!("{host}:{temporary}")),
        300,
    )
    .await?;
    if !copied.status.success() {
        let message = String::from_utf8_lossy(&copied.stderr);
        let message = message.trim();
        return Err(format!(
            "scp failed: {}",
            if message.is_empty() {
                "unknown error"
            } else {
                message
            }
        ));
    }
    remote(
        host,
        &format!(
            "chmod +x \"$HOME/{temporary}\" && mv \"$HOME/{temporary}\" \"$HOME/{REMOTE_BINARY}\""
        ),
        COMMAND_TIMEOUT,
        true,
    )
    .await?;
    Ok(true)
}

async fn remote_report(host: &str, offline: bool) -> Result<Output, String> {
    let offline_arg = if offline { " --offline" } else { "" };
    let result = remote(host, &format!("TOKSCALE_CONFIG_DIR=\"$HOME/{REMOTE_DIR}/engine\" \"$HOME/{REMOTE_BINARY}\" report{offline_arg}"), 900, false).await?;
    if !result.status.success() {
        let status = result
            .status
            .code()
            .map_or_else(|| result.status.to_string(), |code| code.to_string());
        let message: String = tail(&result.stderr).chars().take(200).collect();
        let detail = if message.is_empty() {
            String::new()
        } else {
            format!(" ({message})")
        };
        return Err(format!("remote engine exited with status {status}{detail}"));
    }
    Ok(result)
}

pub async fn sync(
    root: &Path,
    binary: &Path,
    host: &str,
    offline: bool,
    zone: Tz,
) -> Result<(), String> {
    // A destination is either an SSH alias or a hostname with optional user.
    // Reject input that OpenSSH or scp could interpret as options or a path.
    if host.is_empty()
        || host.starts_with('-')
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-@".contains(&b))
    {
        return Err("invalid SSH host (use a hostname or SSH config alias)".into());
    }
    let digest =
        sha256(binary).map_err(|error| format!("could not read the local binary: {error}"))?;
    check_platform(host).await?;
    if ensure_binary(host, binary, &digest).await? {
        eprintln!("Copied the engine to {host}");
    }
    let output = remote_report(host, offline).await?;
    let data = usage::build_import(&output.stdout, &output.stderr, zone, &usage::now())?;
    let path = paths::import_path(root);
    write_import(&path, &data).map_err(|error| format!("could not write the import: {error}"))?;
    println!(
        "{}",
        json!({"updatedAt": data["updatedAt"], "files": data["files"], "errors": data["errors"]})
    );
    println!(
        "Imported {} usage records from {host}",
        data["events"].as_array().unwrap().len()
    );
    Ok(())
}

fn write_import(path: &Path, data: &serde_json::Value) -> io::Result<()> {
    let directory = path.parent().expect("import path has a parent");
    std::fs::create_dir_all(directory)?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(&mut temporary, data)?;
    temporary.flush()?;
    temporary.persist(path)?;
    Ok(())
}
