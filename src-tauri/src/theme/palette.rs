//! Colour parsing, compositing and the mapping from a theme's named colours to the panel palette.

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub kind: Kind,
    pub bg: String,
    pub fg: String,
    pub fg_muted: String,
    pub accent: String,
    pub accent_fg: String,
    pub input_bg: String,
    pub border: String,
    pub link: String,
    pub error: String,
    pub code_bg: String,
    pub selection: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r: r as f64, g: g as f64, b: b as f64, a: 1.0 }
    }

    /// Parse `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.trim().strip_prefix('#')?;
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let expanded: String = match hex.len() {
            3 | 4 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => hex.to_string(),
            _ => return None,
        };
        let channel = |i: usize| u8::from_str_radix(&expanded[i..i + 2], 16).ok();
        let alpha = if expanded.len() == 8 { channel(6)? as f64 / 255.0 } else { 1.0 };
        Some(Self { r: channel(0)? as f64, g: channel(2)? as f64, b: channel(4)? as f64, a: alpha })
    }

    /// Composite this colour over an opaque background.
    pub fn over(self, bg: Rgba) -> Rgba {
        let mix = |fg: f64, back: f64| fg * self.a + back * (1.0 - self.a);
        Rgba { r: mix(self.r, bg.r), g: mix(self.g, bg.g), b: mix(self.b, bg.b), a: 1.0 }
    }

    pub fn with_alpha(self, a: f64) -> Rgba {
        Rgba { a, ..self }
    }

    pub fn hex(self) -> String {
        let c = |v: f64| v.round().clamp(0.0, 255.0) as u8;
        let base = format!("#{:02x}{:02x}{:02x}", c(self.r), c(self.g), c(self.b));
        if self.a >= 0.999 {
            return base;
        }
        format!("{base}{:02x}", c(self.a * 255.0))
    }

    fn luminance(self) -> f64 {
        let lin = |v: f64| {
            let s = v / 255.0;
            if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }
}

pub fn contrast(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Theme keys to try, in order, for each palette field.
pub struct KeyMap {
    pub bg: &'static [&'static str],
    pub fg: &'static [&'static str],
    pub fg_muted: &'static [&'static str],
    pub accent: &'static [&'static str],
    pub accent_fg: &'static [&'static str],
    pub input_bg: &'static [&'static str],
    pub border: &'static [&'static str],
    pub link: &'static [&'static str],
    pub error: &'static [&'static str],
    pub code_bg: &'static [&'static str],
    pub selection: &'static [&'static str],
}

pub const VSCODE_KEYS: KeyMap = KeyMap {
    bg: &["editor.background", "sideBar.background"],
    fg: &["editor.foreground", "foreground"],
    fg_muted: &["descriptionForeground"],
    accent: &["button.background", "focusBorder", "textLink.foreground"],
    accent_fg: &["button.foreground"],
    input_bg: &["input.background"],
    border: &["widget.border", "panel.border", "editorWidget.border"],
    link: &["textLink.foreground"],
    error: &["errorForeground", "editorError.foreground"],
    code_bg: &["textCodeBlock.background"],
    selection: &["editor.selectionBackground"],
};

pub const CREW_KEYS: KeyMap = KeyMap {
    bg: &["--bg"],
    fg: &["--text"],
    fg_muted: &["--muted-strong", "--muted"],
    accent: &["--accent"],
    accent_fg: &["--accent-fg"],
    input_bg: &["--bg-elevated", "--card"],
    border: &["--border"],
    link: &["--info", "--accent"],
    error: &["--danger"],
    code_bg: &["--bg-accent"],
    selection: &["--accent-subtle"],
};

struct Defaults {
    bg: Rgba,
    fg: Rgba,
    error: Rgba,
}

fn defaults(kind: Kind) -> Defaults {
    match kind {
        Kind::Light => Defaults { bg: Rgba::rgb(255, 255, 255), fg: Rgba::rgb(31, 31, 31), error: Rgba::rgb(229, 20, 0) },
        Kind::Dark => Defaults { bg: Rgba::rgb(31, 31, 31), fg: Rgba::rgb(204, 204, 204), error: Rgba::rgb(248, 81, 73) },
    }
}

const MIN_TEXT_CONTRAST: f64 = 4.5;
const MIN_ACCENT_ALPHA: f64 = 0.5;
const MIN_BORDER_ALPHA: f64 = 0.05;
const MIN_SELECTION_ALPHA: f64 = 0.1;

struct Lookup<'a> {
    colors: &'a HashMap<String, String>,
    bg: Rgba,
}

impl Lookup<'_> {
    fn raw(&self, keys: &[&str], min_alpha: f64) -> Option<Rgba> {
        keys.iter()
            .filter_map(|k| self.colors.get(*k).and_then(|v| Rgba::parse(v)))
            .find(|c| c.a >= min_alpha)
    }

    fn solid(&self, keys: &[&str], min_alpha: f64) -> Option<Rgba> {
        self.raw(keys, min_alpha).map(|c| c.over(self.bg))
    }
}

/// Build a palette from a theme's colour map. Translucent colours are composited over the
/// background, invisible ones are skipped, and unreadable text falls back to the default.
pub fn build(colors: &HashMap<String, String>, kind: Kind, keys: &KeyMap) -> Palette {
    let d = defaults(kind);
    let bg = Lookup { colors, bg: d.bg }.solid(keys.bg, 0.0).unwrap_or(d.bg);
    let look = Lookup { colors, bg };
    let fg = readable(look.solid(keys.fg, 0.0).unwrap_or(d.fg), bg, d.fg);
    let accent = look.solid(keys.accent, MIN_ACCENT_ALPHA).unwrap_or(fg);
    let input_bg = look.solid(keys.input_bg, 0.0).unwrap_or(bg);
    Palette {
        kind,
        bg: bg.hex(),
        fg: fg.hex(),
        fg_muted: look.solid(keys.fg_muted, 0.0).unwrap_or(fg.with_alpha(0.65).over(bg)).hex(),
        accent: accent.hex(),
        accent_fg: look.solid(keys.accent_fg, 0.0).unwrap_or_else(|| on_color(accent)).hex(),
        input_bg: input_bg.hex(),
        border: look.solid(keys.border, MIN_BORDER_ALPHA).unwrap_or(fg.with_alpha(0.12).over(bg)).hex(),
        link: look.solid(keys.link, MIN_ACCENT_ALPHA).unwrap_or(accent).hex(),
        error: look.solid(keys.error, MIN_ACCENT_ALPHA).unwrap_or(d.error).hex(),
        code_bg: look.solid(keys.code_bg, 0.0).unwrap_or(input_bg).hex(),
        selection: look.raw(keys.selection, MIN_SELECTION_ALPHA).unwrap_or(accent.with_alpha(0.25)).hex(),
    }
}

fn readable(fg: Rgba, bg: Rgba, fallback: Rgba) -> Rgba {
    if contrast(fg, bg) >= MIN_TEXT_CONTRAST {
        fg
    } else {
        fallback
    }
}

fn on_color(color: Rgba) -> Rgba {
    let white = Rgba::rgb(255, 255, 255);
    let black = Rgba::rgb(0, 0, 0);
    if contrast(white, color) >= contrast(black, color) {
        white
    } else {
        black
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn parses_all_hex_forms() {
        assert_eq!(Rgba::parse("#fff"), Some(Rgba::rgb(255, 255, 255)));
        assert_eq!(Rgba::parse("#0000").map(|c| c.a), Some(0.0));
        assert_eq!(Rgba::parse("#faf4ed").map(Rgba::hex).as_deref(), Some("#faf4ed"));
        assert_eq!(Rgba::parse("#6e6a8614").map(|c| (c.a * 255.0).round()), Some(20.0));
        assert_eq!(Rgba::parse("red"), None);
        assert_eq!(Rgba::parse("#12345"), None);
    }

    #[test]
    fn rose_pine_dawn_maps_to_expected_palette() {
        let colors = map(&[
            ("editor.background", "#faf4ed"),
            ("editor.foreground", "#575279"),
            ("focusBorder", "#6e6a8614"),
            ("button.background", "#d7827e"),
            ("button.foreground", "#faf4ed"),
            ("descriptionForeground", "#797593"),
            ("panel.border", "#0000"),
            ("textLink.foreground", "#907aa9"),
            ("errorForeground", "#b4637a"),
        ]);
        let p = build(&colors, Kind::Light, &VSCODE_KEYS);
        assert_eq!(p.bg, "#faf4ed");
        assert_eq!(p.fg, "#575279");
        assert_eq!(p.accent, "#d7827e");
        assert_eq!(p.accent_fg, "#faf4ed");
        assert_eq!(p.link, "#907aa9");
        assert_ne!(p.border, p.bg, "fully transparent border must fall back to a visible one");
    }

    #[test]
    fn translucent_accent_is_skipped_for_an_opaque_one() {
        let colors = map(&[("focusBorder", "#ff000010"), ("textLink.foreground", "#3366ff")]);
        let p = build(&colors, Kind::Dark, &VSCODE_KEYS);
        assert_eq!(p.accent, "#3366ff");
    }

    #[test]
    fn unreadable_foreground_falls_back_to_default() {
        let colors = map(&[("editor.background", "#ffffff"), ("editor.foreground", "#fefefe")]);
        let p = build(&colors, Kind::Light, &VSCODE_KEYS);
        assert_eq!(p.fg, "#1f1f1f");
    }

    #[test]
    fn empty_theme_uses_kind_defaults() {
        let p = build(&HashMap::new(), Kind::Dark, &VSCODE_KEYS);
        assert_eq!(p.bg, "#1f1f1f");
        assert_eq!(p.fg, "#cccccc");
        assert_eq!(p.kind, Kind::Dark);
    }

    #[test]
    fn crew_variables_map_through_crew_keys() {
        let vars = map(&[("--bg", "#191724"), ("--text", "#e0def4"), ("--accent", "#ebbcba"), ("--accent-fg", "#000")]);
        let p = build(&vars, Kind::Dark, &CREW_KEYS);
        assert_eq!((p.bg.as_str(), p.fg.as_str(), p.accent.as_str()), ("#191724", "#e0def4", "#ebbcba"));
        assert_eq!(p.accent_fg, "#000000");
    }
}
