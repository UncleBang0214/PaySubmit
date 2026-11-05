use std::{
    collections::HashSet,
    io::Read,
    sync::OnceLock,
    time::{Duration, Instant},
};

use chrono::{Duration as ChronoDuration, Local};
use reqwest::{blocking::ClientBuilder, header::CONTENT_TYPE, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    core_config::ApiCredentialConfig,
    core_settings::{SettingsStore, UserApiCredentials},
    feature_api_protocol::{prepare_request, process_response, RequestBody},
};

const API_CATALOG_JSON: &str = include_str!("../../resources/api_catalog.json");
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
const MAX_TIMEOUT_SECONDS: u64 = 300;

static CATALOG: OnceLock<Result<ApiCatalog, String>> = OnceLock::new();

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ApiCatalog {
    pub(crate) products: Vec<ApiProduct>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ApiProduct {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) interfaces: Vec<ApiDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ApiDefinition {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) function: String,
    pub(crate) request_type: String,
    pub(crate) variant: String,
    pub(crate) protocol: String,
    pub(crate) endpoint: String,
    pub(crate) content_type: String,
    #[serde(default)]
    pub(crate) interface_name: String,
    #[serde(default)]
    pub(crate) response_sign_fields: Vec<String>,
    pub(crate) fields: Vec<ApiFieldDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ApiFieldDefinition {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) default: String,
    pub(crate) sign: bool,
    pub(crate) encrypt: bool,
    pub(crate) omit_empty: bool,
}

// 不实现Debug或Serialize，避免任何用户密钥出现在诊断或IPC返回中。
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct ApiCredentials {
    pub(crate) customer_number: String,
    pub(crate) md5_key: String,
    pub(crate) sha256_key: String,
    pub(crate) triple_des_key: String,
    pub(crate) aes_key: String,
    pub(crate) merchant_private_key: String,
    pub(crate) platform_public_key: String,
}

#[derive(Deserialize)]
struct ExecuteParams {
    api_id: String,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    payload: Map<String, Value>,
    /// 请求期填写的商户编号（面向 C 端按次输入，不写入本地密钥）。
    #[serde(default)]
    customer_number: String,
    #[serde(default = "default_timeout_seconds")]
    timeout_seconds: u64,
}

pub(crate) fn catalog() -> Result<Value, String> {
    serde_json::to_value(load_catalog()?).map_err(|error| format!("接口目录序列化失败：{error}"))
}

#[cfg(test)]
#[allow(dead_code)]
fn validate_settings_scope(scope: &str) -> Result<(), String> {
    if scope == "global"
        || load_catalog()?
            .products
            .iter()
            .any(|product| product.id == scope)
    {
        Ok(())
    } else {
        Err("用户配置作用域不存在，请刷新产品列表后重试".to_string())
    }
}

pub(crate) fn execute(params: Value, settings: &SettingsStore) -> Result<Value, String> {
    let params: ExecuteParams =
        serde_json::from_value(params).map_err(|error| format!("接口调试参数格式无效：{error}"))?;
    let (product_id, definition) = load_catalog()?
        .products
        .iter()
        .find_map(|product| {
            product
                .interfaces
                .iter()
                .find(|definition| definition.id == params.api_id)
                .map(|definition| (product.id.as_str(), definition))
        })
        .ok_or_else(|| format!("接口目录中不存在调试入口：{}", params.api_id))?;
    let credentials = credentials_for_definition(product_id, definition, settings)?;

    let endpoint: &str = if !params.endpoint.trim().is_empty() {
        params.endpoint.trim()
    } else if let Some(fixed) = crate::core_config::builtin_endpoint(&definition.id) {
        // 内置业务接口地址来自代码 config，界面不再暴露地址编辑。
        fixed
    } else {
        definition.endpoint.trim()
    };
    let endpoint_url =
        Url::parse(endpoint).map_err(|error| format!("接口地址格式无效：{error}"))?;
    if !matches!(endpoint_url.scheme(), "http" | "https") {
        return Err("接口地址只支持 HTTP 或 HTTPS".to_string());
    }
    if endpoint_url.host_str().is_none() {
        return Err("接口地址缺少主机名".to_string());
    }
    if !(1..=MAX_TIMEOUT_SECONDS).contains(&params.timeout_seconds) {
        return Err(format!("请求超时必须在 1 到 {MAX_TIMEOUT_SECONDS} 秒之间"));
    }

    let customer_number = if !params.customer_number.trim().is_empty() {
        params.customer_number.trim().to_owned()
    } else {
        credentials.customer_number.clone()
    };
    if customer_number.trim().is_empty() {
        return Err("请填写商户编号".to_string());
    }
    let payload = build_payload(definition, &params.payload, &customer_number)?;
    let prepared = prepare_request(definition, &payload, &credentials)?;
    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(params.timeout_seconds))
        .user_agent("PaySubmit/0.1")
        .build()
        .map_err(|error| format!("创建接口调试 HTTP 客户端失败：{error}"))?;

    let started = Instant::now();
    let request = client.post(endpoint_url);
    let response = match &prepared.request_body {
        RequestBody::Form(fields) => request.form(fields).send(),
        RequestBody::Json(body) => request.json(body).send(),
    }
    .map_err(|error| format!("接口请求失败：{error}"))?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(format!(
            "接口响应超过 {} MB 的调试上限",
            MAX_RESPONSE_BYTES / 1024 / 1024
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取接口响应失败：{error}"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(format!(
            "接口响应超过 {} MB 的调试上限",
            MAX_RESPONSE_BYTES / 1024 / 1024
        ));
    }

    let (response_body, binary) = match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        Err(error) => {
            let bytes = error.into_bytes();
            (
                format!(
                    "BASE64:{}",
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
                ),
                true,
            )
        }
    };
    let analysis = if binary {
        json!({
            "parsed": Value::Null,
            "processed": Value::Null,
            "verification": { "status": "skipped", "message": "二进制响应不执行验签或解密" }
        })
    } else {
        process_response(definition, &response_body, &credentials)
    };

    Ok(json!({
        "api_id": definition.id,
        "duration_ms": started.elapsed().as_millis(),
        "request": {
            "method": "POST",
            "endpoint": endpoint,
            "content_type": definition.content_type,
            "payload": payload,
            "envelope": prepared.display_body,
            "sign_source": prepared.sign_source_display,
        },
        "response": {
            "status": status.as_u16(),
            "success": status.is_success(),
            "content_type": content_type,
            "binary": binary,
            "body": response_body,
            "parsed": analysis.get("parsed").cloned().unwrap_or(Value::Null),
            "processed": analysis.get("processed").cloned().unwrap_or(Value::Null),
            "verification": analysis.get("verification").cloned().unwrap_or(Value::Null),
        }
    }))
}

pub(crate) fn require_credential(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("接口调试缺少{label}"))
    } else {
        Ok(())
    }
}

fn credentials_for_definition(
    product_id: &str,
    definition: &ApiDefinition,
    settings: &SettingsStore,
) -> Result<ApiCredentials, String> {
    let route = crate::feature_api_settings::credential_route(product_id, definition)?;
    // 无法安全判断旧全局/编译期凭据的业务归属，不再自动跨域回退。
    Ok(resolve_credentials(
        ApiCredentialConfig::default(),
        settings.effective_api_credentials(product_id, route.domain, route.combo)?,
        &definition.protocol,
    ))
}

impl Drop for ApiCredentials {
    fn drop(&mut self) {
        self.customer_number.zeroize();
        self.md5_key.zeroize();
        self.sha256_key.zeroize();
        self.triple_des_key.zeroize();
        self.aes_key.zeroize();
        self.merchant_private_key.zeroize();
        self.platform_public_key.zeroize();
    }
}

/// 存储层已选择唯一活动来源：非国密仅当前产品，国密仅当前业务域共享；执行时不回退旧凭据。
fn resolve_credentials(
    defaults: ApiCredentialConfig,
    user: UserApiCredentials,
    protocol: &str,
) -> ApiCredentials {
    fn apply(target: &mut String, value: Option<Zeroizing<String>>) {
        if let Some(value) = value {
            target.zeroize();
            *target = value.as_str().to_owned();
        }
    }
    let mut resolved = ApiCredentials::from(defaults);
    apply(&mut resolved.customer_number, user.customer_number);
    apply(&mut resolved.md5_key, user.md5_key);
    apply(&mut resolved.sha256_key, user.sha256_key);
    apply(&mut resolved.triple_des_key, user.triple_des_key);
    apply(&mut resolved.aes_key, user.aes_key);
    // 两种非对称密钥必须独立选择，不能误用另一协议的用户密钥。
    if protocol == "json_rsa_aes" {
        apply(&mut resolved.merchant_private_key, user.rsa_private_key);
        apply(&mut resolved.platform_public_key, user.rsa_public_key);
    } else if protocol.contains("_sm_") || protocol == "form_sm_sign" {
        apply(&mut resolved.merchant_private_key, user.sm2_private_key);
        apply(&mut resolved.platform_public_key, user.sm2_public_key);
    }
    resolved
}

impl From<ApiCredentialConfig> for ApiCredentials {
    fn from(config: ApiCredentialConfig) -> Self {
        Self {
            customer_number: config.customer_number.to_string(),
            md5_key: config.md5_key.to_string(),
            sha256_key: config.sha256_key.to_string(),
            triple_des_key: config.triple_des_key.to_string(),
            aes_key: config.aes_key.to_string(),
            merchant_private_key: config.merchant_private_key.to_string(),
            platform_public_key: config.platform_public_key.to_string(),
        }
    }
}

pub(crate) fn load_catalog() -> Result<&'static ApiCatalog, String> {
    match CATALOG.get_or_init(|| {
        serde_json::from_str(API_CATALOG_JSON)
            .map_err(|error| format!("内置接口目录格式无效：{error}"))
    }) {
        Ok(catalog) => Ok(catalog),
        Err(error) => Err(error.clone()),
    }
}

fn build_payload(
    definition: &ApiDefinition,
    supplied: &Map<String, Value>,
    customer_number: &str,
) -> Result<Value, String> {
    let mut payload = Map::new();
    let known_fields = definition
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<HashSet<_>>();
    let json_payload =
        definition.protocol.starts_with("json_") || definition.protocol == "form_sm_envelope";

    for field in &definition.fields {
        let value = supplied
            .get(&field.name)
            .cloned()
            .unwrap_or_else(|| Value::String(field.default.clone()));
        let value = normalize_field_value(field, value, customer_number, json_payload)?;
        if field.omit_empty && value.as_str().is_some_and(|value| value.is_empty()) {
            continue;
        }
        payload.insert(field.name.clone(), value);
    }
    for (name, value) in supplied {
        if !known_fields.contains(name.as_str()) {
            payload.insert(name.clone(), value.clone());
        }
    }
    Ok(Value::Object(payload))
}

fn normalize_field_value(
    field: &ApiFieldDefinition,
    value: Value,
    customer_number: &str,
    json_payload: bool,
) -> Result<Value, String> {
    let Value::String(value) = value else {
        return Ok(value);
    };
    let value = resolve_templates(&value, customer_number);
    if json_payload && field.kind == "json" && !value.trim().is_empty() {
        return serde_json::from_str(&value)
            .map_err(|error| format!("字段 {} 不是有效的 JSON：{error}", field.name));
    }
    if json_payload && field.kind == "number" && !value.trim().is_empty() {
        let number = value
            .parse::<Number>()
            .map_err(|_| format!("字段 {} 不是有效数字", field.name))?;
        return Ok(Value::Number(number));
    }
    Ok(Value::String(value))
}

fn resolve_templates(value: &str, customer_number: &str) -> String {
    let now = Local::now();
    value
        .replace("{{customer_number}}", customer_number)
        .replace(
            "{{timestamp_millis}}",
            &now.format("%Y-%m-%d_%H:%M:%S%.3f").to_string(),
        )
        .replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M:%S").to_string())
        .replace("{{timestamp}}", &now.format("%Y%m%d%H%M%S").to_string())
        .replace(
            "{{yesterday}}",
            &(now - ChronoDuration::days(1)).format("%Y%m%d").to_string(),
        )
        .replace(
            "{{tomorrow}}",
            &(now + ChronoDuration::days(1)).format("%Y%m%d").to_string(),
        )
        .replace("{{date}}", &now.format("%Y%m%d").to_string())
}

const fn default_timeout_seconds() -> u64 {
    DEFAULT_TIMEOUT_SECONDS
}
