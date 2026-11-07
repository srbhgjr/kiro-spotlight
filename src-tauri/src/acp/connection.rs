//! A running `kiro-cli acp` child process and request/response correlation over its stdio.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, Mutex as AsyncMutex};

use super::protocol::{self, Inbound, RpcError};

#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    #[error("could not start kiro-cli: {0}")]
    Spawn(std::io::Error),
    #[error("kiro-cli i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("kiro-cli exited")]
    Closed,
    #[error("kiro-cli returned an error: {0}")]
    Rpc(#[from] RpcError),
}

/// Receives agent-initiated traffic. Called on the reader task; implementations must not block.
pub trait InboundHandler: Send + Sync + 'static {
    fn on_notification(&self, method: String, params: Value);
    fn on_request(&self, id: Value, method: String, params: Value);
    fn on_exit(&self);
}

type Pending = Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>;

pub struct AcpConnection {
    stdin: AsyncMutex<ChildStdin>,
    child: AsyncMutex<Child>,
    pending: Pending,
    next_id: AtomicU64,
    alive: AtomicBool,
}

impl AcpConnection {
    /// Spawn the agent and start the stdout reader task.
    pub fn spawn(
        program: &Path,
        args: &[String],
        path_env: Option<&str>,
        handler: Arc<dyn InboundHandler>,
    ) -> Result<Arc<Self>, AcpError> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(path) = path_env {
            cmd.env("PATH", path);
        }
        let mut child = cmd.spawn().map_err(AcpError::Spawn)?;
        let stdin = child.stdin.take().ok_or(AcpError::Closed)?;
        let stdout = child.stdout.take().ok_or(AcpError::Closed)?;
        let conn = Arc::new(Self {
            stdin: AsyncMutex::new(stdin),
            child: AsyncMutex::new(child),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            alive: AtomicBool::new(true),
        });
        tokio::spawn(read_loop(conn.clone(), stdout, handler));
        Ok(conn)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Send a request and wait for its response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, AcpError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.lock_pending().insert(id, tx);
        if let Err(err) = self.write(protocol::request(id, method, params)).await {
            self.lock_pending().remove(&id);
            return Err(err);
        }
        let outcome = rx.await.map_err(|_| AcpError::Closed)?;
        Ok(outcome?)
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<(), AcpError> {
        self.write(protocol::notification(method, params)).await
    }

    pub async fn respond(&self, id: &Value, result: Value) -> Result<(), AcpError> {
        self.write(protocol::result(id, result)).await
    }

    pub async fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<(), AcpError> {
        self.write(protocol::error(id, code, message)).await
    }

    pub async fn shutdown(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let _ = self.child.lock().await.start_kill();
    }

    async fn write(&self, line: String) -> Result<(), AcpError> {
        if !self.is_alive() {
            return Err(AcpError::Closed);
        }
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>> {
        self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn resolve(&self, id: u64, outcome: Result<Value, RpcError>) {
        if let Some(tx) = self.lock_pending().remove(&id) {
            let _ = tx.send(outcome);
        }
    }

    fn fail_all(&self) {
        self.lock_pending().clear();
    }
}

async fn read_loop(conn: Arc<AcpConnection>, stdout: ChildStdout, handler: Arc<dyn InboundHandler>) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        dispatch(&conn, handler.as_ref(), &line);
    }
    conn.alive.store(false, Ordering::SeqCst);
    conn.fail_all();
    handler.on_exit();
}

fn dispatch(conn: &AcpConnection, handler: &dyn InboundHandler, line: &str) {
    match protocol::parse_line(line) {
        Some(Inbound::Response { id, outcome }) => conn.resolve(id, outcome),
        Some(Inbound::Request { id, method, params }) => handler.on_request(id, method, params),
        Some(Inbound::Notification { method, params }) => handler.on_notification(method, params),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct Recorder {
        notifications: AtomicUsize,
        requests: AtomicUsize,
        exited: AtomicBool,
    }

    impl InboundHandler for Recorder {
        fn on_notification(&self, _method: String, _params: Value) {
            self.notifications.fetch_add(1, Ordering::SeqCst);
        }
        fn on_request(&self, _id: Value, _method: String, _params: Value) {
            self.requests.fetch_add(1, Ordering::SeqCst);
        }
        fn on_exit(&self) {
            self.exited.store(true, Ordering::SeqCst);
        }
    }

    const FAKE_AGENT: &str = r#"read line
echo '{"jsonrpc":"2.0","method":"session/update","params":{}}'
echo '{"jsonrpc":"2.0","id":"perm-1","method":"session/request_permission","params":{}}'
echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1}}'
read line
echo '{"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"boom"}}'
"#;

    fn spawn_fake(handler: Arc<Recorder>) -> Arc<AcpConnection> {
        let args = vec!["-c".to_string(), FAKE_AGENT.to_string()];
        AcpConnection::spawn(Path::new("/bin/sh"), &args, None, handler).expect("spawn fake agent")
    }

    #[tokio::test]
    async fn correlates_responses_and_routes_agent_traffic() {
        let recorder = Arc::new(Recorder::default());
        let conn = spawn_fake(recorder.clone());

        let first = conn.request("initialize", json!({})).await.expect("initialize");
        assert_eq!(first["protocolVersion"], 1);

        let second = conn.request("session/new", json!({})).await;
        assert!(matches!(second, Err(AcpError::Rpc(ref e)) if e.code == -32000));

        assert_eq!(recorder.notifications.load(Ordering::SeqCst), 1);
        assert_eq!(recorder.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn pending_requests_fail_when_agent_exits() {
        let recorder = Arc::new(Recorder::default());
        let args = vec!["-c".to_string(), "read line".to_string()];
        let conn = AcpConnection::spawn(Path::new("/bin/sh"), &args, None, recorder.clone()).unwrap();

        let outcome = conn.request("initialize", json!({})).await;
        assert!(matches!(outcome, Err(AcpError::Closed)));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(recorder.exited.load(Ordering::SeqCst));
        assert!(!conn.is_alive());
    }
}
