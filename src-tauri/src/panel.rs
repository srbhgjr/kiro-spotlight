//! The Spotlight-style NSPanel: creation, placement, show/hide and appearance.
#![allow(clippy::unused_unit)]

use std::sync::Arc;

use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, Theme, WebviewWindow};
use tauri_nspanel::{tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt};

use crate::session::SessionManager;
use crate::theme::{Mode, ThemePayload};

pub const LABEL: &str = "main";
pub const EVENT_SHOWN: &str = "panel://shown";

const TOP_FRACTION: f64 = 0.22;
const CORNER_RADIUS: f64 = 14.0;

tauri_panel! {
    panel!(SpotlightPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true,
        }
    })

    panel_event!(SpotlightPanelEvents {
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

/// Why the panel is being hidden. Only `Dismiss` ends the conversation immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HideReason {
    Dismiss,
    Toggle,
    FocusLost,
}

/// Convert the main window into a non-activating floating panel with vibrancy.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;
    let _ = window.set_effects(
        EffectsBuilder::new()
            .effect(Effect::Popover)
            .state(EffectState::Active)
            .radius(CORNER_RADIUS)
            .build(),
    );
    let panel = window
        .to_panel::<SpotlightPanel<tauri::Wry>>()
        .map_err(|_| "could not convert the window to a panel".to_string())?;
    panel.set_level(PanelLevel::Floating.value());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .move_to_active_space()
            .into(),
    );
    panel
        .add_style_mask(StyleMask::empty().nonactivating_panel().into())
        .map_err(|_| "could not make the panel non-activating".to_string())?;

    let handler = SpotlightPanelEvents::new();
    let handle = app.clone();
    handler.window_did_resign_key(move |_| hide(&handle, HideReason::FocusLost));
    panel.set_event_handler(Some(handler.as_ref()));
    Ok(())
}

pub fn toggle(app: &AppHandle) {
    if is_visible(app) {
        hide(app, HideReason::Toggle);
    } else {
        show(app);
    }
}

pub fn show(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Ok(panel) = handle.get_webview_panel(LABEL) else {
            return;
        };
        if let Ok(window) = main_window(&handle) {
            let _ = place_near_cursor(&window);
        }
        panel.show_and_make_key();
        manager(&handle).mark_shown();
        let _ = handle.emit(EVENT_SHOWN, ());
    });
}

pub fn hide(app: &AppHandle, reason: HideReason) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Ok(panel) = handle.get_webview_panel(LABEL) else {
            return;
        };
        let was_visible = panel.is_visible();
        if was_visible {
            panel.hide();
        }
        after_hide(&handle, reason, was_visible);
    });
}

fn after_hide(app: &AppHandle, reason: HideReason, was_visible: bool) {
    let manager = manager(app);
    match reason {
        HideReason::Dismiss => {
            tauri::async_runtime::spawn(async move {
                let _ = manager.end_session().await;
            });
        }
        _ if was_visible => manager.schedule_expiry(),
        _ => {}
    }
}

fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_panel(LABEL).map(|p| p.is_visible()).unwrap_or(false)
}

/// Match the native window appearance (vibrancy and `prefers-color-scheme`) to the Kiro theme.
pub fn apply_appearance(app: &AppHandle, theme: &ThemePayload) {
    let Ok(window) = main_window(app) else {
        return;
    };
    let native = match theme.mode {
        Mode::Light => Some(Theme::Light),
        Mode::Dark => Some(Theme::Dark),
        Mode::System => None,
    };
    let _ = window.set_theme(native);
}

fn place_near_cursor(window: &WebviewWindow) -> tauri::Result<()> {
    let primary = window.primary_monitor()?;
    let desktop_scale = primary.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let cursor = window.cursor_position()?.to_logical::<f64>(desktop_scale);
    let Some(monitor) = window
        .monitor_from_point(cursor.x, cursor.y)?
        .or(window.current_monitor()?)
        .or(primary)
    else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let origin = monitor.position().to_logical::<f64>(scale);
    let width = window.outer_size()?.to_logical::<f64>(scale).width;
    let x = origin.x + (size.width - width) / 2.0;
    let y = origin.y + size.height * TOP_FRACTION;
    window.set_position(LogicalPosition::new(x, y))
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(LABEL).ok_or_else(|| "main window missing".to_string())
}

fn manager(app: &AppHandle) -> Arc<SessionManager> {
    app.state::<Arc<SessionManager>>().inner().clone()
}
