//! `--check`: report whether everything is in place. Changes nothing.
//!
//! Modules describe their own state as `Status` lines; this module only
//! collects and prints them.

use crate::paths::Paths;
use crate::{EXTENSION_UUID, apps, folders, settings, system, theme};

const SERVICE: &str = "gruvbox-shell-theme.service";
const INSTALL: &str = "run ~/Projects/gruvbox-shell/install.sh";

/// One line of the report.
pub struct Status {
    ok: bool,
    text: String,
    /// How to fix it if not ok ("" = no hint).
    fix: String,
}

impl Status {
    pub fn new(ok: bool, text: impl Into<String>, fix: impl Into<String>) -> Self {
        Status { ok, text: text.into(), fix: fix.into() }
    }
}

#[derive(Default)]
struct Report {
    problems: usize,
}

impl Report {
    fn section(&self, title: &str) {
        println!("{title}");
    }

    fn add(&mut self, status: Status) {
        println!("  [{}] {}", if status.ok { "ok" } else { "!!" }, status.text);
        if !status.ok {
            if !status.fix.is_empty() {
                println!("       fix: {}", status.fix);
            }
            self.problems += 1;
        }
    }

    fn note(&self, text: &str) {
        println!("  [--] note: {text}");
    }
}

/// Print the report; true if there are no problems.
pub fn run(paths: &Paths) -> bool {
    let mut report = Report::default();
    report.section(&format!("Shell ({})", system::shell_version()));
    stylesheet(&mut report, paths);
    extension(&mut report, paths);
    login_service(&mut report);
    report.add(match folders::is_current(&paths.icons_dir) {
        Ok(current) => Status::new(current, format!("folder icons ({}) match the accent", folders::TARGET_THEME), "gruvbox-shell-theme --folders"),
        Err(e) => Status::new(false, format!("cannot check the folder icons: {e}"), ""),
    });
    report.section("Apps");
    apps::status(paths).into_iter().for_each(|s| report.add(s));
    report.section("Settings");
    settings::status(paths).into_iter().for_each(|s| report.add(s));

    if report.problems == 0 {
        println!("all good");
    } else {
        println!("{} problem(s) found", report.problems);
    }
    report.problems == 0
}

fn stylesheet(report: &mut Report, paths: &Paths) {
    match theme::render() {
        Ok(rendered) => {
            report.add(Status::new(
                theme::is_current(paths, &rendered),
                "generated theme matches the installed GNOME Shell and the colours",
                "gruvbox-shell-theme --reload",
            ));
            for warning in &rendered.warnings {
                report.note(warning);
            }
        }
        Err(e) => report.add(Status::new(false, format!("cannot generate the theme: {e}"), "")),
    }
}

fn extension(report: &mut Report, paths: &Paths) {
    let installed = ["metadata.json", "extension.js"].iter().all(|f| paths.extension_dir.join(f).is_file());
    report.add(Status::new(installed, format!("extension {EXTENSION_UUID} is installed"), INSTALL));
    let state = system::extension_state(EXTENSION_UUID).unwrap_or_else(|| "unknown".into());
    report.add(Status::new(
        state == "ACTIVE",
        format!("extension is running in the shell (state: {state})"),
        "new extensions are only picked up at login: log out and back in \
         (if it says ERROR: journalctl --user -b | grep gruvbox-shell)",
    ));
}

fn login_service(report: &mut Report) {
    report.add(Status::new(system::systemctl_user(&["is-enabled", SERVICE]) == "enabled", "login service is enabled", INSTALL));
    let result = system::systemctl_user(&["show", SERVICE, "-p", "Result", "--value"]);
    report.add(Status::new(
        result == "success",
        format!("last login-service run: {}", if result.is_empty() { "unknown" } else { &result }),
        format!("journalctl --user -u {SERVICE}"),
    ));
}
