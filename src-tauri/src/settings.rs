//! User settings. Nothing conversational is ever stored here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSource {
    #[default]
    Auto,
    KiroIde,
    KiroCrew,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub model: Option<String>,
    pub theme_source: ThemeSource,
    pub idle_minutes: u64,
    pub cli_path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".into(),
            model: None,
            theme_source: ThemeSource::Auto,
            idle_minutes: 10,
            cli_path: None,
        }
    }
}

impl Settings {
    /// Load from `path`, falling back to defaults for a missing or unreadable file.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let settings = Settings::load(Path::new("/nonexistent/settings.json"));
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"themeSource":"kiro-crew","idleMinutes":3}"#).unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.theme_source, ThemeSource::KiroCrew);
        assert_eq!(settings.idle_minutes, 3);
        assert_eq!(settings.hotkey, "Alt+Space");
    }
}
