//! Locating `kiro-cli` and preparing the environment and agent profile it runs with.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::process::Command;

pub const AGENT_NAME: &str = "kiro-spotlight";
const PATH_MARK: &str = "__KIRO_SPOTLIGHT_PATH__";

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Find an executable `kiro-cli`, preferring an explicit override, then `PATH`, then common install dirs.
pub fn locate(override_path: Option<&Path>, path_env: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = override_path.filter(|p| is_executable(p)) {
        return Some(path.to_path_buf());
    }
    let fallbacks = [
        home().join(".local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    path_env
        .into_iter()
        .flat_map(|p| p.split(':'))
        .map(PathBuf::from)
        .chain(fallbacks)
        .map(|dir| dir.join("kiro-cli"))
        .find(|p| is_executable(p))
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// The user's login-shell `PATH`. Apps launched from Finder get a minimal `PATH`, which would
/// stop kiro-cli from finding MCP server launchers such as `npx` or `uvx`.
pub async fn login_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let script = format!("printf '{PATH_MARK}%s{PATH_MARK}' \"$PATH\"");
    let run = Command::new(shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(5), run).await.ok()?.ok()?;
    extract_marked(&String::from_utf8_lossy(&output.stdout))
}

fn extract_marked(text: &str) -> Option<String> {
    let start = text.find(PATH_MARK)? + PATH_MARK.len();
    let rest = &text[start..];
    let value = &rest[..rest.find(PATH_MARK)?];
    (!value.is_empty()).then(|| value.to_string())
}

/// Write `~/.kiro/agents/kiro-spotlight.json` if it does not exist. An existing file is never touched.
pub fn ensure_agent_profile() -> std::io::Result<PathBuf> {
    let dir = home().join(".kiro").join("agents");
    let path = dir.join(format!("{AGENT_NAME}.json"));
    if path.exists() {
        return Ok(path);
    }
    std::fs::create_dir_all(&dir)?;
    let body = serde_json::to_string_pretty(&agent_profile()).unwrap_or_default();
    std::fs::write(&path, body)?;
    Ok(path)
}

pub fn agent_profile() -> Value {
    json!({
        "name": AGENT_NAME,
        "description": "Kiro Spotlight quick chat. Read-only tools run without asking; everything else asks.",
        "prompt": "You are Kiro, answering in a small quick-chat panel on macOS. Keep answers short and direct, use Markdown, and prefer a code block over prose for commands. This conversation is incognito and is deleted when the panel closes.",
        "tools": ["*"],
        "allowedTools": ["fs_read", "grep", "glob", "web_fetch", "web_search"],
        "resources": [],
        "includeMcpJson": true
    })
}

pub fn agent_args(model: Option<&str>) -> Vec<String> {
    let mut args = vec!["acp".to_string(), "--agent".to_string(), AGENT_NAME.to_string()];
    if let Some(model) = model.filter(|m| !m.is_empty()) {
        args.push("--model".into());
        args.push(model.into());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_path_between_markers_ignoring_rc_noise() {
        let text = format!("welcome!\n{PATH_MARK}/a:/b{PATH_MARK}\nbye");
        assert_eq!(extract_marked(&text).as_deref(), Some("/a:/b"));
        assert_eq!(extract_marked("no markers"), None);
        assert_eq!(extract_marked(&format!("{PATH_MARK}{PATH_MARK}")), None);
    }

    #[test]
    fn locate_prefers_executable_override() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("kiro-cli");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(locate(Some(&bin), None), Some(bin.clone()));

        let path_env = dir.path().to_string_lossy().to_string();
        assert_eq!(locate(None, Some(&path_env)), Some(bin));
    }

    #[test]
    fn agent_args_include_model_only_when_set() {
        assert_eq!(agent_args(None), vec!["acp", "--agent", AGENT_NAME]);
        assert_eq!(agent_args(Some("")).len(), 3);
        assert_eq!(agent_args(Some("auto")).last().map(String::as_str), Some("auto"));
    }

    #[test]
    fn agent_profile_trusts_only_read_only_tools() {
        let profile = agent_profile();
        let allowed: Vec<&str> = profile["allowedTools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(!allowed.contains(&"execute_bash"));
        assert!(!allowed.contains(&"fs_write"));
        assert_eq!(profile["includeMcpJson"], true);
    }
}
