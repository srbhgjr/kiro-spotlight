//! Ephemeral conversation lifecycle on top of one long-lived `kiro-cli acp` process.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex as AsyncMutex, OnceCell};

use crate::acp::{protocol, AcpConnection, InboundHandler};
use crate::cleanup::{KiroStore, PendingSession, PendingStore, TempDirs};
use crate::cli;
use crate::settings::Settings;

pub const EVENT_UPDATE: &str = "acp://update";
pub const EVENT_PERMISSION: &str = "acp://permission";
pub const EVENT_AGENT_EXITED: &str = "acp://exited";
pub const EVENT_SESSION_ENDED: &str = "session://ended";

const DELETE_TIMEOUT: Duration = Duration::from_secs(5);
const CANCEL_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
struct ActiveSession {
    id: String,
    cwd: PathBuf,
    cwd_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub agent_name: String,
    pub agent_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PermissionEvent {
    request_id: String,
    session_id: Value,
    tool_call: Value,
    options: Value,
}

pub struct SessionManager {
    app: AppHandle,
    settings: Settings,
    store: KiroStore,
    temp: TempDirs,
    pending: PendingStore,
    path_env: OnceCell<Option<String>>,
    agent: AsyncMutex<Option<Arc<AcpConnection>>>,
    agent_info: Mutex<(String, String)>,
    active: AsyncMutex<Option<ActiveSession>>,
    permissions: Mutex<HashMap<String, Value>>,
    turn_active: AtomicBool,
    hidden_generation: AtomicU64,
}

impl SessionManager {
    pub fn new(app: AppHandle, settings: Settings, state_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            app,
            settings,
            store: KiroStore::under(&cli::home().join(".kiro")),
            temp: TempDirs::new(std::env::temp_dir().join("kiro-spotlight")),
            pending: PendingStore::new(state_dir.join("pending-cleanup.json")),
            path_env: OnceCell::new(),
            agent: AsyncMutex::new(None),
            agent_info: Mutex::new((String::new(), String::new())),
            active: AsyncMutex::new(None),
            permissions: Mutex::new(HashMap::new()),
            turn_active: AtomicBool::new(false),
            hidden_generation: AtomicU64::new(0),
        })
    }

    /// Start (or reuse) the conversation and return its id.
    pub async fn start_session(self: &Arc<Self>) -> Result<SessionInfo, String> {
        let conn = self.ensure_agent().await?;
        let mut active = self.active.lock().await;
        if let Some(session) = active.as_ref() {
            return Ok(self.info(&session.id));
        }
        let session = self.create_session(&conn).await?;
        let info = self.info(&session.id);
        *active = Some(session);
        Ok(info)
    }

    /// Send one user message and wait for the turn to finish. Streamed output arrives as events.
    pub async fn prompt(self: &Arc<Self>, text: String) -> Result<String, String> {
        let session_id = self.start_session().await?.session_id;
        let conn = self.current_agent().await?;
        self.turn_active.store(true, Ordering::SeqCst);
        let params = json!({ "sessionId": session_id, "prompt": [{ "type": "text", "text": text }] });
        let outcome = conn.request("session/prompt", params).await;
        self.turn_active.store(false, Ordering::SeqCst);
        self.remember_cwd_hash(&session_id).await;
        let result = outcome.map_err(|e| e.to_string())?;
        Ok(result["stopReason"].as_str().unwrap_or("end_turn").to_string())
    }

    pub async fn cancel_turn(&self) -> Result<(), String> {
        let id = self.active.lock().await.as_ref().map(|s| s.id.clone());
        match id {
            Some(id) => self.cancel_turn_for(&id).await,
            None => {
                self.cancel_permissions().await;
                Ok(())
            }
        }
    }

    pub async fn answer_permission(&self, request_id: String, option_id: Option<String>) -> Result<(), String> {
        let Some(id) = self.lock_permissions().remove(&request_id) else {
            return Ok(());
        };
        let result = match option_id.as_deref() {
            Some(option) => protocol::permission_selected(option),
            None => protocol::permission_cancelled(),
        };
        let conn = self.current_agent().await?;
        conn.respond(&id, result).await.map_err(|e| e.to_string())
    }

    /// End the conversation: cancel, `session/delete`, sweep kiro-cli storage, remove the temp cwd.
    pub async fn end_session(&self) -> Result<(), String> {
        let Some(session) = self.active.lock().await.take() else {
            return Ok(());
        };
        let _ = self.cancel_turn_for(&session.id).await;
        self.wait_for_turn_end().await;
        let _ = self.app.emit(EVENT_SESSION_ENDED, session.id.clone());
        let Ok(conn) = self.current_agent().await else {
            return Ok(());
        };
        self.delete_session(&conn, &session.id, session.cwd_hash.clone()).await;
        let _ = self.temp.remove(&session.cwd);
        let _ = self.pending.remove(&session.id);
        Ok(())
    }

    /// Called when the panel is shown; cancels any pending idle expiry.
    pub fn mark_shown(&self) {
        self.hidden_generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Called when the panel is hidden without ending the chat; ends it after `idle_minutes`.
    pub fn schedule_expiry(self: &Arc<Self>) {
        let generation = self.hidden_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let delay = Duration::from_secs(self.settings.idle_minutes.max(1) * 60);
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(delay).await;
            if this.hidden_generation.load(Ordering::SeqCst) == generation {
                let _ = this.end_session().await;
            }
        });
    }

    /// Clean up any sessions a previous run left behind, then stop the agent.
    pub async fn shutdown(&self) {
        let _ = self.end_session().await;
        if let Some(conn) = self.agent.lock().await.take() {
            conn.shutdown().await;
        }
    }

    pub async fn recover(self: &Arc<Self>) {
        let has_leftovers = !self.pending.load().is_empty() || !self.temp.leftovers().is_empty();
        if has_leftovers {
            let _ = self.ensure_agent().await;
        }
    }

    async fn ensure_agent(self: &Arc<Self>) -> Result<Arc<AcpConnection>, String> {
        let mut agent = self.agent.lock().await;
        if let Some(conn) = agent.as_ref().filter(|c| c.is_alive()) {
            return Ok(conn.clone());
        }
        let conn = self.spawn_agent().await?;
        self.recover_with(&conn).await;
        *agent = Some(conn.clone());
        Ok(conn)
    }

    async fn spawn_agent(self: &Arc<Self>) -> Result<Arc<AcpConnection>, String> {
        let path_env = self.path_env.get_or_init(cli::login_shell_path).await.clone();
        let program = cli::locate(self.settings.cli_path.as_deref(), path_env.as_deref())
            .ok_or("kiro-cli was not found. Install it and run `kiro-cli login`.")?;
        cli::ensure_agent_profile().map_err(|e| format!("could not write agent profile: {e}"))?;
        let args = cli::agent_args(self.settings.model.as_deref());
        let handler = Arc::new(Bridge { app: self.app.clone() });
        let conn = AcpConnection::spawn(&program, &args, path_env.as_deref(), handler)
            .map_err(|e| e.to_string())?;
        let init = conn
            .request("initialize", protocol::initialize_params(env!("CARGO_PKG_VERSION")))
            .await
            .map_err(|e| format!("kiro-cli did not initialise: {e}"))?;
        self.store_agent_info(&init);
        Ok(conn)
    }

    async fn create_session(&self, conn: &AcpConnection) -> Result<ActiveSession, String> {
        let cwd = self.temp.create().map_err(|e| e.to_string())?;
        let params = json!({ "cwd": cwd, "mcpServers": [] });
        let result = match conn.request("session/new", params).await {
            Ok(result) => result,
            Err(err) => {
                let _ = self.temp.remove(&cwd);
                return Err(err.to_string());
            }
        };
        let id = result["sessionId"]
            .as_str()
            .ok_or("session/new returned no sessionId")?
            .to_string();
        let _ = self.pending.add(PendingSession { session_id: id.clone(), cwd: cwd.clone() });
        Ok(ActiveSession { id, cwd, cwd_hash: None })
    }

    async fn delete_session(&self, conn: &AcpConnection, session_id: &str, known_hash: Option<String>) {
        let hash = known_hash.or_else(|| self.store.find_cwd_hash(session_id));
        let delete = conn.request("session/delete", json!({ "sessionId": session_id }));
        let _ = tokio::time::timeout(DELETE_TIMEOUT, delete).await;
        let hash = hash.or_else(|| self.store.find_cwd_hash(session_id));
        if let Some(hash) = hash {
            let _ = self.store.sweep(&hash, session_id);
        }
    }

    async fn recover_with(&self, conn: &AcpConnection) {
        for entry in self.pending.load() {
            self.delete_session(conn, &entry.session_id, None).await;
            let _ = self.temp.remove(&entry.cwd);
            let _ = self.pending.remove(&entry.session_id);
        }
        for dir in self.temp.leftovers() {
            for session_id in list_sessions_in(conn, &dir).await {
                self.delete_session(conn, &session_id, None).await;
            }
            let _ = self.temp.remove(&dir);
        }
    }

    async fn remember_cwd_hash(&self, session_id: &str) {
        let mut active = self.active.lock().await;
        let Some(session) = active.as_mut().filter(|s| s.id == session_id) else {
            return;
        };
        if session.cwd_hash.is_none() {
            session.cwd_hash = self.store.find_cwd_hash(session_id);
        }
    }

    async fn cancel_turn_for(&self, session_id: &str) -> Result<(), String> {
        self.cancel_permissions().await;
        if !self.turn_active.load(Ordering::SeqCst) {
            return Ok(());
        }
        let conn = self.current_agent().await?;
        conn.notify("session/cancel", json!({ "sessionId": session_id }))
            .await
            .map_err(|e| e.to_string())
    }

    async fn wait_for_turn_end(&self) {
        let deadline = tokio::time::Instant::now() + CANCEL_GRACE;
        while self.turn_active.load(Ordering::SeqCst) && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn cancel_permissions(&self) {
        let ids: Vec<Value> = self.lock_permissions().drain().map(|(_, id)| id).collect();
        let Ok(conn) = self.current_agent().await else {
            return;
        };
        for id in ids {
            let _ = conn.respond(&id, protocol::permission_cancelled()).await;
        }
    }

    async fn current_agent(&self) -> Result<Arc<AcpConnection>, String> {
        self.agent
            .lock()
            .await
            .clone()
            .filter(|c| c.is_alive())
            .ok_or_else(|| "kiro-cli is not running".to_string())
    }

    async fn on_agent_request(&self, id: Value, method: String, params: Value) {
        if method == "session/request_permission" {
            self.queue_permission(id, params);
            return;
        }
        if let Ok(conn) = self.current_agent().await {
            let _ = conn
                .respond_error(&id, protocol::METHOD_NOT_FOUND, "not supported by Kiro Spotlight")
                .await;
        }
    }

    fn queue_permission(&self, id: Value, params: Value) {
        let request_id = id.to_string();
        self.lock_permissions().insert(request_id.clone(), id);
        let event = PermissionEvent {
            request_id,
            session_id: params["sessionId"].clone(),
            tool_call: params["toolCall"].clone(),
            options: params["options"].clone(),
        };
        let _ = self.app.emit(EVENT_PERMISSION, event);
    }

    async fn on_agent_exit(&self) {
        self.turn_active.store(false, Ordering::SeqCst);
        self.lock_permissions().clear();
        self.active.lock().await.take();
        let _ = self.app.emit(EVENT_AGENT_EXITED, ());
    }

    fn store_agent_info(&self, init: &Value) {
        let name = init["agentInfo"]["title"].as_str().unwrap_or("Kiro").to_string();
        let version = init["agentInfo"]["version"].as_str().unwrap_or_default().to_string();
        *self.agent_info.lock().unwrap_or_else(|p| p.into_inner()) = (name, version);
    }

    fn info(&self, session_id: &str) -> SessionInfo {
        let (agent_name, agent_version) = self.agent_info.lock().unwrap_or_else(|p| p.into_inner()).clone();
        SessionInfo { session_id: session_id.to_string(), agent_name, agent_version }
    }

    fn lock_permissions(&self) -> std::sync::MutexGuard<'_, HashMap<String, Value>> {
        self.permissions.lock().unwrap_or_else(|p| p.into_inner())
    }
}

async fn list_sessions_in(conn: &AcpConnection, cwd: &std::path::Path) -> Vec<String> {
    let request = conn.request("session/list", json!({ "cwd": cwd }));
    let Ok(Ok(result)) = tokio::time::timeout(DELETE_TIMEOUT, request).await else {
        return Vec::new();
    };
    session_ids_for_cwd(&result, cwd)
}

fn session_ids_for_cwd(result: &Value, cwd: &std::path::Path) -> Vec<String> {
    let cwd = cwd.to_string_lossy();
    result["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["cwd"].as_str() == Some(cwd.as_ref()))
        .filter_map(|s| s["sessionId"].as_str().map(str::to_string))
        .collect()
}

struct Bridge {
    app: AppHandle,
}

impl Bridge {
    fn manager(&self) -> Arc<SessionManager> {
        self.app.state::<Arc<SessionManager>>().inner().clone()
    }
}

impl InboundHandler for Bridge {
    fn on_notification(&self, method: String, params: Value) {
        if method == "session/update" {
            let _ = self.app.emit(EVENT_UPDATE, params);
        }
    }

    fn on_request(&self, id: Value, method: String, params: Value) {
        let manager = self.manager();
        tauri::async_runtime::spawn(async move {
            manager.on_agent_request(id, method, params).await;
        });
    }

    fn on_exit(&self) {
        let manager = self.manager();
        tauri::async_runtime::spawn(async move {
            manager.on_agent_exit().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn session_list_is_filtered_to_the_exact_cwd() {
        let result = json!({
            "sessions": [
                { "sessionId": "a", "cwd": "/tmp/kiro-spotlight/s-1" },
                { "sessionId": "b", "cwd": "/Users/me/project" },
                { "sessionId": "c", "cwd": "/tmp/kiro-spotlight/s-1" }
            ]
        });
        let ids = session_ids_for_cwd(&result, Path::new("/tmp/kiro-spotlight/s-1"));
        assert_eq!(ids, vec!["a", "c"]);
        assert!(session_ids_for_cwd(&json!({}), Path::new("/x")).is_empty());
    }
}
