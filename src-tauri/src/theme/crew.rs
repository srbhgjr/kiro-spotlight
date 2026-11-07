//! Kiro Crew: `theme_color` / `theme_mode` from its config, palettes from its dashboard stylesheet.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cli::home;

const APP_BACKENDS: &str = "/Applications/KiroCrew.app/Contents/Resources/backend-dist";
const ASSETS: &str = "site-packages/kiro_crew/static/dist/assets";

pub fn config_path() -> PathBuf {
    home().join(".kiro/crew/config.json")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub color: String,
    pub mode: String,
}

/// Read `theme_color` and `theme_mode` wherever they sit in the config tree.
pub fn read_choice(config: &Value) -> Option<Choice> {
    let color = find_key(config, "theme_color")?.as_str()?.to_string();
    let mode = find_key(config, "theme_mode")
        .and_then(Value::as_str)
        .unwrap_or("system")
        .to_string();
    Some(Choice { color, mode })
}

fn find_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find_key(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

/// Stylesheets of the installed Kiro Crew dashboard, native architecture first.
pub fn stylesheets() -> Vec<PathBuf> {
    let native = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    let mut backends: Vec<PathBuf> = read_dirs(Path::new(APP_BACKENDS));
    backends.sort_by_key(|p| !p.to_string_lossy().ends_with(native));
    backends
        .iter()
        .flat_map(|backend| read_dirs(&backend.join("lib")))
        .map(|python| python.join(ASSETS))
        .flat_map(|assets| css_files(&assets))
        .collect()
}

fn read_dirs(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default()
}

fn css_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "css"))
                .collect()
        })
        .unwrap_or_default()
}

/// Custom properties declared for `[data-theme=<theme>]` across all rules that list it.
pub fn theme_vars(css: &str, theme: &str) -> HashMap<String, String> {
    let selector = format!("[data-theme={theme}]");
    let quoted = format!("[data-theme=\"{theme}\"]");
    let mut vars = HashMap::new();
    for chunk in css.split('}') {
        let Some((head, body)) = chunk.rsplit_once('{') else {
            continue;
        };
        let selectors = head.rsplit('{').next().unwrap_or(head);
        let listed = selectors.split(',').map(str::trim).any(|s| s == selector || s == quoted);
        if listed {
            collect_vars(body, &mut vars);
        }
    }
    vars
}

fn collect_vars(body: &str, vars: &mut HashMap<String, String>) {
    for decl in body.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.starts_with("--") {
            vars.insert(name.to_string(), value.trim().to_string());
        }
    }
}

/// Load the light or dark variable set for `color` from the first stylesheet that defines it.
pub fn load_vars(color: &str, variant: &str) -> Option<HashMap<String, String>> {
    let theme = format!("{color}-{variant}");
    stylesheets()
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .map(|css| theme_vars(&css, &theme))
        .find(|vars| vars.contains_key("--bg"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_nested_choice_with_default_mode() {
        let config = json!({ "dashboard": { "theme_mode": "dark", "theme_color": "rosepine" } });
        assert_eq!(read_choice(&config), Some(Choice { color: "rosepine".into(), mode: "dark".into() }));

        let no_mode = json!({ "ui": [{ "theme_color": "nord" }] });
        assert_eq!(read_choice(&no_mode).map(|c| c.mode), Some("system".into()));
        assert_eq!(read_choice(&json!({})), None);
    }

    #[test]
    fn extracts_vars_from_minified_css_including_shared_and_nested_rules() {
        let css = concat!(
            ":root{--bg:#fff}",
            "[data-theme=nord-dark],[data-theme=rosepine-dark]{--shadow-sm:0 1px 2px #0003}",
            "[data-theme=rosepine-dark]{--bg:#191724;--text:#e0def4;--accent:#ebbcba;color-scheme:dark}",
            "@media (min-width:1px){[data-theme=rosepine-dark]{--muted:#908ea4}}",
            "[data-theme=rosepine-light]{--bg:#faf4ed}"
        );
        let vars = theme_vars(css, "rosepine-dark");
        assert_eq!(vars["--bg"], "#191724");
        assert_eq!(vars["--accent"], "#ebbcba");
        assert_eq!(vars["--muted"], "#908ea4");
        assert!(vars.contains_key("--shadow-sm"));
        assert!(!vars.contains_key("color-scheme"));
        assert_eq!(theme_vars(css, "rosepine-light")["--bg"], "#faf4ed");
        assert!(theme_vars(css, "missing-dark").is_empty());
    }
}
