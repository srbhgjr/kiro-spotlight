//! Tauri commands invoked by the panel UI.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::panel::{self, HideReason};
use crate::session::{SessionInfo, SessionManager};
use crate::settings::Settings;
use crate::theme::{self, ThemePayload};

type Manager<'a> = State<'a, Arc<SessionManager>>;

#[tauri::command]
pub async fn start_session(manager: Manager<'_>) -> Result<SessionInfo, String> {
    manager.start_session().await
}

#[tauri::command]
pub async fn send_prompt(manager: Manager<'_>, text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("empty message".into());
    }
    manager.prompt(text).await
}

#[tauri::command]
pub async fn cancel_turn(manager: Manager<'_>) -> Result<(), String> {
    manager.cancel_turn().await
}

#[tauri::command]
pub async fn answer_permission(
    manager: Manager<'_>,
    request_id: String,
    option_id: Option<String>,
) -> Result<(), String> {
    manager.answer_permission(request_id, option_id).await
}

#[tauri::command]
pub async fn new_chat(manager: Manager<'_>) -> Result<(), String> {
    manager.end_session().await
}

#[tauri::command]
pub fn dismiss(app: AppHandle) {
    panel::hide(&app, HideReason::Dismiss);
}

#[tauri::command]
pub fn hide(app: AppHandle) {
    panel::hide(&app, HideReason::Toggle);
}

#[tauri::command]
pub fn get_theme(app: AppHandle, settings: State<'_, Settings>) -> ThemePayload {
    let payload = theme::resolve(settings.theme_source);
    panel::apply_appearance(&app, &payload);
    payload
}

#[tauri::command]
pub fn hotkey_label(settings: State<'_, Settings>) -> String {
    settings.hotkey.clone()
}
