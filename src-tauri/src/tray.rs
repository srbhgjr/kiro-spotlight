//! Menu-bar item: open, new chat, quit.

use std::sync::Arc;

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::panel;
use crate::session::SessionManager;

const ICON: &[u8] = include_bytes!("../icons/tray.png");

pub fn install(app: &AppHandle, hotkey: &str) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Kiro Spotlight", true, Some(hotkey))?;
    let new_chat = MenuItem::with_id(app, "new", "New Chat", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Kiro Spotlight", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &new_chat, &separator, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .tooltip("Kiro Spotlight")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu)
        .build(app)?;
    Ok(())
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open" => panel::show(app),
        "new" => new_chat(app),
        "quit" => quit(app),
        _ => {}
    }
}

fn new_chat(app: &AppHandle) {
    let app = app.clone();
    let manager = manager(&app);
    tauri::async_runtime::spawn(async move {
        let _ = manager.end_session().await;
        panel::show(&app);
    });
}

/// End and clean up the conversation, stop kiro-cli, then exit.
pub fn quit(app: &AppHandle) {
    let app = app.clone();
    let manager = manager(&app);
    tauri::async_runtime::spawn(async move {
        manager.shutdown().await;
        app.exit(0);
    });
}

fn manager(app: &AppHandle) -> Arc<SessionManager> {
    app.state::<Arc<SessionManager>>().inner().clone()
}
