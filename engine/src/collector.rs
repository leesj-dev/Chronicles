use crate::{copilot, limits::Bridge, paths, report, usage};
use chrono_tz::Tz;
use serde_json::Value;
use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::{sync::Semaphore, task::JoinHandle};

static SCAN_SLOT: Semaphore = Semaphore::const_new(1);
const SCAN_TIMEOUT: Duration = Duration::from_secs(600);

pub struct Collector {
    options: report::Options,
    import_path: PathBuf,
    bridge: Bridge,
    zone: Tz,
    config: io::Result<()>,
}

impl Collector {
    pub fn new(
        root: &Path,
        offline: bool,
        zone: Tz,
        config: io::Result<()>,
    ) -> Result<Self, reqwest::Error> {
        Ok(Self {
            options: report::Options::collect(offline),
            import_path: paths::import_path(root),
            bridge: Bridge::new()?,
            zone,
            config,
        })
    }

    pub async fn collect(&mut self) -> Value {
        let scan = self.scan().await;
        usage::aggregate(
            scan,
            read_import(&self.import_path),
            self.zone,
            self.bridge.collect().await,
            &usage::now(),
        )
    }

    async fn scan(&self) -> usage::Scan {
        if self.config.is_err() {
            return scan_result(Err("could not initialize the cache".into()), b"");
        }
        let Ok(permit) = SCAN_SLOT.try_acquire() else {
            return scan_result(Err("scan timed out".into()), b"");
        };
        let options = self.options.clone();
        let zone = self.zone;
        let runtime = tokio::runtime::Handle::current();
        let worker = tokio::task::spawn_blocking(move || {
            // The permit also prevents another scan from redirecting stderr
            // while a timed-out in-process scan is still finishing.
            let _permit = permit;
            #[cfg(unix)]
            let capture = StderrCapture::start().ok();
            let result = runtime.block_on(copilot::scan(&options, None, None));
            #[cfg(unix)]
            let stderr = capture
                .and_then(|capture| capture.finish().ok())
                .unwrap_or_default();
            #[cfg(not(unix))]
            let stderr = Vec::new();
            scan_result(
                result.map(|report| usage::convert_local(report, zone)),
                &stderr,
            )
        });
        scan_before_deadline(worker, SCAN_TIMEOUT).await
    }
}

async fn scan_before_deadline(worker: JoinHandle<usage::Scan>, deadline: Duration) -> usage::Scan {
    match tokio::time::timeout(deadline, worker).await {
        Ok(Ok(scan)) => scan,
        Ok(Err(_)) => scan_result(Err("scan failed".into()), b""),
        Err(_) => scan_result(Err("scan timed out".into()), b""),
    }
}

fn scan_result(result: Result<usage::Scan, String>, stderr: &[u8]) -> usage::Scan {
    let mut errors = usage::warnings(stderr);
    let mut scan = result.unwrap_or_else(|error| {
        errors.push(format!("engine: {error}"));
        usage::Scan::default()
    });
    scan.errors.extend(errors);
    scan
}

fn read_import(path: &Path) -> Option<Result<Value, ()>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(serde_json::from_slice(&bytes).map_err(|_| ())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => Some(Err(())),
    }
}

#[cfg(unix)]
struct StderrCapture {
    saved: std::os::fd::OwnedFd,
    file: std::fs::File,
}

#[cfg(unix)]
impl StderrCapture {
    fn start() -> io::Result<Self> {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        let file = tempfile::tempfile()?;
        // Duplicate stderr with close-on-exec, then redirect fd 2 to the file.
        let saved = unsafe { libc::fcntl(libc::STDERR_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
        if saved == -1 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fcntl returned a fresh owned descriptor.
        let saved = unsafe { OwnedFd::from_raw_fd(saved) };
        if unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { saved, file })
    }

    fn finish(mut self) -> io::Result<Vec<u8>> {
        use std::io::{Read, Seek};
        self.file.rewind()?;
        let mut bytes = Vec::new();
        self.file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(unix)]
impl Drop for StderrCapture {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // Restore stderr on success, error, or unwind.
        unsafe { libc::dup2(self.saved.as_raw_fd(), libc::STDERR_FILENO) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scan_failure_is_an_empty_usage_report_with_an_error() {
        let scan = scan_result(
            Err("could not read history".into()),
            b"noise\nWarning: busy\n",
        );
        let report = usage::aggregate(scan, None, chrono_tz::UTC, Vec::new(), "fixed");
        assert_eq!(report["engine"], Value::Null);
        assert_eq!(report["harnessNames"], json!({}));
        assert_eq!(report["rows"], json!([]));
        assert_eq!(
            report["errors"],
            json!(["engine: Warning: busy", "engine: could not read history"])
        );
    }

    #[test]
    fn scans_return_a_timeout_error_at_the_deadline() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let worker = tokio::spawn(std::future::pending());
            let scan = scan_before_deadline(worker, Duration::from_millis(5)).await;
            assert_eq!(scan.errors, ["engine: scan timed out"]);
        });
    }

    #[test]
    fn absent_import_is_ignored_and_invalid_import_is_one_error() {
        let root = tempfile::tempdir().unwrap();
        let import = paths::import_path(root.path());
        assert!(read_import(&import).is_none());
        std::fs::create_dir_all(import.parent().unwrap()).unwrap();
        std::fs::write(&import, "{").unwrap();
        let report = usage::aggregate(
            Default::default(),
            read_import(&import),
            chrono_tz::UTC,
            Vec::new(),
            "fixed",
        );
        assert_eq!(report["errors"], json!(["SSH import could not be read"]));
    }
}
