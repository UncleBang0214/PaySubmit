//! 商户入驻-变更结算信息（modifyMerchantInfoV2）。
//! 采用国密：随机 SM4 加密请求体，SM2 签名，SM2 加密 SM4 密钥，multipart 上传。
//! 平台商商户编号与 SM2 私钥/公钥来自本地「设置」。
use std::time::Duration;

use reqwest::blocking::{multipart, ClientBuilder};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use zeroize::Zeroizing;

use crate::core_settings::SettingsStore;
use crate::feature_api_crypto as crypto;

const INTERFACE_NAME: &str = "modifyMerchantInfoV2";
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
const MAX_TIMEOUT_SECONDS: u64 = 300;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
struct ModifySettleParams {
    merchant_no: String,
    account_name: String,
    account_no: String,
    settle_bank_type: String,
    bank_code: String,
    merchant_entry_alteration_type: String,
    #[serde(default)]
    update_account_name: String,
    #[serde(default)]
    update_account_no: String,
    #[serde(default)]
    update_settle_bank_type: String,
    #[serde(default)]
    update_bank_code: String,
    #[serde(default)]
    settle_mode: String,
    #[serde(default)]
    settlement_mode: String,
    #[serde(default)]
    settlement_period: String,
    #[serde(default = "default_timeout_seconds")]
    timeout_seconds: u64,
}

fn default_timeout_seconds() -> u64 {
    DEFAULT_TIMEOUT_SECONDS
}

fn text(value: &Option<Zeroizing<String>>) -> &str {
    value.as_ref().map(|item| item.as_str()).unwrap_or("")
}

pub(crate) fn modify_settle_info(params: Value, settings: &SettingsStore) -> Result<Value, String> {
    let params: ModifySettleParams =
        serde_json::from_value(params).map_err(|error| format!("变更结算信息参数无效：{error}"))?;
    let merchant_no = params.merchant_no.trim();
    if merchant_no.is_empty() {
        return Err("请填写子商户编号".to_string());
    }
    for (value, label) in [
        (params.account_name.trim(), "结算账户名"),
        (params.account_no.trim(), "结算账号"),
        (params.settle_bank_type.trim(), "账户类型"),
        (params.bank_code.trim(), "银行编码"),
        (params.merchant_entry_alteration_type.trim(), "变更类型"),
    ] {
        if value.is_empty() {
            return Err(format!("请填写{label}"));
        }
    }
    if !(1..=MAX_TIMEOUT_SECONDS).contains(&params.timeout_seconds) {
        return Err(format!("请求超时必须在 1 到 {MAX_TIMEOUT_SECONDS} 秒之间"));
    }

    // 平台商商户编号 + SM2 私钥/公钥均取自「设置」（combo=sm, scope=global）。
    let credentials = settings.effective_api_credentials("PublicProduct", "domestic", "sm")?;
    let parent_no = text(&credentials.customer_number).trim().to_string();
    let sm2_private = text(&credentials.sm2_private_key).trim().to_string();
    let sm2_public = text(&credentials.sm2_public_key).trim().to_string();
    if parent_no.is_empty() {
        return Err("请先在「设置」中配置平台商商户编号".to_string());
    }
    if sm2_private.is_empty() || sm2_public.is_empty() {
        return Err("请先在「设置」中配置商户 SM2 私钥与平台 SM2 公钥".to_string());
    }

    let order_no = format!("ORDER{}", chrono::Local::now().format("%Y%m%d%H%M%S"));
    let request_data = build_request_data(&params, merchant_no, &order_no)?;

    let sm4_key = crypto::random_key(16);
    let body = crypto::sm4_encrypt(&sm4_key, &request_data)?;
    let sign_source = format!("{body}&{parent_no}");
    let sign = crypto::sm2_sign(&sm2_private, &sign_source)?;
    let encryption_key = crypto::sm2_encrypt(&sm2_public, &sm4_key)?;

    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(params.timeout_seconds))
        .user_agent("PaySubmit/0.1")
        .build()
        .map_err(|error| format!("创建变更结算信息 HTTP 客户端失败：{error}"))?;
    let form = multipart::Form::new()
        .text("interfaceName", INTERFACE_NAME)
        .text("body", body)
        .text("merchantNo", parent_no)
        .text("sign", sign)
        .text("signType", "SM3WITHSM2")
        .text("encryptionKey", encryption_key);
    let response = client
        .post(crate::core_config::API_MERCHANT_ENTRY_UPLOAD_ENDPOINT)
        .multipart(form)
        .send()
        .map_err(|error| format!("变更结算信息请求失败：{error}"))?;
    let status = response.status();
    let mut bytes = Vec::new();
    use std::io::Read;
    response
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取变更结算信息响应失败：{error}"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("变更结算信息响应过大".to_string());
    }
    let response_text = decode_text(&bytes);

    parse_response(&response_text, status.as_u16(), &sm2_private, &sm2_public)
}

fn build_request_data(
    params: &ModifySettleParams,
    merchant_no: &str,
    order_no: &str,
) -> Result<String, String> {
    let mut root = Map::new();
    root.insert("merchantNo".to_string(), json!(merchant_no));
    root.insert("orderNo".to_string(), json!(order_no));
    // 与示例 DTO 的布尔字段默认值保持一致。
    root.insert("updateChannelIndustryTypeCode".to_string(), json!(false));
    root.insert("synChannel".to_string(), json!(false));
    root.insert("enterpriseOwner".to_string(), json!(false));
    root.insert("updatePosMchName".to_string(), json!(false));

    let mut settle = Map::new();
    settle.insert("accountName".to_string(), json!(params.account_name.trim()));
    settle.insert("accountNo".to_string(), json!(params.account_no.trim()));
    settle.insert(
        "settleBankType".to_string(),
        json!(params.settle_bank_type.trim()),
    );
    settle.insert("bankCode".to_string(), json!(params.bank_code.trim()));
    settle.insert(
        "merchantEntryAlterationType".to_string(),
        json!(params.merchant_entry_alteration_type.trim()),
    );
    for (key, value) in [
        ("updateAccountName", params.update_account_name.trim()),
        ("updateAccountNo", params.update_account_no.trim()),
        (
            "updateSettleBankType",
            params.update_settle_bank_type.trim(),
        ),
        ("updateBankCode", params.update_bank_code.trim()),
        ("settleMode", params.settle_mode.trim()),
        ("settlementMode", params.settlement_mode.trim()),
        ("settlementPeriod", params.settlement_period.trim()),
    ] {
        if !value.is_empty() {
            settle.insert(key.to_string(), json!(value));
        }
    }
    root.insert("updateSettleInfo".to_string(), Value::Object(settle));

    serde_json::to_string(&Value::Object(root))
        .map_err(|error| format!("变更结算信息报文序列化失败：{error}"))
}

fn parse_response(
    text: &str,
    http_status: u16,
    sm2_private: &str,
    sm2_public: &str,
) -> Result<Value, String> {
    let parsed: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => {
            return Ok(json!({
                "ok": false,
                "code": "",
                "message": plain_message(text),
                "result": "",
                "raw": text,
                "http_status": http_status,
            }));
        }
    };
    let code = parsed.get("code").and_then(Value::as_str).unwrap_or("");
    let message = parsed.get("message").and_then(Value::as_str).unwrap_or("");
    let data = parsed.get("data").and_then(Value::as_str).unwrap_or("");
    let sign = parsed.get("sign").and_then(Value::as_str).unwrap_or("");
    let response_key = parsed
        .get("encryptionKey")
        .and_then(Value::as_str)
        .unwrap_or("");

    if data.is_empty() || sign.is_empty() {
        return Ok(json!({
            "ok": code == "0000",
            "code": code,
            "message": plain_message(if message.is_empty() { text } else { message }),
            "result": "",
            "raw": text,
            "http_status": http_status,
        }));
    }

    let verified = crypto::sm2_verify(sm2_public, data, sign).unwrap_or(false);
    if !verified {
        return Ok(json!({
            "ok": false,
            "code": code,
            "message": "服务端响应验签未通过",
            "result": "",
            "raw": text,
            "http_status": http_status,
        }));
    }

    let mut result_text = String::new();
    if !response_key.is_empty() {
        let sm4_key = crypto::sm2_decrypt(sm2_private, response_key)?;
        let plain = crypto::sm4_decrypt(&sm4_key, data)?;
        result_text = extract_result(&plain);
    }

    Ok(json!({
        "ok": code == "0000",
        "code": code,
        "message": plain_message(message),
        "result": result_text,
        "raw": text,
        "http_status": http_status,
    }))
}

fn extract_result(plain: &str) -> String {
    match serde_json::from_str::<Value>(plain) {
        Ok(value) => value
            .get("result")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| plain.to_string()),
        Err(_) => plain.to_string(),
    }
}

fn decode_text(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => text,
        Err(_) => {
            let (text, _, had_errors) = encoding_rs::GB18030.decode(bytes);
            if had_errors {
                String::from_utf8_lossy(bytes).to_string()
            } else {
                text.into_owned()
            }
        }
    }
}

fn plain_message(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "服务端未返回说明".to_string();
    }
    if trimmed.chars().count() > 300 {
        trimmed.chars().take(300).collect()
    } else {
        trimmed.to_string()
    }
}
