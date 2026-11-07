#[allow(dead_code)]
mod acp;
#[allow(dead_code)]
mod cleanup;
#[allow(dead_code)]
mod cli;
#[allow(dead_code)]
mod session;
#[allow(dead_code)]
mod settings;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running Kiro Spotlight");
}
