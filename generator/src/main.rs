//! Gruvbox Dark recolour of the *installed* GNOME Shell's own stylesheet, plus
//! everything else that makes up the look: matching folder icons, GTK 3/4,
//! Ptyxis and Firefox colours, and the desktop settings.
//!
//! GNOME Shell has no colour API (the accent is limited to GNOME's 9 presets
//! and everything else is hard-coded), so shell colours can only be changed by
//! a stylesheet. Instead of shipping a hand-made theme that goes stale with
//! every GNOME release, this tool takes the stock stylesheet straight out of
//! the installed gnome-shell and only swaps colours:
//!   * neutral greys  -> the gruvbox grey ramp (by lightness, so hover/active
//!                       steps stay distinct)
//!   * reds/oranges/… -> the matching gruvbox hue
//!   * the accent     -> ACCENT_BG for fills, a light shade of it for lines/text
//! Layout and widgets are byte-for-byte the stock ones.
//!
//! The result is loaded by the "Gruvbox Shell" extension
//! (gruvbox-shell@ex_gatito), which uses it *in place of* the stock
//! stylesheet. (The User Themes extension would load it *on top of* the stock
//! one, making the shell match every widget against both: slow first opens.)
//!
//! It runs at every login (systemd user unit gruvbox-shell-theme.service)
//! before GNOME Shell starts and rewrites the theme only when it changed.
//!
//! Safety: the generated stylesheet is checked against the stock one (same
//! selectors, same properties, in the same order; only colour values may
//! differ) before it is installed. If anything at all goes wrong -- missing or
//! changed GNOME files, a failed check, a write error, a bug (panic) -- the
//! generated stylesheet is removed, so the extension leaves the stock shell
//! look in place instead of loading a stylesheet that may not match the shell.
//!
//! Modules: `cli` (arguments), `paths` (locations), `theme` (the stylesheet:
//! `color` + `css` do the recolouring), `folders` (icon theme), `apps` (GTK,
//! Ptyxis, Firefox files; `firefox` finds profiles and the theme add-on),
//! `settings` (desktop settings), `check` (status report), `system` (GNOME
//! command-line tools), `fsutil` (safe file writing).

#![forbid(unsafe_code)]

mod apps;
mod check;
mod cli;
mod color;
mod css;
mod firefox;
mod folders;
mod fsutil;
mod paths;
mod settings;
mod system;
mod theme;

use std::panic::{self, AssertUnwindSafe};
use std::process::ExitCode;

use cli::Mode;
use paths::Paths;

/// Shared with extension/metadata.json and install.sh.
pub const EXTENSION_UUID: &str = "gruvbox-shell@ex_gatito";

const NAME: &str = "gruvbox-shell-theme";

fn main() -> ExitCode {
    let mode = match cli::parse(std::env::args().skip(1)) {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("{NAME}: {e}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    if mode == Mode::Help {
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }
    let paths = match Paths::from_env() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("{NAME}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match mode {
        Mode::Theme { force, reload } => run_theme(&paths, force, reload),
        Mode::Folders { dry_run } => run_folders(&paths, dry_run),
        Mode::Apps => run_steps("apps", || apps::install(&paths)),
        Mode::Settings => run_steps("settings", settings::apply),
        Mode::Check => match guarded(|| Ok(check::run(&paths))) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                println!("check failed: {e}");
                ExitCode::from(2)
            }
        },
        Mode::Help => ExitCode::SUCCESS, // handled above
    }
}

fn run_theme(paths: &Paths, force: bool, reload: bool) -> ExitCode {
    let result = guarded(|| {
        let rendered = theme::render()?;
        let written = theme::install(paths, &rendered, force)?;
        Ok((rendered.warnings, written))
    });
    match result {
        Ok((warnings, written)) => {
            for warning in warnings {
                eprintln!("{NAME}: warning: {warning}");
            }
            if !written {
                println!("{NAME}: up to date");
                return ExitCode::SUCCESS;
            }
            println!("{NAME}: wrote {}", paths.theme_css.display());
            if reload && let Err(e) = system::reload_extension(EXTENSION_UUID) {
                eprintln!("{NAME}: warning: {e}; log out and back in to see the new theme");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{NAME}: FAILED ({e}); falling back to the stock shell theme");
            theme::remove(paths);
            ExitCode::FAILURE
        }
    }
}

fn run_folders(paths: &Paths, dry_run: bool) -> ExitCode {
    // No fail-safe removal here: on any error the previous icons stay.
    let result = guarded(|| {
        let palette = folders::FolderPalette::new()?;
        for (role, colour) in palette.named_shades() {
            println!("  {role:8} {colour}");
        }
        if dry_run { Ok(None) } else { folders::install(&paths.icons_dir, &palette).map(Some) }
    });
    match result {
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(None)) => {
            println!("{NAME}: folder icons: up to date");
            ExitCode::SUCCESS
        }
        Ok(Some(Some(count))) => {
            println!("{NAME}: wrote {} ({count} files)", paths.icons_dir.join(folders::TARGET_THEME).display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{NAME}: FAILED to build the folder icons ({e})");
            ExitCode::FAILURE
        }
    }
}

/// Run a step that reports what it did, one line per item.
fn run_steps(what: &str, step: impl FnOnce() -> Result<Vec<String>, String>) -> ExitCode {
    match guarded(step) {
        Ok(lines) if lines.is_empty() => {
            println!("{NAME}: {what}: nothing to change");
            ExitCode::SUCCESS
        }
        Ok(lines) => {
            for line in lines {
                println!("{NAME}: {line}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{NAME}: FAILED to apply the {what} ({e})");
            ExitCode::FAILURE
        }
    }
}

/// Run `f`, turning a panic into an error, so that a bug still ends in the
/// caller's error handling (the fail-safe). Relies on `panic = "unwind"`.
fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown".into());
        Err(format!("panic: {message}"))
    })
}

/// A fresh, empty directory for one test (under $TMPDIR).
#[cfg(test)]
fn test_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("{NAME}-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarded_turns_panics_into_errors() {
        let result: Result<(), String> = guarded(|| panic!("boom"));
        assert_eq!(result, Err("panic: boom".into()));
        assert_eq!(guarded(|| Ok(1)), Ok(1));
    }
}
