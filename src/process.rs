use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::config;
use crate::error::RunnerError;

/// Directory used to store the supervised `llama-server` process's pidfile and log.
pub const RUNNER_DIR: &str = ".runner";

/// Path to the pidfile written for the supervised `llama-server` process.
pub const PID_FILE: &str = ".runner/llama-server.pid";

/// Path to the log file the supervised `llama-server` process's stdout/stderr are redirected to.
pub const LOG_FILE: &str = ".runner/llama-server.log";

const HEALTH_POLL_TIMEOUT: Duration = Duration::from_secs(120);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const STOP_POLL_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Reads the PID stored in the pidfile at the given path, if it exists.
///
/// # Arguments
/// * `path` - Path to the pidfile.
///
/// # Returns
/// `Some(pid)` if the pidfile exists and contains a valid PID, `None` if the pidfile does
/// not exist, or a `RunnerError` if it exists but cannot be read or parsed.
pub fn read_pidfile(path: &Path) -> Result<Option<u32>, RunnerError> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)
        .map_err(|e| RunnerError::Io(format!("failed to read pidfile {}: {e}", path.display())))?;
    let pid = contents
        .trim()
        .parse::<u32>()
        .map_err(|e| RunnerError::Process(format!("invalid pid in {}: {e}", path.display())))?;
    Ok(Some(pid))
}

/// Writes the given PID to the pidfile at the given path.
///
/// # Arguments
/// * `path` - Path to the pidfile.
/// * `pid` - Process ID to write.
///
/// # Returns
/// `Ok(())` on success, or a `RunnerError` if the file cannot be written.
pub fn write_pidfile(path: &Path, pid: u32) -> Result<(), RunnerError> {
    fs::write(path, pid.to_string())
        .map_err(|e| RunnerError::Io(format!("failed to write pidfile {}: {e}", path.display())))
}

/// Removes the pidfile at the given path if it exists.
///
/// # Arguments
/// * `path` - Path to the pidfile.
///
/// # Returns
/// `Ok(())` on success (including if the file was already absent), or a `RunnerError` if
/// removal fails.
pub fn remove_pidfile(path: &Path) -> Result<(), RunnerError> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| {
            RunnerError::Io(format!("failed to remove pidfile {}: {e}", path.display()))
        })?;
    }
    Ok(())
}

/// Checks whether the process with the given PID is currently alive by sending signal 0.
///
/// # Arguments
/// * `pid` - Process ID to check.
///
/// # Returns
/// `true` if the process is alive, `false` otherwise.
pub fn is_process_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Sends SIGTERM to the process with the given PID.
///
/// # Arguments
/// * `pid` - Process ID to terminate.
///
/// # Returns
/// `Ok(())` if the signal was sent, or a `RunnerError` if the `kill` command could not be run.
fn send_sigterm(pid: u32) -> Result<(), RunnerError> {
    Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .map_err(|e| RunnerError::Process(format!("failed to send SIGTERM to {pid}: {e}")))?;
    Ok(())
}

/// Checks whether the `/health` endpoint at the given host and port returns HTTP 200.
///
/// # Arguments
/// * `host` - Host the server is bound to.
/// * `port` - Port the server is listening on.
///
/// # Returns
/// `true` if the endpoint responded with HTTP 200, `false` for any other outcome
/// (including connection failure).
fn health_check(host: &str, port: u16) -> bool {
    let url = format!("http://{host}:{port}/health");
    match ureq::get(&url)
        .config()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .call()
    {
        Ok(response) => response.status() == 200,
        Err(_) => false,
    }
}

/// Polls the `/health` endpoint until it responds with HTTP 200 or the given timeout elapses.
///
/// # Arguments
/// * `host` - Host the server is bound to.
/// * `port` - Port the server is listening on.
/// * `timeout` - Maximum duration to poll for.
///
/// # Returns
/// `true` if the endpoint became healthy within the timeout, `false` otherwise.
fn poll_health(host: &str, port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        if health_check(host, port) {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(HEALTH_POLL_INTERVAL);
    }
}

/// Ensures no live `llama-server` process is already supervised, clearing a stale
/// pidfile left behind by a process that is no longer running.
///
/// # Arguments
/// * `pid_path` - Path to the pidfile.
///
/// # Returns
/// `Ok(())` if no live supervised process exists, or a `RunnerError` if one is already
/// running.
fn ensure_not_running(pid_path: &Path) -> Result<(), RunnerError> {
    if let Some(pid) = read_pidfile(pid_path)? {
        if is_process_alive(pid) {
            return Err(RunnerError::Process(format!(
                "llama-server already running with pid {pid} (see {})",
                pid_path.display()
            )));
        }
        remove_pidfile(pid_path)?;
    }
    Ok(())
}

/// Opens the shared stdout/stderr log file for the supervised `llama-server` process,
/// creating it if absent and truncating it if present.
///
/// # Arguments
/// * `log_path` - Path to the log file.
///
/// # Returns
/// A `(stdout, stderr)` pair of independent handles to the same log file, or a
/// `RunnerError` if the file could not be created or duplicated.
fn open_log_handles(log_path: &Path) -> Result<(fs::File, fs::File), RunnerError> {
    let stdout_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)
        .map_err(|e| {
            RunnerError::Io(format!("failed to create log {}: {e}", log_path.display()))
        })?;
    let stderr_file = stdout_file
        .try_clone()
        .map_err(|e| RunnerError::Io(format!("failed to duplicate log handle: {e}")))?;
    Ok((stdout_file, stderr_file))
}

/// Spawns `llama-server` for the given configuration, redirecting its stdout and
/// stderr to the given log handles.
///
/// # Arguments
/// * `cfg` - The runner configuration to translate into CLI flags.
/// * `stdout_file` - Handle `llama-server`'s stdout is redirected to.
/// * `stderr_file` - Handle `llama-server`'s stderr is redirected to.
///
/// # Returns
/// The spawned `Child`, or a `RunnerError` if `llama-server` could not be spawned.
fn spawn_llama_server(
    cfg: &config::Config,
    stdout_file: fs::File,
    stderr_file: fs::File,
) -> Result<Child, RunnerError> {
    let args = config::server_args(cfg);
    Command::new("llama-server")
        .args(&args)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|e| RunnerError::Process(format!("failed to spawn llama-server: {e}")))
}

/// Runs the `start` subcommand: spawns and supervises a `llama-server` process
/// according to the given configuration file.
///
/// # Arguments
/// * `config_path` - Path to the TOML configuration file to load.
///
/// # Returns
/// `Ok(())` if `llama-server` was spawned and became healthy, or a `RunnerError` describing
/// why it could not be started.
pub fn handle_start(config_path: &Path) -> Result<(), RunnerError> {
    let cfg = config::load_config(config_path)?;

    let pid_path = PathBuf::from(PID_FILE);
    ensure_not_running(&pid_path)?;

    fs::create_dir_all(RUNNER_DIR)
        .map_err(|e| RunnerError::Io(format!("failed to create {RUNNER_DIR}: {e}")))?;

    let log_path = PathBuf::from(LOG_FILE);
    let (stdout_file, stderr_file) = open_log_handles(&log_path)?;

    let mut child = spawn_llama_server(&cfg, stdout_file, stderr_file)?;

    write_pidfile(&pid_path, child.id())?;

    if poll_health(&cfg.host, cfg.port, HEALTH_POLL_TIMEOUT) {
        println!(
            "llama-server is healthy at http://{}:{} (pid {})",
            cfg.host,
            cfg.port,
            child.id()
        );
        Ok(())
    } else {
        let _ = child.kill();
        let _ = child.wait();
        remove_pidfile(&pid_path)?;
        Err(RunnerError::Timeout(
            "llama-server did not become healthy within 120s".to_string(),
        ))
    }
}

/// Runs the `stop` subcommand: sends SIGTERM to the supervised `llama-server`
/// process and waits for it to exit.
///
/// # Returns
/// `Ok(())` if the process was stopped, or a `RunnerError` if it was not running or did not
/// exit in time.
pub fn handle_stop() -> Result<(), RunnerError> {
    let pid_path = PathBuf::from(PID_FILE);
    let pid = match read_pidfile(&pid_path)? {
        Some(pid) => pid,
        None => {
            return Err(RunnerError::Process(
                "llama-server is not running (no pidfile found)".to_string(),
            ));
        }
    };

    send_sigterm(pid)?;

    let start = Instant::now();
    while start.elapsed() < STOP_POLL_TIMEOUT {
        if !is_process_alive(pid) {
            remove_pidfile(&pid_path)?;
            println!("llama-server (pid {pid}) stopped");
            return Ok(());
        }
        std::thread::sleep(STOP_POLL_INTERVAL);
    }

    Err(RunnerError::Process(format!(
        "llama-server (pid {pid}) did not exit within {}s",
        STOP_POLL_TIMEOUT.as_secs()
    )))
}

/// Runs the `status` subcommand: reports pidfile presence, process liveness, and
/// `/health` reachability for the `llama-server` described by the given configuration
/// file.
///
/// # Arguments
/// * `config_path` - Path to the TOML configuration file to load.
///
/// # Returns
/// `Ok(())` if the pidfile is present, the process is alive, and `/health` returns 200,
/// or a `RunnerError` if any of those checks fail.
pub fn handle_status(config_path: &Path) -> Result<(), RunnerError> {
    let cfg = config::load_config(config_path)?;
    let pid_path = PathBuf::from(PID_FILE);

    let pid = read_pidfile(&pid_path)?;
    let pidfile_present = pid.is_some();
    let process_alive = pid.map(is_process_alive).unwrap_or(false);
    let healthy = health_check(&cfg.host, cfg.port);

    println!("pidfile present: {pidfile_present}");
    println!("process alive: {process_alive}");
    println!("health check (GET /health == 200): {healthy}");

    if pidfile_present && process_alive && healthy {
        Ok(())
    } else {
        Err(RunnerError::Process(
            "llama-server is not fully healthy".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Builds a unique pidfile path under the OS temp directory for a single test.
    ///
    /// # Arguments
    /// * `label` - A short label identifying the test, to keep paths readable.
    ///
    /// # Returns
    /// A path under the OS temp directory that no other test run is expected to use.
    fn unique_pidfile_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("llm-runner-test-{label}-{nanos}.pid"))
    }

    #[test]
    fn write_then_read_pidfile_round_trips() {
        let path = unique_pidfile_path("round-trip");
        write_pidfile(&path, 4242).expect("write_pidfile");
        assert_eq!(read_pidfile(&path).expect("read_pidfile"), Some(4242));
        remove_pidfile(&path).expect("cleanup");
    }

    #[test]
    fn read_pidfile_returns_none_when_absent() {
        let path = unique_pidfile_path("absent");
        assert_eq!(read_pidfile(&path).expect("read_pidfile"), None);
    }

    #[test]
    fn remove_pidfile_is_idempotent_when_absent() {
        let path = unique_pidfile_path("remove-absent");
        assert!(remove_pidfile(&path).is_ok());
    }

    #[test]
    fn remove_pidfile_deletes_existing_file() {
        let path = unique_pidfile_path("remove-existing");
        write_pidfile(&path, 1).expect("write_pidfile");
        remove_pidfile(&path).expect("remove_pidfile");
        assert!(!path.exists());
    }

    #[test]
    fn read_pidfile_errors_on_non_numeric_contents() {
        let path = unique_pidfile_path("invalid-contents");
        fs::write(&path, "not-a-pid").expect("write invalid contents");
        assert!(read_pidfile(&path).is_err());
        let _ = fs::remove_file(&path);
    }
}
