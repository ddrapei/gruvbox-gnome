//! The GNOME command-line tools this program talks to (gnome-shell, gsettings,
//! gnome-extensions, systemctl), each run with a timeout so a hung tool can
//! never stall the login service.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;

const TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(20);

pub struct Output {
    pub success: bool,
    /// Trimmed, lossily decoded.
    pub stdout: String,
}

/// Run a program; None if it could not be started or did not finish in time.
pub fn run(program: &str, args: &[&str], timeout: Duration) -> Option<Output> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Drain stdout while waiting: a child that fills the pipe buffer would
    // otherwise block forever on write and never exit.
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let stdout = reader.join().ok()?;
    Some(Output { success: status.success(), stdout: String::from_utf8_lossy(&stdout).trim().to_string() })
}

/// Start a program and let it run on its own (e.g. open a file in Firefox).
pub fn spawn_detached(program: &str, args: &[&OsStr]) -> io::Result<()> {
    Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(drop)
}

/// Whether an executable `program` is on PATH.
pub fn has_program(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path)
            .any(|dir| std::fs::metadata(dir.join(program)).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
    })
}

pub fn shell_version() -> String {
    run("gnome-shell", &["--version"], Duration::from_secs(5))
        .map(|out| out.stdout)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// A GSettings value as printed by `gsettings get` (GVariant text, e.g. `'x'`).
pub fn gsetting(schema: &str, key: &str) -> Option<String> {
    run("gsettings", &["get", schema, key], TIMEOUT).filter(|out| out.success).map(|out| out.stdout)
}

/// Set a GSettings value (`value` in GVariant text, e.g. `'x'`).
pub fn gsetting_set(schema: &str, key: &str, value: &str) -> Result<(), String> {
    match run("gsettings", &["set", schema, key, value], TIMEOUT) {
        Some(out) if out.success => Ok(()),
        _ => Err(format!("`gsettings set {schema} {key} {value}` failed")),
    }
}

/// `systemctl --user <args>`; empty output if it fails.
pub fn systemctl_user(args: &[&str]) -> String {
    let args: Vec<&str> = std::iter::once("--user").chain(args.iter().copied()).collect();
    run("systemctl", &args, TIMEOUT).map(|out| out.stdout).unwrap_or_default()
}

/// UUIDs in `org.gnome.shell enabled-extensions`.
pub fn enabled_extensions() -> Vec<String> {
    gsetting("org.gnome.shell", "enabled-extensions").map(|list| parse_string_list(&list)).unwrap_or_default()
}

/// The strings of a GVariant text `as`, e.g. `['a@b', 'c@d']` (extension
/// UUIDs never contain quotes).
pub fn parse_string_list(text: &str) -> Vec<String> {
    static ITEM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"'([^']*)'").unwrap());
    ITEM_RE.captures_iter(text).map(|c| c[1].to_string()).collect()
}

/// `gnome-extensions info` state of a loaded extension (e.g. "ACTIVE").
pub fn extension_state(uuid: &str) -> Option<String> {
    let out = run("gnome-extensions", &["info", uuid], TIMEOUT)?;
    out.stdout.lines().find_map(|l| l.trim().strip_prefix("State:").map(|s| s.trim().to_string()))
}

/// Disable and re-enable an extension, so it re-reads its files. Does
/// nothing if the extension is not enabled.
pub fn reload_extension(uuid: &str) -> Result<(), String> {
    if !enabled_extensions().iter().any(|u| u == uuid) {
        return Ok(());
    }
    for action in ["disable", "enable"] {
        if !run("gnome-extensions", &[action, uuid], TIMEOUT).is_some_and(|out| out.success) {
            return Err(format!("`gnome-extensions {action} {uuid}` failed"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_programs_on_path() {
        assert!(has_program("sh"));
        assert!(!has_program("surely-no-such-program-here"));
    }

    #[test]
    fn string_list() {
        assert_eq!(parse_string_list("['a@b', 'c@d']"), ["a@b", "c@d"]);
        assert_eq!(parse_string_list("@as []"), Vec::<String>::new());
    }

    #[test]
    fn large_output_does_not_deadlock() {
        // 1 MiB is far beyond the 64 KiB pipe buffer.
        let out = run("head", &["-c", "1048576", "/dev/zero"], Duration::from_secs(5)).expect("finishes");
        assert!(out.success);
    }

    #[test]
    fn timeout_kills_the_child() {
        let started = Instant::now();
        assert!(run("sleep", &["5"], Duration::from_millis(200)).is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
