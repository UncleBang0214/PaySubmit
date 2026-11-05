use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde_json::Value;
use zeroize::Zeroize;

use crate::{core_settings::SettingsStore, feature_api_settings};

/// 配置损坏时保持应用可用；用户修复文件后可在界面重试加载。
#[derive(Clone)]
pub(crate) struct SettingsState {
    directory: Result<PathBuf, String>,
    store: Arc<Mutex<Option<SettingsStore>>>,
}

impl SettingsState {
    pub(crate) fn new(directory: Result<PathBuf, String>) -> Self {
        Self {
            directory,
            store: Arc::new(Mutex::new(None)),
        }
    }

    /// 所有调用者均在spawn_blocking中读取，避免磁盘与DPAPI阻塞界面。
    pub(crate) fn store(&self) -> Result<SettingsStore, String> {
        let mut state = self
            .store
            .lock()
            .map_err(|_| "用户配置状态不可用，请重启应用后重试".to_string())?;
        if let Some(store) = state.as_ref() {
            return Ok(store.clone());
        }
        let store = SettingsStore::open(self.directory.clone()?)?;
        *state = Some(store.clone());
        Ok(store)
    }
}

pub(crate) fn invoke(
    state: &SettingsState,
    action: &str,
    mut params: Value,
) -> Result<Value, String> {
    let result = (|| match action {
        "settings.get" => state.store()?.snapshot(),
        "settings.api.catalog" => feature_api_settings::catalog(),
        "settings.api.save" => {
            feature_api_settings::validate_target(&params)?;
            let store = state.store()?;
            let input = std::mem::take(&mut params);
            store.save_api(input)
        }
        "settings.api.import" => {
            feature_api_settings::validate_target(&params)?;
            let store = state.store()?;
            let input = std::mem::take(&mut params);
            store.import_legacy(input)
        }
        _ => Err("不支持的用户配置操作".to_string()),
    })();
    clear_input(&mut params);
    result
}

fn clear_input(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(items) => items.iter_mut().for_each(clear_input),
        Value::Object(fields) => fields.values_mut().for_each(clear_input),
        _ => {}
    }
}
