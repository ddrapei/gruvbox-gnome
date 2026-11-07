//! Firefox: the profile(s) in use, and the Gruvbox theme add-on.
//!
//! Supports Fedora's Firefox package (profiles under ~/.mozilla/firefox or,
//! for newer installs, ~/.config/mozilla/firefox), not the Flatpak.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::paths::Paths;
use crate::system;

/// The "Gruvbox" theme on addons.mozilla.org. The file is pinned by its
/// SHA-256, so a changed or tampered download is refused.
pub const THEME_ID: &str = "{08d5243b-4236-4a27-984b-1ded22ce01c3}";
const THEME_URL: &str = "https://addons.mozilla.org/firefox/downloads/file/3729287/gruvboxgruvboxgruvboxgruvboxgr-1.0.xpi";
const THEME_SHA256: &str = "ca7f473e2ce1ed2f64aa7041f78c6fdd8b08b40aa2598ffede099fde7098f947";
const THEME_FILE: &str = "firefox-gruvbox-theme.xpi";

pub fn is_installed() -> bool {
    system::has_program("firefox")
}

/// The profile directories Firefox uses (existing ones only).
pub fn profiles(paths: &Paths) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in &paths.firefox_roots {
        for profile in profiles_in(root) {
            if profile.is_dir() && !found.contains(&profile) {
                found.push(profile);
            }
        }
    }
    found
}

/// The default profile of each Firefox installation (installs.ini or the
/// `[Install…]` sections of profiles.ini); for old setups without those, the
/// profile marked `Default=1`.
fn profiles_in(root: &Path) -> Vec<PathBuf> {
    let read = |name: &str| fs::read_to_string(root.join(name)).ok().map(|text| parse_ini(&text));
    let resolve = |path: &str| if Path::new(path).is_absolute() { PathBuf::from(path) } else { root.join(path) };
    let install_defaults = |ini: &Ini, prefix: &str| -> Vec<PathBuf> {
        ini.iter().filter(|(section, _)| section.starts_with(prefix)).filter_map(|(_, keys)| get(keys, "Default")).map(resolve).collect()
    };

    if let Some(installs) = read("installs.ini") {
        let defaults = install_defaults(&installs, "");
        if !defaults.is_empty() {
            return defaults;
        }
    }
    let Some(profiles) = read("profiles.ini") else { return Vec::new() };
    let defaults = install_defaults(&profiles, "Install");
    if !defaults.is_empty() {
        return defaults;
    }
    profiles
        .iter()
        .filter(|(section, keys)| section.starts_with("Profile") && get(keys, "Default") == Some("1"))
        .filter_map(|(_, keys)| {
            let path = get(keys, "Path")?;
            Some(if get(keys, "IsRelative") == Some("0") { PathBuf::from(path) } else { root.join(path) })
        })
        .collect()
}

type Ini = Vec<(String, Vec<(String, String)>)>;

/// Minimal INI reader: `[section]` and `key=value` lines.
fn parse_ini(text: &str) -> Ini {
    let mut sections: Ini = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push((name.to_string(), Vec::new()));
        } else if let (Some((key, value)), Some((_, keys))) = (line.split_once('='), sections.last_mut()) {
            keys.push((key.trim().to_string(), value.trim().to_string()));
        }
    }
    sections
}

fn get<'a>(keys: &'a [(String, String)], key: &str) -> Option<&'a str> {
    keys.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

pub fn theme_installed(profile: &Path) -> bool {
    profile.join("extensions").join(format!("{THEME_ID}.xpi")).is_file()
}

/// Download (or reuse) the verified theme add-on and open it in Firefox, which
/// asks the user to confirm the installation. Returns what to tell the user.
pub fn offer_theme(paths: &Paths) -> Result<String, String> {
    let xpi = paths.cache_dir.join(THEME_FILE);
    if !has_expected_hash(&xpi) {
        download(&xpi)?;
    }
    if !is_installed() {
        return Ok(format!("open {} in Firefox to install the Gruvbox theme", xpi.display()));
    }
    system::spawn_detached("firefox", &[xpi.as_os_str()]).map_err(|e| format!("cannot start Firefox: {e}"))?;
    Ok("opened the Gruvbox theme in Firefox: click \"Add\" to install it".into())
}

fn has_expected_hash(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| sha256_hex(&bytes) == THEME_SHA256)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn download(dest: &Path) -> Result<(), String> {
    let dir = dest.parent().expect("cache file has a parent");
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let part = dest.with_extension("xpi.part");
    let part_str = part.to_str().ok_or("cache path is not UTF-8")?;
    let downloaded = system::run(
        "curl",
        &["--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--max-time", "60", "--output", part_str, THEME_URL],
        Duration::from_secs(90),
    )
    .is_some_and(|out| out.success);
    let verified = downloaded && has_expected_hash(&part);
    if !verified {
        let _ = fs::remove_file(&part);
        return Err(if downloaded {
            format!("the Firefox theme from {THEME_URL} does not match its pinned checksum; refusing to install it")
        } else {
            format!("cannot download the Firefox theme from {THEME_URL}")
        });
    }
    fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    fn profiles_for(test: &str, files: &[(&str, &str)], dirs: &[&str]) -> Vec<String> {
        let home = test_dir(test);
        let root = home.join(".mozilla/firefox");
        fs::create_dir_all(&root).unwrap();
        for dir in dirs {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        for (name, text) in files {
            fs::write(root.join(name), text).unwrap();
        }
        let paths = Paths::for_test(&home);
        profiles(&paths).iter().map(|p| p.strip_prefix(&root).unwrap().display().to_string()).collect()
    }

    #[test]
    fn installs_ini_wins_over_profiles_ini() {
        let found = profiles_for(
            "firefox-installs-ini",
            &[
                ("installs.ini", "[11457493C5A56847]\nDefault=dlkc.default-release\nLocked=1\n"),
                ("profiles.ini", "[Profile1]\nName=default\nIsRelative=1\nPath=8qtd.default\nDefault=1\n"),
            ],
            &["dlkc.default-release", "8qtd.default"],
        );
        assert_eq!(found, ["dlkc.default-release"]);
    }

    #[test]
    fn falls_back_to_the_default_profile() {
        let found = profiles_for("firefox-default-profile", &[("profiles.ini", "[Profile0]\nPath=a.default\nIsRelative=1\nDefault=1\n[Profile1]\nPath=b\n")], &["a.default", "b"]);
        assert_eq!(found, ["a.default"]);
    }

    #[test]
    fn missing_directories_are_ignored() {
        assert!(profiles_for("firefox-missing", &[("installs.ini", "[X]\nDefault=gone\n")], &[]).is_empty());
    }

    #[test]
    fn sha256() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
