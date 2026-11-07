//! Resolve the panel's colours from the user's selected Kiro theme.

pub mod crew;
pub mod ide;
pub mod palette;
pub mod watch;

use serde::Serialize;

use crate::settings::ThemeSource;
use palette::{Kind, Palette, CREW_KEYS, VSCODE_KEYS};

pub const EVENT_THEME_CHANGED: &str = "theme://changed";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemePayload {
    pub source: &'static str,
    pub name: String,
    pub mode: Mode,
    pub light: Option<Palette>,
    pub dark: Option<Palette>,
}

pub fn resolve(source: ThemeSource) -> ThemePayload {
    let resolved = match source {
        ThemeSource::Auto => from_ide().or_else(from_crew),
        ThemeSource::KiroIde => from_ide(),
        ThemeSource::KiroCrew => from_crew(),
        ThemeSource::System => None,
    };
    resolved.unwrap_or_else(system)
}

fn system() -> ThemePayload {
    ThemePayload { source: "system", name: "macOS".into(), mode: Mode::System, light: None, dark: None }
}

fn from_ide() -> Option<ThemePayload> {
    let settings = ide::read_jsonc(&ide::settings_path())?;
    let dirs = ide::extension_dirs();
    match ide::read_selection(&settings)? {
        ide::Selection::Fixed(name) => {
            let theme = ide::find_theme(&name, &dirs)?;
            Some(single("kiro-ide", name, theme.kind, ide_palette(&theme)))
        }
        ide::Selection::FollowSystem { light, dark } => {
            let light_theme = ide::find_theme(&light, &dirs);
            let dark_theme = ide::find_theme(&dark, &dirs);
            Some(ThemePayload {
                source: "kiro-ide",
                name: format!("{light} / {dark}"),
                mode: Mode::System,
                light: light_theme.as_ref().map(ide_palette),
                dark: dark_theme.as_ref().map(ide_palette),
            })
            .filter(|p| p.light.is_some() || p.dark.is_some())
        }
    }
}

fn ide_palette(theme: &ide::IdeTheme) -> Palette {
    palette::build(&theme.colors, theme.kind, &VSCODE_KEYS)
}

fn from_crew() -> Option<ThemePayload> {
    let config = ide::read_jsonc(&crew::config_path())?;
    let choice = crew::read_choice(&config)?;
    let light = crew::load_vars(&choice.color, "light").map(|v| palette::build(&v, Kind::Light, &CREW_KEYS));
    let dark = crew::load_vars(&choice.color, "dark").map(|v| palette::build(&v, Kind::Dark, &CREW_KEYS));
    let mode = match choice.mode.as_str() {
        "light" if light.is_some() => Mode::Light,
        "dark" if dark.is_some() => Mode::Dark,
        _ => Mode::System,
    };
    let payload = ThemePayload { source: "kiro-crew", name: choice.color, mode, light, dark };
    (payload.light.is_some() || payload.dark.is_some()).then_some(payload)
}

fn single(source: &'static str, name: String, kind: Kind, palette: Palette) -> ThemePayload {
    let (mode, light, dark) = match kind {
        Kind::Light => (Mode::Light, Some(palette), None),
        Kind::Dark => (Mode::Dark, None, Some(palette)),
    };
    ThemePayload { source, name, mode, light, dark }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_source_never_reads_kiro() {
        let payload = resolve(ThemeSource::System);
        assert_eq!(payload.source, "system");
        assert_eq!(payload.mode, Mode::System);
        assert!(payload.light.is_none() && payload.dark.is_none());
    }

    #[test]
    #[ignore = "reads the real Kiro IDE and Kiro Crew settings on this machine"]
    fn prints_resolved_themes_on_this_machine() {
        for source in [ThemeSource::Auto, ThemeSource::KiroIde, ThemeSource::KiroCrew] {
            println!("{source:?} => {}", serde_json::to_string(&resolve(source)).unwrap());
        }
    }

    #[test]
    fn single_theme_fills_only_its_kind() {
        let p = palette::build(&Default::default(), Kind::Light, &VSCODE_KEYS);
        let payload = single("kiro-ide", "X".into(), Kind::Light, p);
        assert_eq!(payload.mode, Mode::Light);
        assert!(payload.light.is_some() && payload.dark.is_none());
    }
}
