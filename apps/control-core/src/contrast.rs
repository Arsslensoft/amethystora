//! The theme maker's checks: the sums palette.awk does (/usr/lib/amethystora/theme/palette.awk, with checks=1), done here
//! so that the window measures as a colour is picked without running anything. The tests hold each to palette.awk itself.
//!
//! A palette's surfaces are worked out of its ground (seven of them, as palette.awk has them); then the text on every
//! surface (4.5:1), a label on the accent (4.5:1, white, the ground or the text, whichever reads best) and the outline at
//! the strength the palette draws it (3:1). Each check gives the lowest ratio and the nearest colour that passes: the
//! text moved in OKLab lightness until it reads, or the accent as the palette deepens or lightens it for its label.

use std::collections::BTreeMap;

/// A colour, red, green and blue from 0 to 255
type Rgb = (f64, f64, f64);

/// What palette.awk calls light: a ground at least this luminous is drawn as a light theme
pub const LIGHT_GROUND: f64 = 0.18;

/// One check: what it measures, the lowest ratio, the ratio it needs, and the colour that passes nearest the one picked
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub name: &'static str,
    pub ratio: f64,
    pub need: f64,
    /// The key of the palette the nearest colour is for, and the colour
    pub key: &'static str,
    pub nearest: String,
}

impl Check {
    pub fn passes(&self) -> bool {
        self.ratio >= self.need
    }
}

pub fn parse(hex: &str) -> Option<Rgb> {
    let digits = hex.strip_prefix('#').filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()))?;
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).map(f64::from).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

/// As palette.awk writes a colour: each channel rounded half up and kept within 0 and 255
fn rgb(r: f64, g: f64, b: f64) -> Rgb {
    let c = |v: f64| ((v + 0.5).floor()).clamp(0.0, 255.0);
    (c(r), c(g), c(b))
}

pub fn hex((r, g, b): Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", r as u8, g as u8, b as u8)
}

fn lin(v: f64) -> f64 {
    let v = v / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn unlin(v: f64) -> f64 {
    let v = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    v.clamp(0.0, 1.0)
}

pub fn luminance((r, g, b): Rgb) -> f64 {
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

pub fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    rgb(a.0 * (1.0 - t) + b.0 * t, a.1 * (1.0 - t) + b.1 * t, a.2 * (1.0 - t) + b.2 * t)
}

fn to_oklab((r, g, b): Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (lin(r), lin(g), lin(b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    (
        0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
    )
}

fn from_oklab(lightness: f64, a: f64, b: f64) -> Rgb {
    let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lightness - 0.0894841775 * a - 1.291485548 * b).powi(3);
    rgb(
        255.0 * unlin(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        255.0 * unlin(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        255.0 * unlin(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
    )
}

struct Palette {
    bg: Rgb,
    fg: Rgb,
    grounds: [Rgb; 7],
}

impl Palette {
    fn new(bg: Rgb, fg: Rgb, light: bool) -> Palette {
        let white = (255.0, 255.0, 255.0);
        let grounds = if light {
            [bg, mix(bg, white, 0.55), mix(bg, white, 0.35), mix(bg, fg, 0.05), mix(bg, white, 0.7), mix(bg, white, 0.75), bg]
        } else {
            [bg, mix(bg, (0.0, 0.0, 0.0), 0.18), mix(bg, fg, 0.06), mix(bg, fg, 0.035), mix(bg, fg, 0.07), mix(bg, fg, 0.1), mix(bg, fg, 0.08)]
        };
        Palette { bg, fg, grounds }
    }

    fn lowest(&self, color: Rgb) -> f64 {
        self.grounds.iter().map(|&g| ratio(color, g)).fold(f64::INFINITY, f64::min)
    }

    fn reads(&self, color: Rgb, target: f64) -> bool {
        self.grounds.iter().all(|&g| ratio(color, g) >= target)
    }

    fn label_on(&self, fill: Rgb) -> Option<Rgb> {
        [(255.0, 255.0, 255.0), self.bg, self.fg].into_iter().find(|&c| ratio(c, fill) >= 4.5)
    }

    /// palette.awk's pair(): the fill as it is when a label reads on it, else moved away from the best label until one does
    fn pair(&self, fill: Rgb) -> Rgb {
        if self.label_on(fill).is_some() {
            return fill;
        }
        let mut best = (255.0, 255.0, 255.0);
        let mut bestc = 0.0;
        for c in [(255.0, 255.0, 255.0), self.bg, self.fg] {
            if ratio(c, fill) > bestc {
                bestc = ratio(c, fill);
                best = c;
            }
        }
        let dir = if luminance(best) > luminance(fill) { -1.0 } else { 1.0 };
        let (mut l, a, b) = to_oklab(fill);
        let mut out = fill;
        for _ in 0..60 {
            l += dir * 0.01;
            out = from_oklab(l, a, b);
            if self.label_on(out).is_some() {
                break;
            }
        }
        out
    }

    /// palette.awk's strength() for a line: the first mix of the text into the ground, 30% to 90% in steps of 5, that
    /// reads at 3:1 on every surface; 90 when none does
    fn outline(&self) -> Rgb {
        let mut p = 30;
        while p <= 90 {
            let line = mix(self.bg, self.fg, f64::from(p) / 100.0);
            if self.reads(line, 3.0) {
                return line;
            }
            p += 5;
        }
        mix(self.bg, self.fg, 0.9)
    }
}

/// A palette with every key a theme has, what it leaves out worked out of what it has as amethystora-theme-file's
/// complete() does it (a terminal's way: the bright colours as the normal ones, the selection the accent over the ground)
pub fn complete(colors: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> =
        colors.iter().filter(|(_, v)| parse(v).is_some()).map(|(k, v)| (k.clone(), v.to_lowercase())).collect();
    let bg = out.get("background").and_then(|c| parse(c)).unwrap_or((23.0, 16.0, 33.0));
    let fg = out.get("foreground").and_then(|c| parse(c)).unwrap_or((239.0, 228.0, 255.0));
    out.insert("background".into(), hex(bg));
    out.insert("foreground".into(), hex(fg));
    out.entry("color0".into()).or_insert_with(|| hex(mix(bg, fg, 0.15)));
    out.entry("color7".into()).or_insert_with(|| hex(fg));
    for (i, fallback) in [(1, "#e5484d"), (2, "#3fa65a"), (3, "#d9a21b"), (4, "#3b82f6"), (5, "#a855f7"), (6, "#14b8a6")] {
        out.entry(format!("color{i}")).or_insert_with(|| fallback.to_string());
    }
    for i in 0..8 {
        let normal = out[&format!("color{i}")].clone();
        out.entry(format!("color{}", i + 8)).or_insert(normal);
    }
    let accent = out.entry("accent".into()).or_insert_with(|| out_color4(colors)).clone();
    out.entry("cursor".into()).or_insert_with(|| hex(fg));
    let selection = parse(&accent).map(|a| hex(mix(bg, a, 0.4))).unwrap_or_else(|| hex(bg));
    out.entry("selection_background".into()).or_insert(selection);
    out.entry("selection_foreground".into()).or_insert_with(|| hex(fg));
    out
}

fn out_color4(colors: &BTreeMap<String, String>) -> String {
    colors.get("color4").filter(|c| parse(c).is_some()).map(|c| c.to_lowercase()).unwrap_or_else(|| "#3b82f6".into())
}

/// palette.awk's text_fix(): the text moved in lightness, away from the ground, until it reads at 4.5:1 and its outline
/// reaches 3:1 on the surfaces that text makes; the last tried when none does
fn text_fix(bg: Rgb, fg: Rgb, light: bool) -> Rgb {
    let dir = if luminance(bg) >= LIGHT_GROUND { -1.0 } else { 1.0 };
    let (l, a, b) = to_oklab(fg);
    let mut c = fg;
    for i in 0..=100 {
        c = from_oklab(l + dir * f64::from(i) * 0.01, a, b);
        let palette = Palette::new(bg, c, light);
        if palette.reads(c, 4.5) && palette.reads(palette.outline(), 3.0) {
            return c;
        }
    }
    c
}

/// Whether a ground is light, as the helper writes light.mode for it
pub fn is_light(background: &str) -> bool {
    parse(background).is_some_and(|bg| luminance(bg) >= LIGHT_GROUND)
}

/// The three checks for a palette by its keys, or none when its ground, text or accent is not a colour
pub fn checks(colors: &BTreeMap<String, String>) -> Vec<Check> {
    let get = |key: &str| colors.get(key).and_then(|c| parse(c));
    let (Some(bg), Some(fg), Some(accent)) = (get("background"), get("foreground"), get("accent")) else { return Vec::new() };
    let light = luminance(bg) >= LIGHT_GROUND;
    let palette = Palette::new(bg, fg, light);
    let text = text_fix(bg, fg, light);
    let label = [(255.0, 255.0, 255.0), bg, fg].into_iter().map(|c| ratio(c, accent)).fold(0.0, f64::max);
    vec![
        Check { name: "text", ratio: palette.lowest(fg), need: 4.5, key: "foreground", nearest: hex(text) },
        Check { name: "label", ratio: label, need: 4.5, key: "accent", nearest: hex(palette.pair(accent)) },
        Check { name: "outline", ratio: palette.lowest(palette.outline()), need: 3.0, key: "foreground", nearest: hex(text) },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    fn root() -> PathBuf {
        std::env::var_os("AMETHYSTORA_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../system_files/shared"))
    }

    fn palette(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// palette.awk's checks for a palette, as `name ratio nearest` lines, or None where there is no awk or no palette.awk
    fn awk_checks(colors: &BTreeMap<String, String>) -> Option<Vec<(String, String, String)>> {
        let script = root().join("usr/lib/amethystora/theme/palette.awk");
        if !script.exists() {
            return None;
        }
        let mut child = Command::new("awk").arg("-f").arg(&script).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().ok()?;
        {
            let stdin = child.stdin.as_mut()?;
            for (key, value) in colors {
                writeln!(stdin, "{key}={value}").ok()?;
            }
            if is_light(&colors["background"]) {
                writeln!(stdin, "light=1").ok()?;
            }
            writeln!(stdin, "checks=1").ok()?;
        }
        let out = String::from_utf8(child.wait_with_output().ok()?.stdout).ok()?;
        Some(
            out.lines()
                .filter_map(|line| line.strip_prefix("check_"))
                .filter_map(|line| {
                    let (name, rest) = line.split_once('=')?;
                    let (ratio, nearest) = rest.split_once(' ')?;
                    Some((name.to_string(), ratio.to_string(), nearest.to_string()))
                })
                .collect(),
        )
    }

    /// Every shipped theme's palette, and a few that fail
    fn palettes() -> Vec<BTreeMap<String, String>> {
        let mut all = vec![
            palette(&[("background", "#20202a"), ("foreground", "#40404a"), ("accent", "#e0e070")]),
            palette(&[("background", "#f0f0f0"), ("foreground", "#9a9a9a"), ("accent", "#ffd27a")]),
            palette(&[("background", "#171021"), ("foreground", "#efe4ff"), ("accent", "#e08a5a")]),
            palette(&[("background", "#808080"), ("foreground", "#909090"), ("accent", "#7f7f7f")]),
        ];
        if let Ok(themes) = std::fs::read_dir(root().join("usr/share/amethystora/themes")) {
            for theme in themes.flatten() {
                let Ok(text) = std::fs::read_to_string(theme.path().join("colors.toml")) else { continue };
                let mut colors = BTreeMap::new();
                for line in text.lines() {
                    let Some((key, value)) = line.split_once('=') else { continue };
                    let (key, value) = (key.trim(), value.trim().trim_matches('"').to_lowercase());
                    if ["background", "foreground", "accent"].contains(&key) && parse(&value).is_some() {
                        colors.insert(key.to_string(), value);
                    }
                }
                if colors.len() == 3 {
                    all.push(colors);
                }
            }
        }
        all
    }

    #[test]
    fn the_checks_are_palette_awks() {
        let mut compared = 0;
        for colors in palettes() {
            let Some(theirs) = awk_checks(&colors) else {
                eprintln!("no awk or no palette.awk here: the checks are not compared");
                return;
            };
            let ours = checks(&colors);
            assert_eq!(ours.len(), theirs.len(), "{colors:?}");
            for (check, (name, ratio, nearest)) in ours.iter().zip(&theirs) {
                assert_eq!(check.name, name, "{colors:?}");
                assert_eq!(format!("{:.2}", check.ratio), *ratio, "{name} of {colors:?}");
                let (a, b) = (parse(&check.nearest).unwrap(), parse(nearest).unwrap());
                let off = (a.0 - b.0).abs().max((a.1 - b.1).abs()).max((a.2 - b.2).abs());
                assert!(off <= 1.0, "{name} of {colors:?}: {} and {nearest}", check.nearest);
            }
            compared += 1;
        }
        assert!(compared >= 4);
    }

    #[test]
    fn a_failing_palette_says_what_passes() {
        let colors = palette(&[("background", "#20202a"), ("foreground", "#40404a"), ("accent", "#e0e070")]);
        let all = checks(&colors);
        let text = all.iter().find(|c| c.name == "text").unwrap();
        assert!(!text.passes());
        let mut fixed = colors.clone();
        fixed.insert("foreground".into(), text.nearest.clone());
        assert!(checks(&fixed).iter().all(Check::passes), "{:?}", checks(&fixed));
        assert!(checks(&palette(&[("background", "nope"), ("foreground", "#ffffff"), ("accent", "#000000")])).is_empty());
        assert!(is_light("#f0f0f0") && !is_light("#171021"));
        let whole = complete(&palette(&[("background", "#000000"), ("foreground", "#FFFFFF"), ("color4", "#112233")]));
        assert_eq!(whole.len(), 22);
        assert_eq!(whole["accent"], "#112233");
        assert_eq!(whole["color12"], "#112233");
        assert_eq!(whole["foreground"], "#ffffff");
    }
}
