use serde_json::Value;
use tauri::{Manager, State};

use crate::{feature_api_debug, feature_settings};

#[tauri::command]
pub async fn invoke_action(
    action: String,
    params: Value,
    settings: State<'_, feature_settings::SettingsState>,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    // 用户拒绝启动同意书时退出应用；仅用于关闭程序本身，不涉及任何业务数据。
    if action == "app.exit" {
        app.exit(0);
        return Ok(serde_json::json!({ "exiting": true }));
    }

    // 在系统文件管理器中打开本地密钥配置目录，便于用户备份或迁移；仅定位文件夹，不读取内容。
    if action == "app.open_config_dir" {
        return open_config_dir(&app);
    }

    if matches!(
        action.as_str(),
        "settings.get" | "settings.api.catalog" | "settings.api.save" | "settings.api.import"
    ) {
        let settings = settings.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            feature_settings::invoke(&settings, &action, params)
        })
        .await
        .map_err(|_| "本地密钥任务异常结束，请重新加载后确认保存状态".to_string())?;
    }

    if action == "api.catalog" {
        return feature_api_debug::catalog();
    }

    if action == "api.execute" {
        let settings = settings.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            feature_api_debug::execute(params, &settings.store()?)
        })
        .await
        .map_err(|_| "接口提交任务异常结束，请重试".to_string())?;
    }

    Err(format!("未知功能：{action}"))
}

/// 打开本机配置目录；仅定位文件夹，不读取或回传任何配置内容。
fn open_config_dir(app: &tauri::AppHandle) -> Result<Value, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|_| "无法获取应用配置目录，请确认当前Windows用户环境正常".to_string())?;
    std::fs::create_dir_all(&directory).map_err(|error| format!("创建配置目录失败：{error}"))?;
    open_in_file_manager(&directory)?;
    Ok(serde_json::json!({ "path": directory.to_string_lossy() }))
}

#[cfg(target_os = "windows")]
fn open_in_file_manager(directory: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(directory)
        .spawn()
        .map_err(|error| format!("打开配置目录失败：{error}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn open_in_file_manager(_directory: &std::path::Path) -> Result<(), String> {
    Err("打开配置目录仅支持Windows".to_string())
}
