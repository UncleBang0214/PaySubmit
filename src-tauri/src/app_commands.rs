use serde_json::Value;
use tauri::State;

use crate::{feature_api_debug, feature_settings};

#[tauri::command]
pub async fn invoke_action(
    action: String,
    params: Value,
    settings: State<'_, feature_settings::SettingsState>,
) -> Result<Value, String> {
    if matches!(
        action.as_str(),
        "settings.get"
            | "settings.api.catalog"
            | "settings.api.save"
            | "settings.api.import"
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
