//! Locating `kiro-cli` and preparing the environment and session options it runs with.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::process::Command;

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

/// `kiro-cli acp` with no `--agent`: a custom agent selects an engine without `session/delete`.
pub fn agent_args() -> Vec<String> {
    vec!["acp".to_string()]
}

/// Per-session options applied right after `session/new`, in order.
///
/// `autopilot: off` makes kiro-cli ask through `session/request_permission` before running
/// commands or writing files; `contentCollection` and `memoryReflection` are switched off so
/// the conversation is not used for content collection or written into Kiro memory.
pub fn session_options(model: Option<&str>) -> Vec<(&'static str, Value)> {
    let mut options = vec![
        ("autopilot", json!("off")),
        ("contentCollection", json!("disabled")),
        ("memoryReflection", json!("off")),
    ];
    if let Some(model) = model.filter(|m| !m.is_empty()) {
        options.push(("model", json!(model)));
    }
    options
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
    fn agent_never_selects_a_custom_agent() {
        assert_eq!(agent_args(), vec!["acp"]);
    }

    #[test]
    fn session_options_lock_down_autonomy_and_collection() {
        let options = session_options(None);
        let ids: Vec<&str> = options.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vec!["autopilot", "contentCollection", "memoryReflection"]);
        assert_eq!(options[0].1, "off");
        assert_eq!(session_options(Some("")).len(), 3);
        assert_eq!(session_options(Some("claude-sonnet-5.5")).last().unwrap().1, "claude-sonnet-5.5");
    }
}
