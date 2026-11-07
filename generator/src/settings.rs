//! `--settings`: the desktop settings that make up the look. Each is described
//! once, here, and used both to apply it and by `--check`.

use std::time::Duration;

use crate::check::Status;
use crate::paths::Paths;
use crate::{EXTENSION_UUID, apps, folders, system};

/// The GTK 3 theme; install.sh builds it into ~/.themes under this name.
pub const GTK_THEME: &str = "Gruvbox-Dark";
const INTERFACE: &str = "org.gnome.desktop.interface";
const FIX: &str = "gruvbox-shell-theme --settings";

/// A GSettings key and its wanted value (GVariant text).
struct Setting {
    schema: &'static str,
    key: &'static str,
    value: String,
    what: &'static str,
}

fn settings() -> Vec<Setting> {
    let quoted = |s: &str| format!("'{s}'");
    vec![
        Setting { schema: INTERFACE, key: "color-scheme", value: quoted("prefer-dark"), what: "dark style" },
        Setting {
            schema: INTERFACE,
            key: "accent-color",
            value: quoted("red"),
            what: "GNOME accent preset nearest to the brick red, for apps that follow it",
        },
        Setting { schema: INTERFACE, key: "icon-theme", value: quoted(folders::TARGET_THEME), what: "brick-red folder icons" },
        Setting { schema: INTERFACE, key: "gtk-theme", value: quoted(GTK_THEME), what: "GTK 3 app theme" },
        Setting { schema: "org.gnome.shell", key: "disable-user-extensions", value: "false".into(), what: "user extensions allowed" },
    ]
}

const EXTENSIONS_ON: &[&str] = &[EXTENSION_UUID];
const EXTENSIONS_OFF: &[(&str, &str)] = &[
    ("user-theme@gnome-shell-extensions.gcampax.github.com", "User Themes would load the stylesheet a second time"),
    ("accent-directories@taiwbi.com", "Accent Icons would switch the icon theme on every lock and unlock"),
];

/// `current` with the wanted extensions on and the conflicting ones off;
/// everything else (and the order) is kept.
fn wanted_extensions(current: &[String]) -> Vec<String> {
    let mut wanted: Vec<String> =
        current.iter().filter(|uuid| !EXTENSIONS_OFF.iter().any(|(off, _)| off == uuid)).cloned().collect();
    for on in EXTENSIONS_ON {
        if !wanted.iter().any(|uuid| uuid == on) {
            wanted.push((*on).to_string());
        }
    }
    wanted
}

/// GVariant text of a string array (the items never contain quotes: they
/// are extension UUIDs / Ptyxis profile ids).
fn string_list(items: &[String]) -> String {
    if items.is_empty() {
        return "@as []".into();
    }
    format!("[{}]", items.iter().map(|i| format!("'{i}'")).collect::<Vec<_>>().join(", "))
}

// --- Ptyxis -------------------------------------------------------------------

const PTYXIS: &str = "org.gnome.Ptyxis";

fn ptyxis_profile_schema(uuid: &str) -> String {
    format!("{PTYXIS}.Profile:/org/gnome/Ptyxis/Profiles/{uuid}/")
}

fn unquote(value: &str) -> String {
    value.trim_matches('\'').to_string()
}

/// The default Ptyxis profile: None if Ptyxis is not installed, Some("") if
/// it has never been started (no profile yet).
fn ptyxis_default_profile() -> Option<String> {
    system::gsetting(PTYXIS, "default-profile-uuid").map(|v| unquote(&v))
}

fn ptyxis_palette(uuid: &str) -> Option<String> {
    system::gsetting(&ptyxis_profile_schema(uuid), "palette").map(|v| unquote(&v))
}

/// A new profile id in Ptyxis' format (a UUID without dashes).
fn new_ptyxis_profile_id() -> Result<String, String> {
    let uuid = std::fs::read_to_string("/proc/sys/kernel/random/uuid").map_err(|e| format!("cannot make a profile id: {e}"))?;
    let id: String = uuid.trim().chars().filter(|c| *c != '-').collect();
    if id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit()) { Ok(id) } else { Err(format!("unexpected UUID {uuid:?}")) }
}

fn apply_ptyxis(changes: &mut Vec<String>) -> Result<(), String> {
    let Some(mut uuid) = ptyxis_default_profile() else {
        changes.push("Ptyxis is not installed: skipped its palette".into());
        return Ok(());
    };
    if uuid.is_empty() {
        // Ptyxis was never started: create the default profile it would create.
        let mut ids = system::gsetting(PTYXIS, "profile-uuids").map(|v| system::parse_string_list(&v)).unwrap_or_default();
        uuid = match ids.first() {
            Some(first) => first.clone(),
            None => {
                let id = new_ptyxis_profile_id()?;
                ids.push(id.clone());
                system::gsetting_set(PTYXIS, "profile-uuids", &string_list(&ids))?;
                id
            }
        };
        system::gsetting_set(PTYXIS, "default-profile-uuid", &format!("'{uuid}'"))?;
        changes.push(format!("created the default Ptyxis profile {uuid}"));
    }
    if ptyxis_palette(&uuid).as_deref() != Some(apps::PTYXIS_PALETTE_ID) {
        system::gsetting_set(&ptyxis_profile_schema(&uuid), "palette", &format!("'{}'", apps::PTYXIS_PALETTE_ID))?;
        changes.push(format!("Ptyxis palette set to {}", apps::PTYXIS_PALETTE_ID));
    }
    Ok(())
}

// --- Flatpak ------------------------------------------------------------------

/// Lets Flatpak apps read the GTK 4 colours and the themes.
const FLATPAK_FILESYSTEMS: &[&str] = &["xdg-config/gtk-4.0:ro", "~/.themes:ro", "~/.local/share/themes:ro"];

/// The `--filesystem` overrides that apply to all Flatpak apps; None if
/// Flatpak is not installed.
fn flatpak_filesystems() -> Option<Vec<String>> {
    let out = system::run("flatpak", &["override", "--user", "--show"], Duration::from_secs(10)).filter(|o| o.success)?;
    Some(
        out.stdout
            .lines()
            .find_map(|l| l.trim().strip_prefix("filesystems="))
            .map(|list| list.split(';').filter(|s| !s.is_empty()).map(String::from).collect())
            .unwrap_or_default(),
    )
}

fn missing_flatpak_filesystems(present: &[String]) -> Vec<&'static str> {
    FLATPAK_FILESYSTEMS.iter().copied().filter(|fs| !present.iter().any(|p| p == fs)).collect()
}

fn apply_flatpak(changes: &mut Vec<String>) -> Result<(), String> {
    let Some(present) = flatpak_filesystems() else { return Ok(()) };
    let missing = missing_flatpak_filesystems(&present);
    if missing.is_empty() {
        return Ok(());
    }
    let args: Vec<String> = missing.iter().map(|fs| format!("--filesystem={fs}")).collect();
    let mut argv = vec!["override", "--user"];
    argv.extend(args.iter().map(String::as_str));
    if !system::run("flatpak", &argv, Duration::from_secs(10)).is_some_and(|o| o.success) {
        return Err("`flatpak override --user` failed".into());
    }
    changes.push(format!("Flatpak apps may now read {}", missing.join(", ")));
    Ok(())
}

// --- apply / status -----------------------------------------------------------

/// Apply everything; returns what changed.
pub fn apply() -> Result<Vec<String>, String> {
    let mut changes = Vec::new();
    for s in settings() {
        if system::gsetting(s.schema, s.key).as_deref() != Some(s.value.as_str()) {
            system::gsetting_set(s.schema, s.key, &s.value)?;
            changes.push(format!("{} {} = {} ({})", s.schema, s.key, s.value, s.what));
        }
    }
    let current = system::enabled_extensions();
    let wanted = wanted_extensions(&current);
    if wanted != current {
        system::gsetting_set("org.gnome.shell", "enabled-extensions", &string_list(&wanted))?;
        changes.push(format!("enabled extensions: {}", wanted.join(", ")));
    }
    apply_ptyxis(&mut changes)?;
    apply_flatpak(&mut changes)?;
    Ok(changes)
}

/// For `--check`.
pub fn status(paths: &Paths) -> Vec<Status> {
    let mut statuses: Vec<Status> = settings()
        .into_iter()
        .map(|s| {
            let now = system::gsetting(s.schema, s.key).unwrap_or_else(|| "unknown".into());
            Status::new(now == s.value, format!("{}: {} is {} (now: {now})", s.what, s.key, s.value), FIX)
        })
        .collect();

    let theme_css = paths.gtk_themes_dir.join(GTK_THEME).join("gtk-3.0/gtk.css");
    statuses.push(Status::new(theme_css.is_file(), format!("GTK 3 theme {GTK_THEME} is installed"), "install.sh --setup"));

    let enabled = system::enabled_extensions();
    for on in EXTENSIONS_ON {
        statuses.push(Status::new(enabled.iter().any(|u| u == on), format!("extension {on} is enabled"), FIX));
    }
    for (off, why) in EXTENSIONS_OFF {
        statuses.push(Status::new(!enabled.iter().any(|u| u == off), format!("not enabled: {off} ({why})"), FIX));
    }

    match ptyxis_default_profile() {
        None => {}
        Some(uuid) if uuid.is_empty() => statuses.push(Status::new(false, "Ptyxis has a default profile", FIX)),
        Some(uuid) => {
            let palette = ptyxis_palette(&uuid).unwrap_or_default();
            let ok = palette == apps::PTYXIS_PALETTE_ID;
            statuses.push(Status::new(ok, format!("Ptyxis uses the {} palette (now: {palette})", apps::PTYXIS_PALETTE_ID), FIX));
        }
    }
    if let Some(present) = flatpak_filesystems() {
        let missing = missing_flatpak_filesystems(&present);
        statuses.push(Status::new(missing.is_empty(), "Flatpak apps may read the GTK 4 colours and the themes", FIX));
    }
    statuses
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn extensions_keep_others_and_order() {
        let current = strings(&["copyous@boerdereinar.dev", "user-theme@gnome-shell-extensions.gcampax.github.com", "x@y"]);
        assert_eq!(wanted_extensions(&current), strings(&["copyous@boerdereinar.dev", "x@y", EXTENSION_UUID]));
        let done = wanted_extensions(&current);
        assert_eq!(wanted_extensions(&done), done, "idempotent");
    }

    #[test]
    fn gvariant_string_lists() {
        assert_eq!(string_list(&strings(&["a@b", "c@d"])), "['a@b', 'c@d']");
        assert_eq!(string_list(&[]), "@as []");
        assert_eq!(system::parse_string_list(&string_list(&strings(&["a@b"]))), strings(&["a@b"]));
    }

    #[test]
    fn flatpak_missing_filesystems() {
        assert_eq!(missing_flatpak_filesystems(&strings(FLATPAK_FILESYSTEMS)), Vec::<&str>::new());
        assert_eq!(missing_flatpak_filesystems(&strings(&["~/.themes:ro"])), ["xdg-config/gtk-4.0:ro", "~/.local/share/themes:ro"]);
    }

    #[test]
    fn ptyxis_profile_ids_look_like_ptyxis_ones() {
        let id = new_ptyxis_profile_id().unwrap();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
