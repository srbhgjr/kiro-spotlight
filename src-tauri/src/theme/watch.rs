//! Re-resolve and push the theme when Kiro IDE or Kiro Crew settings change on disk.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use super::{crew, ide, resolve, ThemePayload, EVENT_THEME_CHANGED};
use crate::settings::ThemeSource;

const DEBOUNCE: Duration = Duration::from_millis(200);

/// Keeps the file watcher alive for as long as it is held.
pub struct ThemeWatcher {
    _watcher: RecommendedWatcher,
}

pub fn start(app: AppHandle, source: ThemeSource) -> notify::Result<ThemeWatcher> {
    let files = [ide::settings_path(), crew::config_path()];
    let (tx, rx) = mpsc::channel::<()>();
    let watched = files.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        if event.is_ok_and(|e| touches(&e, &watched)) {
            let _ = tx.send(());
        }
    })?;
    for dir in files.iter().filter_map(|f| f.parent()).filter(|d| d.is_dir()) {
        watcher.watch(dir, RecursiveMode::NonRecursive)?;
    }
    std::thread::spawn(move || debounce_loop(rx, app, source));
    Ok(ThemeWatcher { _watcher: watcher })
}

fn touches(event: &Event, files: &[PathBuf]) -> bool {
    event.paths.iter().any(|p| files.iter().any(|f| p.file_name() == f.file_name() && p.parent() == f.parent()))
}

fn debounce_loop(rx: mpsc::Receiver<()>, app: AppHandle, source: ThemeSource) {
    let mut last: Option<ThemePayload> = Some(resolve(source));
    while rx.recv().is_ok() {
        while rx.recv_timeout(DEBOUNCE).is_ok() {}
        let next = resolve(source);
        if last.as_ref() != Some(&next) {
            let _ = app.emit(EVENT_THEME_CHANGED, next.clone());
            last = Some(next);
        }
    }
}
