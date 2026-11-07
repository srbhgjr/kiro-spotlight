#[allow(dead_code)]
mod acp;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running Kiro Spotlight");
}
