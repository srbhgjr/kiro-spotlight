mod acp;
mod cleanup;
mod cli;
mod commands;
mod panel;
mod session;
mod settings;
mod theme;
mod tray;

#[cfg(test)]
mod live_tests;

use std::error::Error;

use tauri::{ActivationPolicy, App, AppHandle, Emitter, Manager, RunEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use session::SessionManager;
use settings::Settings;
use theme::watch::ThemeWatcher;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::start_session,
            commands::send_prompt,
            commands::cancel_turn,
            commands::answer_permission,
            commands::new_chat,
            commands::dismiss,
            commands::hide,
            commands::get_theme,
            commands::hotkey_label,
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while building Kiro Spotlight")
        .run(on_run_event);
}

fn setup(app: &mut App) -> Result<(), Box<dyn Error>> {
    app.set_activation_policy(ActivationPolicy::Accessory);
    let settings = Settings::load(&app.path().app_config_dir()?.join("settings.json"));
    let manager = SessionManager::new(app.handle().clone(), settings.clone(), app.path().app_data_dir()?);
    app.manage(manager.clone());
    app.manage(settings.clone());

    panel::install(app.handle())?;
    tray::install(app.handle(), &settings.hotkey)?;
    register_hotkey(app.handle(), &settings.hotkey)?;
    if let Some(watcher) = watch_theme(app.handle(), &settings) {
        app.manage(watcher);
    }
    tauri::async_runtime::spawn(async move { manager.recover().await });
    Ok(())
}

fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), Box<dyn Error>> {
    let fallback = Shortcut::new(Some(Modifiers::ALT), Code::Space);
    let shortcut = hotkey.parse::<Shortcut>().unwrap_or(fallback);
    app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            panel::toggle(app);
        }
    })?;
    Ok(())
}

fn watch_theme(app: &AppHandle, settings: &Settings) -> Option<ThemeWatcher> {
    let handle = app.clone();
    theme::watch::start(settings.theme_source, move |payload| {
        panel::apply_appearance(&handle, &payload);
        let _ = handle.emit(theme::EVENT_THEME_CHANGED, payload);
    })
    .ok()
}

/// Keep running in the menu bar when no window is open; only an explicit quit exits.
fn on_run_event(_app: &AppHandle, event: RunEvent) {
    if let RunEvent::ExitRequested { code: None, api, .. } = event {
        api.prevent_exit();
    }
}
