//! Where everything lives.
//!
//! Shared contracts: the extension (extension/src/extension.ts) reads the
//! stylesheet from `<data home>/themes/Gruvbox-Shell/gnome-shell/gnome-shell.css`;
//! install.sh builds the GTK 3 theme into `~/.themes` and applies the same
//! XDG rules as here.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::EXTENSION_UUID;

pub const THEME_NAME: &str = "Gruvbox-Shell";

pub struct Paths {
    /// The generated shell stylesheet.
    pub theme_css: PathBuf,
    /// Where install.sh puts the extension.
    pub extension_dir: PathBuf,
    /// Icon themes (Adwaita-Brick is written here).
    pub icons_dir: PathBuf,
    /// GTK 3 themes (`~/.themes`, where the Gruvbox-Dark GTK theme installs).
    pub gtk_themes_dir: PathBuf,
    pub gtk3_css: PathBuf,
    pub gtk4_css: PathBuf,
    pub ptyxis_palettes_dir: PathBuf,
    /// Firefox profile roots: the XDG location newer Firefox uses for new
    /// profiles, then the classic one.
    pub firefox_roots: Vec<PathBuf>,
    /// Downloads (the Firefox theme add-on).
    pub cache_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Self, String> {
        let fallback_home = std::env::home_dir().map(PathBuf::into_os_string);
        Self::resolve(|name| std::env::var_os(name), fallback_home)
    }

    /// XDG base directory spec: an `XDG_*_HOME` variable counts only if it is
    /// an absolute path; otherwise the default under the home directory is used.
    fn resolve(var: impl Fn(&str) -> Option<OsString>, fallback_home: Option<OsString>) -> Result<Self, String> {
        let absolute = |value: Option<OsString>| value.map(PathBuf::from).filter(|p| p.is_absolute());
        let home = absolute(var("HOME").filter(|h| !h.is_empty()).or(fallback_home))
            .ok_or("cannot find the home directory (HOME is unset or not an absolute path)")?;
        let xdg = |name: &str, default: &str| absolute(var(name)).unwrap_or_else(|| home.join(default));
        let data = xdg("XDG_DATA_HOME", ".local/share");
        let config = xdg("XDG_CONFIG_HOME", ".config");
        let cache = xdg("XDG_CACHE_HOME", ".cache");
        Ok(Paths {
            theme_css: data.join("themes").join(THEME_NAME).join("gnome-shell/gnome-shell.css"),
            extension_dir: data.join("gnome-shell/extensions").join(EXTENSION_UUID),
            icons_dir: data.join("icons"),
            gtk_themes_dir: home.join(".themes"),
            gtk3_css: config.join("gtk-3.0/gtk.css"),
            gtk4_css: config.join("gtk-4.0/gtk.css"),
            ptyxis_palettes_dir: data.join("org.gnome.Ptyxis/palettes"),
            firefox_roots: vec![config.join("mozilla/firefox"), home.join(".mozilla/firefox")],
            cache_dir: cache.join("gruvbox-shell"),
        })
    }

    /// Everything under one directory, as if it were the home directory.
    #[cfg(test)]
    pub fn for_test(home: &std::path::Path) -> Self {
        Self::resolve(|_| None, Some(home.as_os_str().to_owned())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(vars: &[(&str, &str)]) -> Result<Paths, String> {
        let var = |name: &str| vars.iter().find(|(n, _)| *n == name).map(|(_, v)| OsString::from(v));
        Paths::resolve(var, None)
    }

    #[test]
    fn xdg_variables_must_be_absolute() {
        let home = [("HOME", "/home/u")];
        assert_eq!(resolve(&home).unwrap().icons_dir, PathBuf::from("/home/u/.local/share/icons"));
        let p = resolve(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "/data"), ("XDG_CONFIG_HOME", "/cfg")]).unwrap();
        assert_eq!(p.icons_dir, PathBuf::from("/data/icons"));
        assert_eq!(p.gtk4_css, PathBuf::from("/cfg/gtk-4.0/gtk.css"));
        assert_eq!(p.firefox_roots[0], PathBuf::from("/cfg/mozilla/firefox"));
        for bad in ["rel/dir", ""] {
            let p = resolve(&[("HOME", "/home/u"), ("XDG_DATA_HOME", bad), ("XDG_CONFIG_HOME", bad)]).unwrap();
            assert_eq!(p.icons_dir, PathBuf::from("/home/u/.local/share/icons"));
            assert_eq!(p.gtk3_css, PathBuf::from("/home/u/.config/gtk-3.0/gtk.css"));
        }
    }

    #[test]
    fn no_silent_fallback_without_a_home() {
        assert!(resolve(&[]).is_err());
        assert!(resolve(&[("HOME", "relative")]).is_err());
    }
}
