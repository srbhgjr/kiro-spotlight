//! Filesystem side of incognito: per-session temp dirs, the pending-cleanup record, and the
//! sweep of kiro-cli's per-cwd session storage after `session/delete`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const INDEX_DIR: &str = ".index";

/// Where kiro-cli keeps ACP sessions: `<sessions>/<cwdHash>/<sessionId>/` and `<index>/<cwdHash>.jsonl`.
#[derive(Debug, Clone)]
pub struct KiroStore {
    pub sessions: PathBuf,
    pub index: PathBuf,
}

impl KiroStore {
    pub fn under(kiro_home: &Path) -> Self {
        Self {
            sessions: kiro_home.join("sessions"),
            index: kiro_home.join("session-index"),
        }
    }

    /// Find the cwd hash directory that holds `session_id`, from the session dir or the index.
    pub fn find_cwd_hash(&self, session_id: &str) -> Option<String> {
        self.hash_from_sessions(session_id)
            .or_else(|| self.hash_from_index(session_id))
    }

    fn hash_from_sessions(&self, session_id: &str) -> Option<String> {
        fs::read_dir(&self.sessions)
            .ok()?
            .flatten()
            .find(|entry| entry.path().join(session_id).is_dir())
            .and_then(|entry| entry.file_name().into_string().ok())
            .filter(|hash| is_valid_hash(hash))
    }

    fn hash_from_index(&self, session_id: &str) -> Option<String> {
        fs::read_dir(&self.index)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
            .find(|path| fs::read_to_string(path).is_ok_and(|text| text.contains(session_id)))
            .and_then(|path| path.file_stem()?.to_str().map(str::to_string))
            .filter(|hash| is_valid_hash(hash))
    }

    /// Remove the hash dir and index file, but only if they belong to `session_id` alone.
    pub fn sweep(&self, hash: &str, session_id: &str) -> io::Result<SweepReport> {
        if !is_valid_hash(hash) {
            return Ok(SweepReport::default());
        }
        let dir = self.sessions.join(hash);
        let index = self.index.join(format!("{hash}.jsonl"));
        let mut report = SweepReport::default();
        if dir.is_dir() && dir_is_exclusive(&dir, session_id)? {
            fs::remove_dir_all(&dir)?;
            report.removed_dir = true;
        }
        if index.is_file() && index_is_exclusive(&index, hash, session_id)? {
            fs::remove_file(&index)?;
            report.removed_index = true;
        }
        Ok(report)
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct SweepReport {
    pub removed_dir: bool,
    pub removed_index: bool,
}

fn is_valid_hash(hash: &str) -> bool {
    (8..=64).contains(&hash.len()) && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

fn dir_is_exclusive(dir: &Path, session_id: &str) -> io::Result<bool> {
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name();
        if name != INDEX_DIR && name != session_id {
            return Ok(false);
        }
    }
    Ok(true)
}

fn index_is_exclusive(index: &Path, hash: &str, session_id: &str) -> io::Result<bool> {
    let ours = format!("{hash}/{session_id}");
    let text = fs::read_to_string(index)?;
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .all(|line| session_path(line).as_deref() == Some(ours.as_str())))
}

fn session_path(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    value.get("sessionPath")?.as_str().map(str::to_string)
}

/// Temp working directories, one per conversation, under a single base dir.
#[derive(Debug, Clone)]
pub struct TempDirs {
    pub base: PathBuf,
}

impl TempDirs {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    pub fn create(&self) -> io::Result<PathBuf> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = self.base.join(format!("s-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Remove `dir` only if it is a direct child of the base dir.
    pub fn remove(&self, dir: &Path) -> io::Result<()> {
        if dir.parent() != Some(self.base.as_path()) || !dir.exists() {
            return Ok(());
        }
        fs::remove_dir_all(dir)
    }

    pub fn leftovers(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.base)
            .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingSession {
    pub session_id: String,
    pub cwd: PathBuf,
}

/// Sessions created but not yet cleaned up, so a crash can be recovered at next launch.
#[derive(Debug, Clone)]
pub struct PendingStore {
    pub path: PathBuf,
}

impl PendingStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Vec<PendingSession> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn add(&self, entry: PendingSession) -> io::Result<()> {
        let mut all = self.load();
        all.retain(|e| e.session_id != entry.session_id);
        all.push(entry);
        self.save(&all)
    }

    pub fn remove(&self, session_id: &str) -> io::Result<()> {
        let mut all = self.load();
        all.retain(|e| e.session_id != session_id);
        self.save(&all)
    }

    fn save(&self, all: &[PendingSession]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        if all.is_empty() {
            return match fs::remove_file(&self.path) {
                Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            };
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(all).unwrap_or_default())?;
        fs::rename(tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "a04ff4a97e5affac";
    const SID: &str = "sess_fe5a1535-f557-4634-86bd-4dacaa18b989";

    fn store_with_session(root: &Path) -> KiroStore {
        let store = KiroStore::under(root);
        fs::create_dir_all(store.sessions.join(HASH).join(SID)).unwrap();
        fs::create_dir_all(store.sessions.join(HASH).join(INDEX_DIR)).unwrap();
        fs::write(store.sessions.join(HASH).join(SID).join("messages.jsonl"), "{}").unwrap();
        fs::create_dir_all(&store.index).unwrap();
        let lines = format!(
            "{{\"op\":\"add\",\"sessionPath\":\"{HASH}/{SID}\",\"at\":1}}\n{{\"op\":\"remove\",\"sessionPath\":\"{HASH}/{SID}\",\"at\":2}}\n"
        );
        fs::write(store.index.join(format!("{HASH}.jsonl")), lines).unwrap();
        store
    }

    #[test]
    fn finds_hash_from_session_dir_then_index() {
        let root = tempfile::tempdir().unwrap();
        let store = store_with_session(root.path());
        assert_eq!(store.find_cwd_hash(SID).as_deref(), Some(HASH));

        fs::remove_dir_all(store.sessions.join(HASH).join(SID)).unwrap();
        assert_eq!(store.find_cwd_hash(SID).as_deref(), Some(HASH));
        assert_eq!(store.find_cwd_hash("sess_other"), None);
    }

    #[test]
    fn sweep_removes_everything_for_an_exclusive_session() {
        let root = tempfile::tempdir().unwrap();
        let store = store_with_session(root.path());
        let report = store.sweep(HASH, SID).unwrap();
        assert_eq!(report, SweepReport { removed_dir: true, removed_index: true });
        assert!(!store.sessions.join(HASH).exists());
        assert!(!store.index.join(format!("{HASH}.jsonl")).exists());
    }

    #[test]
    fn sweep_leaves_storage_shared_with_another_session() {
        let root = tempfile::tempdir().unwrap();
        let store = store_with_session(root.path());
        fs::create_dir_all(store.sessions.join(HASH).join("sess_someone_else")).unwrap();
        let index = store.index.join(format!("{HASH}.jsonl"));
        let mut text = fs::read_to_string(&index).unwrap();
        text.push_str(&format!("{{\"op\":\"add\",\"sessionPath\":\"{HASH}/sess_someone_else\"}}\n"));
        fs::write(&index, text).unwrap();

        let report = store.sweep(HASH, SID).unwrap();
        assert_eq!(report, SweepReport::default());
        assert!(store.sessions.join(HASH).exists());
        assert!(index.exists());
    }

    #[test]
    fn sweep_rejects_path_like_hashes() {
        let root = tempfile::tempdir().unwrap();
        let store = store_with_session(root.path());
        assert_eq!(store.sweep("../sessions", SID).unwrap(), SweepReport::default());
        assert_eq!(store.sweep("", SID).unwrap(), SweepReport::default());
        assert!(store.sessions.join(HASH).exists());
    }

    #[test]
    fn temp_dirs_only_remove_their_own_children() {
        let root = tempfile::tempdir().unwrap();
        let dirs = TempDirs::new(root.path().join("kiro-spotlight"));
        let made = dirs.create().unwrap();
        assert!(made.is_dir());
        assert_eq!(dirs.leftovers(), vec![made.clone()]);

        let outsider = root.path().join("keep-me");
        fs::create_dir_all(&outsider).unwrap();
        dirs.remove(&outsider).unwrap();
        assert!(outsider.exists());

        dirs.remove(&made).unwrap();
        assert!(!made.exists());
    }

    #[test]
    fn pending_store_round_trips_and_deletes_when_empty() {
        let root = tempfile::tempdir().unwrap();
        let store = PendingStore::new(root.path().join("state").join("pending.json"));
        assert!(store.load().is_empty());

        let entry = PendingSession { session_id: SID.into(), cwd: root.path().join("cwd") };
        store.add(entry.clone()).unwrap();
        store.add(entry.clone()).unwrap();
        assert_eq!(store.load(), vec![entry]);

        store.remove(SID).unwrap();
        assert!(store.load().is_empty());
        assert!(!store.path.exists());
    }
}
