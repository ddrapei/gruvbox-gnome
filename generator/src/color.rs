//! Colour maths (OKLab) and the stock -> gruvbox palette.
//!
//! The arithmetic mirrors the original Python generator this was ported from,
//! operation for operation (`powf` where Python used `**`, ties-to-even
//! rounding like Python's `round()`); the port was verified to produce
//! byte-identical stylesheets.

use std::f64::consts::PI;

pub type Rgb = [f64; 3];
pub type Lab = [f64; 3];

// Accent, used for solid fills (checked toggles, switches, slider fill):
// brick red (a touch redder than the Ptyxis sudo/root header bar, #5e2724).
// Same accent as ~/.config/gtk-{3,4}.0/gtk.css; also the base of the
// Adwaita-Brick folder icons (--folders).
pub const ACCENT_BG: &str = "#5e2424";
// Text/icons drawn on top of ACCENT_BG: gruvbox fg (8.7:1 on the brick red;
// use "#282828" for a light accent).
pub const ACCENT_FG: &str = "#ebdbb2";
// Light shade of the accent for everything that must stand out against the
// dark panels: focus rings, borders (e.g. active workspace), links, text
// selection and translucent highlights. None = derived from ACCENT_BG the way
// libadwaita derives its accent text colour (OKLab lightness raised to 0.85).
pub const ACCENT_LINE: Option<&str> = None;
// Solid black (only the top bar uses it) -> gruvbox bg0_h, like Firefox's toolbar.
pub const SOLID_BLACK: &str = "#1d2021";

// Stock grey -> gruvbox grey. Same mapping as the GTK palette files
// (#222226 window -> #282828, #2e2e32 headerbar -> #32302f, ...).
// Greys in between are interpolated by lightness.
const RAMP: &[(&str, &str)] = &[
    ("#000000", "#000000"),
    ("#1d1d20", "#1d2021"),
    ("#222226", "#282828"),
    ("#2e2e33", "#32302f"),
    ("#36363a", "#3c3836"),
    ("#47474c", "#504945"),
    ("#5e5c64", "#665c54"),
    ("#77767b", "#7c6f64"),
    ("#9a9996", "#a89984"),
    ("#c0bfbc", "#bdae93"),
    ("#deddda", "#d5c4a1"),
    ("#fafafb", "#ebdbb2"),
    ("#ffffff", "#ebdbb2"),
];

// Stock hue -> gruvbox hue. Other shades of a hue are shifted by the same
// amount, so lighter/darker variants keep their relative difference.
const FAMILIES: &[(&str, &str)] = &[
    ("#c01c28", "#cc241d"), // red (destructive, recording)
    ("#ff7800", "#fe8019"), // orange (privacy indicators)
    ("#cd9309", "#fabd2f"), // yellow (warnings)
    ("#3584e4", "#458588"), // blue
    ("#2ec27e", "#98971a"), // green
    ("#2190a4", "#689d6a"), // teal
    ("#9141ac", "#b16286"), // purple
    ("#d56199", "#d3869b"), // pink
];
const NEUTRAL_CHROMA: f64 = 0.04;

pub fn hex_to_rgb(hex: &str) -> Result<Rgb, String> {
    let h = hex.trim_start_matches('#');
    let h: String = if h.chars().count() == 3 {
        h.chars().flat_map(|c| [c, c]).collect()
    } else {
        h.to_string()
    };
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("not a hex colour: #{h}"));
    }
    let channel = |i: usize| f64::from(u8::from_str_radix(&h[i..i + 2], 16).unwrap()) / 255.0;
    Ok([channel(0), channel(2), channel(4)])
}

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(c: f64) -> f64 {
    let c = if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    c.max(0.0).min(1.0)
}

pub fn rgb_to_oklab(rgb: Rgb) -> Lab {
    let [r, g, b] = rgb.map(to_linear);
    let third = 1.0 / 3.0;
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).powf(third);
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).powf(third);
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).powf(third);
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

pub fn oklab_to_rgb(lab: Lab) -> Rgb {
    let [ll, a, b] = lab;
    let l = (ll + 0.3963377774 * a + 0.2158037573 * b).powf(3.0);
    let m = (ll - 0.1055613458 * a - 0.0638541728 * b).powf(3.0);
    let s = (ll - 0.0894841775 * a - 1.2914855480 * b).powf(3.0);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
    .map(from_linear)
}

fn hue(lab: Lab) -> f64 {
    lab[2].atan2(lab[1])
}

fn hue_dist(h1: f64, h2: f64) -> f64 {
    let d = (h1 - h2).abs() % (2.0 * PI);
    d.min(2.0 * PI - d)
}

/// 0..1 channel -> 0..255, rounded half-to-even like Python's `round()`.
pub fn to_byte(c: f64) -> u8 {
    (c * 255.0).round_ties_even() as u8
}

pub fn to_hex(rgb: Rgb) -> String {
    let [r, g, b] = rgb.map(to_byte);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// All derived colour data; built inside the fail-safe (bad settings fail safe).
pub struct Palette {
    /// (stock lightness, gruvbox colour), sorted by lightness.
    ramp: Vec<(f64, Lab)>,
    /// (stock hue colour, gruvbox hue colour).
    families: Vec<(Lab, Lab)>,
    pub accent_line: String,
}

impl Palette {
    pub fn new() -> Result<Self, String> {
        for value in [ACCENT_BG, ACCENT_FG, SOLID_BLACK]
            .into_iter()
            .chain(RAMP.iter().chain(FAMILIES).flat_map(|&(s, t)| [s, t]))
        {
            hex_to_rgb(value)?;
        }
        let lab = |hex: &str| hex_to_rgb(hex).map(rgb_to_oklab);
        let mut ramp = RAMP
            .iter()
            .map(|&(s, t)| Ok((lab(s)?[0], lab(t)?)))
            .collect::<Result<Vec<_>, String>>()?;
        ramp.sort_by(|a, b| a.0.total_cmp(&b.0));
        let families = FAMILIES
            .iter()
            .map(|&(s, t)| Ok((lab(s)?, lab(t)?)))
            .collect::<Result<Vec<_>, String>>()?;
        let accent_line = match ACCENT_LINE {
            Some(line) => {
                hex_to_rgb(line)?;
                line.to_string()
            }
            None => {
                let [l, a, b] = lab(ACCENT_BG)?;
                to_hex(oklab_to_rgb([l.max(0.85), a, b]))
            }
        };
        Ok(Palette { ramp, families, accent_line })
    }

    pub fn map_rgb(&self, rgb: Rgb) -> Rgb {
        let lab = rgb_to_oklab(rgb);
        let chroma = (lab[1].powf(2.0) + lab[2].powf(2.0)).powf(0.5);
        let out = if chroma < NEUTRAL_CHROMA {
            let l = lab[0];
            // Nearest ramp stops below and above (first one wins on ties).
            let mut lo: Option<&(f64, Lab)> = None;
            let mut hi: Option<&(f64, Lab)> = None;
            for stop in &self.ramp {
                if stop.0 <= l && lo.is_none_or(|x| stop.0 > x.0) {
                    lo = Some(stop);
                }
                if stop.0 >= l && hi.is_none_or(|x| stop.0 < x.0) {
                    hi = Some(stop);
                }
            }
            let lo = lo.unwrap_or(&self.ramp[0]);
            let hi = hi.unwrap_or(&self.ramp[self.ramp.len() - 1]);
            let t = if hi.0 == lo.0 { 0.0 } else { (l - lo.0) / (hi.0 - lo.0) };
            [0, 1, 2].map(|i| lo.1[i] + (hi.1[i] - lo.1[i]) * t)
        } else {
            let h = hue(lab);
            let mut best = &self.families[0];
            for family in &self.families[1..] {
                if hue_dist(hue(family.0), h) < hue_dist(hue(best.0), h) {
                    best = family;
                }
            }
            let (src, dst) = best;
            [0, 1, 2].map(|i| lab[i] + dst[i] - src[i])
        };
        oklab_to_rgb(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pal: &Palette, hex: &str) -> String {
        to_hex(pal.map_rgb(hex_to_rgb(hex).unwrap()))
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(hex_to_rgb("#fff").unwrap(), [1.0, 1.0, 1.0]);
        assert_eq!(hex_to_rgb("#000000").unwrap(), [0.0, 0.0, 0.0]);
        for bad in ["#12345g", "#1234", "", "#\u{e9}\u{e9}\u{e9}"] {
            assert!(hex_to_rgb(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn oklab_round_trip() {
        for hex in ["#5e2424", "#ebdbb2", "#282828", "#458588", "#ffffff"] {
            assert_eq!(to_hex(oklab_to_rgb(rgb_to_oklab(hex_to_rgb(hex).unwrap()))), hex);
        }
    }

    #[test]
    fn ramp_stops_map_exactly() {
        let pal = Palette::new().unwrap();
        for (stock, gruvbox) in RAMP {
            assert_eq!(map(&pal, stock), *gruvbox, "{stock}");
        }
    }

    #[test]
    fn hue_families_map_their_anchor_exactly() {
        let pal = Palette::new().unwrap();
        for (stock, gruvbox) in FAMILIES {
            assert_eq!(map(&pal, stock), *gruvbox, "{stock}");
        }
    }

    #[test]
    fn accent_line_is_light() {
        let pal = Palette::new().unwrap();
        let [l, ..] = rgb_to_oklab(hex_to_rgb(&pal.accent_line).unwrap());
        // Lightness is raised to 0.85, but a saturated accent at that
        // lightness can fall outside sRGB; clipping then lowers it slightly
        // (#5e2424 -> #ffb8b4 at 0.8475), just like libadwaita's derivation.
        assert!(l > 0.84, "{} has lightness {l}", pal.accent_line);
    }
}
