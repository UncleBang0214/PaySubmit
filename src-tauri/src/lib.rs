#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_commands;
mod core_config;
mod core_settings;
mod feature_api_crypto;
mod feature_api_debug;
mod feature_api_protocol;
mod feature_api_settings;
mod feature_settings;
mod feature_sm_cert;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![app_commands::invoke_action])
        .setup(|app| {
            app.manage(feature_settings::SettingsState::new(
                app.path()
                    .app_config_dir()
                    .map_err(|_| "无法获取当前Windows用户的应用配置目录".to_string()),
            ));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("支付提交工具");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PaySubmit");
}
