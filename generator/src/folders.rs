//! `--folders`: the Adwaita-Brick folder icon theme.
//!
//! GNOME's red folder icons (Adwaita-Red, from the Accent Icons extension;
//! built into the binary from assets/icons, see build.rs) recoloured to the
//! accent, `ACCENT_BG` -- the same constant as the shell theme, so both always
//! match.
//!
//! The folder itself is dark (the accent) and the glyph inside it is light: the
//! reverse of Adwaita, where the folder is light and the glyph dark. All
//! shades are derived from the accent in OKLCh (see `Role::offset`).
//!
//! Every Adwaita-Red folder icon is drawn in the same order: back panel + tab
//! (#e62d42), bottom edge strip (gradient #e94a5c with #f4a2ab highlights),
//! front face (#f2939e), then the glyph (#e62d42 again). Back and glyph share
//! a colour, so they are told apart by position: anything after the front face
//! is the glyph. Icons without a front face (network-workgroup) and colours
//! without a role are shifted by the accent's offset from GNOME's red; greys
//! and other hues are kept.

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// static ADWAITA_RED: &[(&str, &[u8])] -- (path in the theme, contents)
include!(concat!(env!("OUT_DIR"), "/adwaita_red.rs"));

use regex::{NoExpand, Regex};

use crate::color::{self, ACCENT_BG};
use crate::css::try_replace_all;

pub const TARGET_THEME: &str = "Adwaita-Brick";
/// GNOME's red accent, the base colour of Adwaita-Red.
const SOURCE_ACCENT: &str = "#e62d42";

#[derive(Clone, Copy)]
enum Role {
    Front,
    Edge,
    EdgeHi,
    Back,
    Glyph,
}

impl Role {
    const ALL: [Role; 5] = [Role::Front, Role::Edge, Role::EdgeHi, Role::Back, Role::Glyph];

    fn name(self) -> &'static str {
        match self {
            Role::Front => "front",
            Role::Edge => "edge",
            Role::EdgeHi => "edge_hi",
            Role::Back => "back",
            Role::Glyph => "glyph",
        }
    }

    /// (lightness offset, chroma factor) relative to the accent in OKLCh.
    fn offset(self) -> (f64, f64) {
        match self {
            Role::Front => (0.0, 1.0),     // the accent itself
            Role::Edge => (-0.035, 1.0),   // bottom strip, a step darker than the front
            Role::EdgeHi => (0.03, 1.0),   // highlights at both ends of the strip
            Role::Back => (-0.07, 1.0),    // back panel and tab, darkest: keeps the depth
            Role::Glyph => (0.35, 0.55),   // light dusty rose on the dark front
        }
    }

    /// The role of an Adwaita-Red colour outside the glyph.
    fn of_source(hex: &str) -> Option<Role> {
        match hex {
            "#e62d42" => Some(Role::Back),
            "#e94a5c" => Some(Role::Edge),
            "#f4a2ab" => Some(Role::EdgeHi),
            "#f2939e" => Some(Role::Front),
            _ => None,
        }
    }
}

static HEX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#[0-9a-fA-F]{6}\b").unwrap());
// The front face; everything after it is the glyph.
static FRONT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r##"(?i)fill="#f2939e""##).unwrap());
static NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^Name=.*$").unwrap());
static HIDDEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^Hidden=true\n").unwrap());

type Lch = [f64; 3];

fn hex_to_lch(hex: &str) -> Result<Lch, String> {
    let [l, a, b] = color::rgb_to_oklab(color::hex_to_rgb(hex)?);
    Ok([l, a.hypot(b), b.atan2(a)])
}

fn lch_to_hex([l, c, h]: Lch) -> String {
    color::to_hex(color::oklab_to_rgb([l, c * h.cos(), c * h.sin()]))
}

pub struct FolderPalette {
    /// Indexed by `Role as usize`.
    shades: [String; Role::ALL.len()],
    source: Lch,
    target: Lch,
}

impl FolderPalette {
    pub fn new() -> Result<Self, String> {
        let source = hex_to_lch(SOURCE_ACCENT)?;
        let target = hex_to_lch(ACCENT_BG)?;
        let [tl, tc, th] = target;
        let shades = Role::ALL.map(|role| {
            let (dl, cf) = role.offset();
            lch_to_hex([tl + dl, tc * cf, th])
        });
        Ok(FolderPalette { shades, source, target })
    }

    /// (role name, colour) of every derived shade.
    pub fn named_shades(&self) -> impl Iterator<Item = (&'static str, &str)> {
        Role::ALL.into_iter().map(|role| (role.name(), self.shade(role)))
    }

    fn shade(&self, role: Role) -> &str {
        &self.shades[role as usize]
    }

    /// Colours without a role: same offset from GNOME's red as the accent has.
    fn shift(&self, hex: &str) -> Result<String, String> {
        let [l, c, h] = hex_to_lch(hex)?;
        let [sl, sc, sh] = self.source;
        let [tl, tc, th] = self.target;
        let hue_gap = ((h - sh + PI).rem_euclid(2.0 * PI) - PI).abs();
        if c < 0.06 || hue_gap > 0.6 {
            return Ok(hex.to_string()); // greys and other hues: keep
        }
        Ok(lch_to_hex([l + tl - sl, c * tc / sc, h + th - sh]))
    }

    fn recolor_svg(&self, text: &str) -> Result<String, String> {
        let split = FRONT_RE.find(text).map_or(text.len(), |m| m.end());
        let (folder, glyph) = text.split_at(split);
        let folder = try_replace_all(&HEX_RE, folder, |caps| {
            let hex = caps[0].to_ascii_lowercase();
            match Role::of_source(&hex) {
                Some(role) => Ok(self.shade(role).to_string()),
                None => self.shift(&hex),
            }
        })?;
        let glyph = try_replace_all(&HEX_RE, glyph, |caps| {
            let hex = caps[0].to_ascii_lowercase();
            if hex == SOURCE_ACCENT { Ok(self.shade(Role::Glyph).to_string()) } else { self.shift(&hex) }
        })?;
        Ok(folder + &glyph)
    }

    fn recolor_file(&self, path: &str, contents: &[u8]) -> Result<Vec<u8>, String> {
        let text = || std::str::from_utf8(contents).map_err(|e| format!("{path}: {e}"));
        Ok(if path.ends_with(".svg") {
            self.recolor_svg(text()?)?.into_bytes()
        } else if path == "index.theme" {
            let text = NAME_RE.replace_all(text()?, NoExpand(&format!("Name={TARGET_THEME}")));
            HIDDEN_RE.replace_all(&text, "").into_owned().into_bytes()
        } else {
            contents.to_vec()
        })
    }
}

/// Every file of the target theme: (path relative to the theme, contents).
fn render(pal: &FolderPalette) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    ADWAITA_RED.iter().map(|(path, contents)| Ok((PathBuf::from(path), pal.recolor_file(path, contents)?))).collect()
}

/// Rewrite `<icons_dir>/Adwaita-Brick` unless it is already current (None);
/// returns the number of files written. The new theme is built next to the
/// old one and swapped in by two renames, so the icon theme is never missing
/// for longer than that; on error the old one stays.
pub fn install(icons_dir: &Path, pal: &FolderPalette) -> Result<Option<usize>, String> {
    let files = render(pal)?;
    let target = icons_dir.join(TARGET_THEME);
    if matches(&target, &files) {
        return Ok(None); // unchanged: don't make every app reload its icons
    }
    let staging = icons_dir.join(format!(".{TARGET_THEME}.new"));
    let previous = icons_dir.join(format!(".{TARGET_THEME}.old"));
    let io_err = |e: std::io::Error| format!("cannot write {}: {e}", target.display());

    for dir in [&staging, &previous] {
        remove_dir_if_exists(dir).map_err(io_err)?;
    }
    let build = || -> std::io::Result<()> {
        for (rel, contents) in &files {
            let path = staging.join(rel);
            fs::create_dir_all(path.parent().expect("a file path has a parent"))?;
            fs::write(&path, contents)?;
        }
        Ok(())
    };
    if let Err(e) = build() {
        let _ = fs::remove_dir_all(&staging);
        return Err(io_err(e));
    }
    let had_previous = target.exists();
    if had_previous {
        fs::rename(&target, &previous).map_err(io_err)?;
    }
    if let Err(e) = fs::rename(&staging, &target) {
        if had_previous {
            let _ = fs::rename(&previous, &target); // put the old theme back
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(io_err(e));
    }
    remove_dir_if_exists(&previous).map_err(io_err)?;
    Ok(Some(files.len()))
}

/// Whether `<icons_dir>/Adwaita-Brick` is exactly what `install` would write.
pub fn is_current(icons_dir: &Path) -> Result<bool, String> {
    Ok(matches(&icons_dir.join(TARGET_THEME), &render(&FolderPalette::new()?)?))
}

fn matches(target: &Path, files: &[(PathBuf, Vec<u8>)]) -> bool {
    files.iter().all(|(rel, contents)| fs::read(target.join(rel)).is_ok_and(|c| &c == contents))
}

fn remove_dir_if_exists(dir: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    const SVG: &str = r##"<path fill="#E62D42"/><stop stop-color="#e94a5c"/><path fill="#f2939e"/><path fill="#E62D42"/>"##;

    #[test]
    fn all_lists_roles_in_declaration_order() {
        // `FolderPalette::shades` is indexed by `role as usize`.
        for (i, role) in Role::ALL.into_iter().enumerate() {
            assert_eq!(role as usize, i, "{}", role.name());
        }
    }

    #[test]
    fn front_is_the_accent() {
        assert_eq!(FolderPalette::new().unwrap().shade(Role::Front), ACCENT_BG);
    }

    #[test]
    fn back_and_glyph_are_told_apart_by_position() {
        let pal = FolderPalette::new().unwrap();
        let expected = format!(
            r#"<path fill="{}"/><stop stop-color="{}"/><path fill="{}"/><path fill="{}"/>"#,
            pal.shade(Role::Back),
            pal.shade(Role::Edge),
            pal.shade(Role::Front),
            pal.shade(Role::Glyph),
        );
        assert_eq!(pal.recolor_svg(SVG).unwrap(), expected);
    }

    #[test]
    fn greys_are_kept() {
        let pal = FolderPalette::new().unwrap();
        assert_eq!(pal.recolor_svg(r##"fill="#9a9996""##).unwrap(), r##"fill="#9a9996""##);
    }

    #[test]
    fn install_then_current_and_replaces_old_theme() {
        let icons = test_dir("folders-install");
        fs::create_dir_all(icons.join(TARGET_THEME)).unwrap();
        fs::write(icons.join(TARGET_THEME).join("stale.svg"), "old").unwrap();
        let pal = FolderPalette::new().unwrap();

        assert!(!is_current(&icons).unwrap());
        assert_eq!(install(&icons, &pal).unwrap(), Some(ADWAITA_RED.len()));
        assert!(is_current(&icons).unwrap());
        assert_eq!(install(&icons, &pal).unwrap(), None, "unchanged: not rewritten");
        assert!(!icons.join(TARGET_THEME).join("stale.svg").exists(), "old theme fully replaced");
        let index = fs::read_to_string(icons.join(TARGET_THEME).join("index.theme")).unwrap();
        assert!(index.contains(&format!("\nName={TARGET_THEME}\n")) && !index.contains("Hidden=true"), "{index}");
        assert!(!icons.join(format!(".{TARGET_THEME}.new")).exists());
        assert!(!icons.join(format!(".{TARGET_THEME}.old")).exists());
    }
}
