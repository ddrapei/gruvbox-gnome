//! Stylesheet rewriting and the structure check.

use std::ops::Range;
use std::sync::LazyLock;

use regex::{Captures, Regex};

use crate::color::{self, ACCENT_BG, ACCENT_FG, Palette, SOLID_BLACK};

static HEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#(?:[0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b").unwrap());
static RGBA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(,\s*[\d.]+\s*)?\)").unwrap()
});
// Colour syntaxes this tool does not convert. If GNOME starts using them,
// those spots simply keep their stock colour; a warning is logged.
static UNHANDLED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\b(?:oklab|oklch|lab|lch|hsla?|hwb|color-mix|color)\(|",
        r"rgba?\(\s*\d+\s+\d+|var\(--|-st-accent-[a-z-]*",
    ))
    .unwrap()
});
static COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());
static ACCENT_LEFT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-st-accent-(?:fg-)?color").unwrap());
static ANY_HEX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#[0-9a-fA-F]+\b").unwrap());

const FILL_PROPS: &[&str] = &["background-color", "background", "-barlevel-active-background-color"];

/// `Regex::replace_all` with a fallible replacement.
pub(crate) fn try_replace_all(
    re: &Regex,
    text: &str,
    mut replace: impl FnMut(&Captures) -> Result<String, String>,
) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        out.push_str(&text[last..m.start()]);
        out.push_str(&replace(&caps)?);
        last = m.end();
    }
    out.push_str(&text[last..]);
    Ok(out)
}

fn recolor_block(body: &str, pal: &Palette) -> Result<String, String> {
    let body = try_replace_all(&HEX_RE, body, |caps| {
        let hex = &caps[0];
        if hex.eq_ignore_ascii_case("#000000") || hex.eq_ignore_ascii_case("#000") {
            return Ok(SOLID_BLACK.to_string());
        }
        Ok(color::to_hex(pal.map_rgb(color::hex_to_rgb(hex)?)))
    })?;
    let body = try_replace_all(&RGBA_RE, &body, |caps| {
        let channel = |i: usize| {
            caps[i].parse::<u32>().map_err(|e| format!("bad rgb() channel {:?}: {e}", &caps[i]))
        };
        let (r, g, b) = (channel(1)?, channel(2)?, channel(3)?);
        if (r, g, b) == (0, 0, 0) {
            // shadows stay black
            return Ok(caps[0].to_string());
        }
        let rgb = [r, g, b].map(|c| f64::from(c) / 255.0);
        let [nr, ng, nb] = pal.map_rgb(rgb).map(color::to_byte);
        Ok(match caps.get(4) {
            Some(alpha) => format!("rgba({nr}, {ng}, {nb}{})", alpha.as_str()),
            None => format!("rgb({nr}, {ng}, {nb})"),
        })
    })?;
    // Map the stock colours first, then insert the accent: the accent is
    // already a gruvbox colour and must not go through the mapping again.
    let body = body.replace("-st-accent-fg-color", ACCENT_FG);
    let decls: Vec<String> = body
        .split(';')
        .map(|decl| {
            if !decl.contains("-st-accent-color") {
                return decl.to_string();
            }
            let prop = decl.split(':').next().unwrap_or("").trim();
            if FILL_PROPS.contains(&prop) {
                // Solid fills get the accent; translucent tints of it (selected
                // items, ripples) would be invisible, so they use the light shade.
                decl.replace(
                    "st-transparentize(-st-accent-color",
                    &format!("st-transparentize({}", pal.accent_line),
                )
                .replace("-st-accent-color", ACCENT_BG)
            } else {
                decl.replace("-st-accent-color", &pal.accent_line)
            }
        })
        .collect();
    Ok(decls.join(";"))
}

/// Byte ranges of the bodies (between the braces) of all top-level blocks.
/// The one place that walks braces; `blocks` and `recolor` build on it.
fn block_bodies(css: &str) -> Result<Vec<Range<usize>>, String> {
    const UNBALANCED: &str = "unbalanced braces in stylesheet";
    let (mut bodies, mut depth, mut start) = (Vec::new(), 0usize, 0);
    // Bytes, not chars: '{' and '}' never occur inside a multi-byte UTF-8
    // sequence, so every index found is also a valid char boundary.
    for (i, byte) in css.bytes().enumerate() {
        match byte {
            b'{' => {
                if depth == 0 {
                    start = i + 1;
                }
                depth += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1).ok_or(UNBALANCED)?;
                if depth == 0 {
                    bodies.push(start..i);
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(UNBALANCED.into());
    }
    Ok(bodies)
}

/// Split CSS into (prelude, body) pairs for top-level blocks.
fn blocks(css: &str) -> Result<Vec<(&str, &str)>, String> {
    let mut prelude_start = 0;
    Ok(block_bodies(css)?
        .into_iter()
        .map(|body| {
            let prelude = &css[prelude_start..body.start - 1];
            prelude_start = body.end + 1;
            (prelude, &css[body])
        })
        .collect())
}

/// Recolour only declaration blocks, never selectors (e.g. `#add`).
pub fn recolor(css: &str, pal: &Palette) -> Result<String, String> {
    let mut out = String::with_capacity(css.len());
    let mut copied = 0;
    for body in block_bodies(css)? {
        out.push_str(&css[copied..body.start]);
        out.push_str(&recolor_block(&css[body.clone()], pal)?);
        copied = body.end;
    }
    out.push_str(&css[copied..]);
    Ok(out)
}

type Skeleton = Vec<(String, Vec<String>)>;

/// Selectors and property names, in order: everything except values.
fn skeleton(css: &str) -> Result<Skeleton, String> {
    let css = COMMENT_RE.replace_all(css, "");
    Ok(blocks(&css)?
        .into_iter()
        .map(|(prelude, body)| {
            let selector = prelude.split_whitespace().collect::<Vec<_>>().join(" ");
            let props = body
                .split(';')
                .filter(|d| !d.trim().is_empty())
                .map(|d| d.split(':').next().unwrap_or("").trim().to_string())
                .collect();
            (selector, props)
        })
        .collect())
}

/// Fail if the generated stylesheet differs from the stock one in anything
/// but colour values. Returns a list of warnings.
pub fn validate(source: &str, generated: &str) -> Result<Vec<String>, String> {
    if skeleton(source)? != skeleton(generated)? {
        return Err("generated stylesheet does not match the stock structure".into());
    }
    let left = ACCENT_LEFT_RE.find_iter(generated).count();
    if left > 0 {
        return Err(format!("{left} accent keywords left unconverted"));
    }
    let uncommented = COMMENT_RE.replace_all(generated, "");
    let bodies: String = blocks(&uncommented)?.into_iter().map(|(_, body)| body).collect();
    let bad_hex: Vec<&str> = ANY_HEX_RE
        .find_iter(&bodies)
        .map(|m| m.as_str())
        .filter(|h| h.len() != 4 && h.len() != 7)
        .collect();
    if !bad_hex.is_empty() {
        let shown: Vec<String> = bad_hex.iter().take(3).map(|h| format!("'{h}'")).collect();
        return Err(format!("malformed colours in output: [{}]", shown.join(", ")));
    }
    let mut unhandled: Vec<&str> = UNHANDLED_RE.find_iter(&bodies).map(|m| m.as_str()).collect();
    unhandled.sort_unstable();
    unhandled.dedup();
    let mut warnings = Vec::new();
    if !unhandled.is_empty() {
        warnings.push(format!(
            "stock stylesheet uses colour syntax this tool does not convert \
             (those spots keep stock colours): {}",
            unhandled.join(", ")
        ));
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> Palette {
        Palette::new().unwrap()
    }

    #[test]
    fn selectors_are_never_recoloured() {
        let out = recolor("#add { color: #ffffff; }", &pal()).unwrap();
        assert!(out.starts_with("#add {"), "{out}");
        assert!(out.contains("color: #ebdbb2"), "{out}");
    }

    #[test]
    fn black_and_shadows() {
        let out = recolor(".a { background: #000; box-shadow: 0 1px rgba(0, 0, 0, 0.5); }", &pal())
            .unwrap();
        assert!(out.contains(SOLID_BLACK), "{out}");
        assert!(out.contains("rgba(0, 0, 0, 0.5)"), "{out}");
    }

    #[test]
    fn accent_fill_vs_line() {
        let p = pal();
        let out = recolor(
            ".a { background-color: -st-accent-color; border-color: -st-accent-color; \
             color: -st-accent-fg-color; }",
            &p,
        )
        .unwrap();
        assert!(out.contains(&format!("background-color: {ACCENT_BG}")), "{out}");
        assert!(out.contains(&format!("border-color: {}", p.accent_line)), "{out}");
        assert!(out.contains(&format!("color: {ACCENT_FG}")), "{out}");
    }

    #[test]
    fn validate_catches_structure_changes() {
        assert!(validate(".a { color: red; }", ".a { color: blue; }").unwrap().is_empty());
        assert!(validate(".a { color: red; }", ".b { color: red; }").is_err());
        assert!(validate(".a { color: red; }", ".a { colour: red; }").is_err());
        assert!(validate(".a { color: red; }", ".a { color: red; ").is_err());
    }
}
