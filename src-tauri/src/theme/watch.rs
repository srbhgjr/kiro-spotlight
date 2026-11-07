//! Re-resolve the theme when Kiro IDE or Kiro Crew settings change on disk.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::{crew, ide, resolve, ThemePayload};
use crate::settings::ThemeSource;

const DEBOUNCE: Duration = Duration::from_millis(200);

/// Keeps the file watcher alive for as long as it is held.
pub struct ThemeWatcher {
    _watcher: RecommendedWatcher,
}

/// Watch the settings files and call `on_change` with the new theme when it actually changes.
pub fn start<F>(source: ThemeSource, on_change: F) -> notify::Result<ThemeWatcher>
where
    F: Fn(ThemePayload) + Send + 'static,
{
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
    std::thread::spawn(move || debounce_loop(rx, source, on_change));
    Ok(ThemeWatcher { _watcher: watcher })
}

fn touches(event: &Event, files: &[PathBuf]) -> bool {
    event
        .paths
        .iter()
        .any(|p| files.iter().any(|f| p.file_name() == f.file_name() && p.parent() == f.parent()))
}

fn debounce_loop<F: Fn(ThemePayload)>(rx: mpsc::Receiver<()>, source: ThemeSource, on_change: F) {
    let mut last = resolve(source);
    while rx.recv().is_ok() {
        while rx.recv_timeout(DEBOUNCE).is_ok() {}
        let next = resolve(source);
        if next != last {
            on_change(next.clone());
            last = next;
        }
    }
}
