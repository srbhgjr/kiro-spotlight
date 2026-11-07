//! Kiro IDE: the selected `workbench.colorTheme` and the theme JSON contributed by an extension.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::palette::Kind;
use crate::cli::home;

const MAX_INCLUDE_DEPTH: usize = 5;

pub fn settings_path() -> PathBuf {
    home().join("Library/Application Support/Kiro/User/settings.json")
}

pub fn extension_dirs() -> Vec<PathBuf> {
    vec![
        home().join(".kiro/extensions"),
        PathBuf::from("/Applications/Kiro.app/Contents/Resources/app/extensions"),
    ]
}

/// Which theme(s) the IDE is set to show.
#[derive(Debug, PartialEq)]
pub enum Selection {
    Fixed(String),
    FollowSystem { light: String, dark: String },
}

pub fn read_selection(settings: &Value) -> Option<Selection> {
    let get = |key: &str| settings.get(key).and_then(Value::as_str).map(str::to_string);
    let current = get("workbench.colorTheme");
    let auto = settings.get("window.autoDetectColorScheme").and_then(Value::as_bool) == Some(true);
    if !auto {
        return current.map(Selection::Fixed);
    }
    let light = get("workbench.preferredLightColorTheme").or_else(|| current.clone())?;
    let dark = get("workbench.preferredDarkColorTheme").or(current)?;
    Some(Selection::FollowSystem { light, dark })
}

#[derive(Debug, Clone)]
pub struct IdeTheme {
    pub kind: Kind,
    pub colors: HashMap<String, String>,
}

pub fn read_jsonc(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    json5::from_str(&text).ok()
}

/// Find the theme whose label or id is `name` in any extension under `dirs`, and load its colours.
pub fn find_theme(name: &str, dirs: &[PathBuf]) -> Option<IdeTheme> {
    dirs.iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flat_map(|entries| entries.flatten().map(|e| e.path()))
        .find_map(|ext| theme_in_extension(&ext, name))
}

fn theme_in_extension(ext: &Path, name: &str) -> Option<IdeTheme> {
    let manifest = read_jsonc(&ext.join("package.json"))?;
    let nls = read_jsonc(&ext.join("package.nls.json")).unwrap_or(Value::Null);
    let themes = manifest.pointer("/contributes/themes")?.as_array()?;
    let entry = themes.iter().find(|t| theme_matches(t, name, &nls))?;
    let path = ext.join(entry.get("path")?.as_str()?);
    let kind = kind_of(entry.get("uiTheme").and_then(Value::as_str).unwrap_or("vs-dark"));
    let colors = load_colors(&path, 0);
    Some(IdeTheme { kind, colors })
}

fn theme_matches(entry: &Value, name: &str, nls: &Value) -> bool {
    let label = entry.get("label").and_then(Value::as_str).map(|l| localize(l, nls));
    let id = entry.get("id").and_then(Value::as_str);
    label.as_deref() == Some(name) || id == Some(name)
}

fn localize(label: &str, nls: &Value) -> String {
    let key = label.strip_prefix('%').and_then(|l| l.strip_suffix('%'));
    key.and_then(|k| nls.get(k))
        .and_then(|v| v.as_str().or_else(|| v.get("message").and_then(Value::as_str)))
        .unwrap_or(label)
        .to_string()
}

fn kind_of(ui_theme: &str) -> Kind {
    match ui_theme {
        "vs" | "hc-light" => Kind::Light,
        _ => Kind::Dark,
    }
}

/// Load a theme file's `colors`, following `include` to parent themes; the child wins.
fn load_colors(path: &Path, depth: usize) -> HashMap<String, String> {
    let Some(theme) = read_jsonc(path) else {
        return HashMap::new();
    };
    let mut colors = parent_colors(path, &theme, depth);
    let own = theme.get("colors").and_then(Value::as_object).into_iter().flatten();
    for (key, value) in own {
        if let Some(value) = value.as_str() {
            colors.insert(key.clone(), value.to_string());
        }
    }
    colors
}

fn parent_colors(path: &Path, theme: &Value, depth: usize) -> HashMap<String, String> {
    let include = theme.get("include").and_then(Value::as_str);
    match (include, path.parent()) {
        (Some(rel), Some(dir)) if depth < MAX_INCLUDE_DEPTH => load_colors(&dir.join(rel), depth + 1),
        _ => HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn fixed_and_auto_detect_selections() {
        let fixed = json!({ "workbench.colorTheme": "Rosé Pine Dawn" });
        assert_eq!(read_selection(&fixed), Some(Selection::Fixed("Rosé Pine Dawn".into())));

        let auto = json!({
            "workbench.colorTheme": "Rosé Pine Dawn",
            "window.autoDetectColorScheme": true,
            "workbench.preferredDarkColorTheme": "Rosé Pine"
        });
        assert_eq!(
            read_selection(&auto),
            Some(Selection::FollowSystem { light: "Rosé Pine Dawn".into(), dark: "Rosé Pine".into() })
        );
        assert_eq!(read_selection(&json!({})), None);
    }

    #[test]
    fn finds_theme_by_label_with_jsonc_and_include() {
        let root = tempfile::tempdir().unwrap();
        let ext = root.path().join("vendor.theme-1.0.0");
        write(
            &ext.join("package.json"),
            r#"{ "contributes": { "themes": [
                 // a comment, as VS Code manifests allow
                 { "label": "%dawn%", "uiTheme": "vs", "path": "./themes/dawn.json" },
               ] } }"#,
        );
        write(&ext.join("package.nls.json"), r#"{ "dawn": "My Dawn" }"#);
        write(&ext.join("themes/base.json"), r##"{ "colors": { "editor.background": "#000000", "editor.foreground": "#111111" } }"##);
        write(&ext.join("themes/dawn.json"), r##"{ "include": "./base.json", "colors": { "editor.background": "#faf4ed", } }"##);

        let theme = find_theme("My Dawn", &[root.path().to_path_buf()]).expect("theme found");
        assert_eq!(theme.kind, Kind::Light);
        assert_eq!(theme.colors["editor.background"], "#faf4ed");
        assert_eq!(theme.colors["editor.foreground"], "#111111");
        assert!(find_theme("Missing", &[root.path().to_path_buf()]).is_none());
    }

    #[test]
    fn include_cycles_terminate() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("loop.json");
        write(&file, r##"{ "include": "./loop.json", "colors": { "a": "#fff" } }"##);
        assert_eq!(load_colors(&file, 0)["a"], "#fff");
    }
}
