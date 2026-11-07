//! Command-line parsing.

pub const USAGE: &str = "\
Usage:  gruvbox-shell-theme [--force] [--reload]
        gruvbox-shell-theme --folders [--dry-run]
        gruvbox-shell-theme --apps
        gruvbox-shell-theme --settings
        gruvbox-shell-theme --check
  (none)      regenerate the shell stylesheet if it no longer matches the
              installed GNOME Shell or the colours (run at every login)
  --force     rewrite it even if it already matches
  --reload    make the running shell re-read it if it was rewritten
  --folders   rebuild the Adwaita-Brick folder icons in the accent colour;
              --dry-run only prints the derived colours
  --apps      write the GTK 3/4 colours, the Ptyxis palette and the Firefox
              files, and offer the Firefox theme add-on if it is missing
  --settings  apply the desktop settings (dark style, icon and GTK theme,
              extensions, Ptyxis palette, Flatpak access)
  --check     report whether everything is in place and up to date (changes
              nothing; exit code 0 = all good)

Undo:   gnome-extensions disable gruvbox-shell@ex_gatito
        (optionally: systemctl --user disable gruvbox-shell-theme.service)";

/// What to do. Each variant carries only the options that apply to it, so
/// contradictory combinations cannot be represented.
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Theme { force: bool, reload: bool },
    Folders { dry_run: bool },
    Apps,
    Settings,
    Check,
    Help,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Mode, String> {
    let (mut force, mut reload, mut dry_run) = (false, false, false);
    let mut mode_flag: Option<String> = None;
    for arg in args {
        match arg.as_str() {
            "--force" => force = true,
            "--reload" => reload = true,
            "--dry-run" => dry_run = true,
            "--folders" | "--apps" | "--settings" | "--check" => {
                if let Some(previous) = &mode_flag {
                    return Err(format!("{previous} and {arg} are separate modes"));
                }
                mode_flag = Some(arg);
            }
            "-h" | "--help" => return Ok(Mode::Help),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if (force || reload) && mode_flag.is_some() {
        return Err("--force and --reload only apply to the stylesheet (no mode flag)".into());
    }
    if dry_run && mode_flag.as_deref() != Some("--folders") {
        return Err("--dry-run only works with --folders".into());
    }
    Ok(match mode_flag.as_deref() {
        None => Mode::Theme { force, reload },
        Some("--folders") => Mode::Folders { dry_run },
        Some("--apps") => Mode::Apps,
        Some("--settings") => Mode::Settings,
        Some(_) => Mode::Check,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(args: &[&str]) -> Result<Mode, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn modes() {
        assert_eq!(parse_str(&[]), Ok(Mode::Theme { force: false, reload: false }));
        assert_eq!(parse_str(&["--reload", "--force"]), Ok(Mode::Theme { force: true, reload: true }));
        assert_eq!(parse_str(&["--folders", "--dry-run"]), Ok(Mode::Folders { dry_run: true }));
        assert_eq!(parse_str(&["--apps"]), Ok(Mode::Apps));
        assert_eq!(parse_str(&["--settings"]), Ok(Mode::Settings));
        assert_eq!(parse_str(&["--check"]), Ok(Mode::Check));
        assert_eq!(parse_str(&["--check", "--help"]), Ok(Mode::Help));
    }

    #[test]
    fn rejects_contradictions_and_unknown_arguments() {
        let bad: [&[&str]; 7] = [
            &["--check", "--folders"],
            &["--apps", "--settings"],
            &["--folders", "--force"],
            &["--check", "--reload"],
            &["--dry-run"],
            &["--apps", "--dry-run"],
            &["-x"],
        ];
        for args in bad {
            assert!(parse_str(args).is_err(), "{args:?} should be rejected");
        }
    }
}
