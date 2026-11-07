//! Tests against the real `kiro-cli` on this machine. Ignored by default because they need a
//! signed-in kiro-cli and spend a small amount of credits. Run with:
//! `cargo test live -- --ignored --nocapture`

use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::acp::{protocol, AcpConnection, InboundHandler, Spawn};
use crate::cleanup::{KiroStore, TempDirs};
use crate::cli;

struct Quiet;

impl InboundHandler for Quiet {
    fn on_notification(&self, _method: String, _params: Value) {}
    fn on_request(&self, _id: Value, _method: String, _params: Value) {}
    fn on_exit(&self) {}
}

async fn connect() -> Arc<AcpConnection> {
    connect_in(vec!["acp".to_string()], None).await
}

async fn connect_in(args: Vec<String>, cwd: Option<&Path>) -> Arc<AcpConnection> {
    let path_env = cli::login_shell_path().await;
    let program = cli::locate(None, path_env.as_deref()).expect("kiro-cli installed");
    let spec = Spawn { program: &program, args: &args, path_env: path_env.as_deref(), cwd };
    let conn = AcpConnection::spawn(&spec, Arc::new(Quiet)).expect("spawn");
    conn.request("initialize", protocol::initialize_params("test")).await.expect("initialize");
    conn
}

#[tokio::test]
#[ignore = "needs a signed-in kiro-cli"]
async fn live_engine_supports_delete_and_session_options() {
    let base = tempfile::tempdir().unwrap();
    let temp = TempDirs::new(base.path().join("kiro-spotlight"));
    std::fs::create_dir_all(&temp.base).unwrap();
    let conn = connect_in(cli::agent_args(), Some(&temp.base)).await;
    let cwd = temp.create().unwrap();
    let store = KiroStore::under(&cli::home().join(".kiro"));

    let created = conn.request("session/new", json!({ "cwd": cwd, "mcpServers": [] })).await.unwrap();
    let session_id = created["sessionId"].as_str().unwrap().to_string();
    let mut last = Value::Null;
    for (config_id, value) in cli::session_options(None) {
        let params = json!({ "sessionId": session_id, "configId": config_id, "value": value });
        last = conn.request("session/set_config_option", params).await.expect(config_id);
    }
    let current: Vec<(String, String)> = last["configOptions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| (c["id"].as_str().unwrap_or_default().into(), c["currentValue"].as_str().unwrap_or_default().into()))
        .collect();
    println!("{current:?}");

    let hash = store.find_cwd_hash(&session_id);
    conn.request("session/delete", json!({ "sessionId": session_id })).await.expect("session/delete supported");
    if let Some(hash) = hash.or_else(|| store.find_cwd_hash(&session_id)) {
        store.sweep(&hash, &session_id).unwrap();
    }
    temp.remove(&cwd).unwrap();
    conn.shutdown().await;
    assert!(current.contains(&("autopilot".into(), "off".into())));
    assert!(current.contains(&("contentCollection".into(), "disabled".into())));
    assert!(current.contains(&("memoryReflection".into(), "off".into())));
}

#[tokio::test]
#[ignore = "needs a signed-in kiro-cli"]
async fn live_session_leaves_nothing_behind() {
    let conn = connect().await;
    let base = tempfile::tempdir().unwrap();
    let temp = TempDirs::new(base.path().join("kiro-spotlight"));
    let cwd = temp.create().unwrap();
    let store = KiroStore::under(&cli::home().join(".kiro"));

    let created = conn.request("session/new", json!({ "cwd": cwd, "mcpServers": [] })).await.unwrap();
    let session_id = created["sessionId"].as_str().unwrap().to_string();
    let prompt = json!({ "sessionId": session_id, "prompt": [{ "type": "text", "text": "Reply with exactly: OK" }] });
    let stop = conn.request("session/prompt", prompt).await.unwrap();
    println!("stopReason={}", stop["stopReason"]);

    let hash = store.find_cwd_hash(&session_id).expect("kiro-cli stored the session");
    println!("cwdHash={hash}");
    conn.request("session/delete", json!({ "sessionId": session_id })).await.unwrap();
    let report = store.sweep(&hash, &session_id).unwrap();
    println!("{report:?}");
    temp.remove(&cwd).unwrap();
    conn.shutdown().await;

    assert!(!store.sessions.join(&hash).exists(), "session dir left behind");
    assert!(!store.index.join(format!("{hash}.jsonl")).exists(), "index left behind");
    assert!(store.find_cwd_hash(&session_id).is_none(), "session id still referenced");
    assert!(!cwd.exists());
}

#[tokio::test]
#[ignore = "needs a signed-in kiro-cli"]
async fn live_session_list_finds_orphans_by_cwd() {
    let conn = connect().await;
    let base = tempfile::tempdir().unwrap();
    let temp = TempDirs::new(base.path().join("kiro-spotlight"));
    let cwd = temp.create().unwrap();
    let store = KiroStore::under(&cli::home().join(".kiro"));

    let created = conn.request("session/new", json!({ "cwd": cwd, "mcpServers": [] })).await.unwrap();
    let session_id = created["sessionId"].as_str().unwrap().to_string();
    let listed = conn.request("session/list", json!({ "cwd": cwd })).await.unwrap();
    let found = listed["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|s| s["sessionId"] == session_id.as_str());
    println!("session/list found new session: {found}");

    let hash = store.find_cwd_hash(&session_id);
    conn.request("session/delete", json!({ "sessionId": session_id })).await.unwrap();
    if let Some(hash) = hash.or_else(|| store.find_cwd_hash(&session_id)) {
        store.sweep(&hash, &session_id).unwrap();
    }
    temp.remove(&cwd).unwrap();
    conn.shutdown().await;
    assert!(found, "session/list did not return the session for its cwd");
}
