use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::{
    feature_api_crypto as crypto,
    feature_api_debug::{require_credential, ApiCredentials, ApiDefinition},
};

pub(crate) enum RequestBody {
    Form(Vec<(String, String)>),
    Json(Value),
}

pub(crate) struct PreparedRequest {
    pub(crate) request_body: RequestBody,
    pub(crate) display_body: Value,
    pub(crate) sign_source_display: String,
}

pub(crate) fn prepare_request(
    definition: &ApiDefinition,
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    match definition.protocol.as_str() {
        "form_md5_fields" => prepare_form_md5(definition, payload, credentials),
        "form_sm_fields" => prepare_form_sm_fields(definition, payload, credentials),
        "form_sm_sign" => prepare_form_sm_sign(definition, payload, credentials),
        "form_sm_envelope" => prepare_form_sm_envelope(definition, payload, credentials),
        "json_sm_envelope" => prepare_json_sm(payload, credentials),
        "json_rsa_aes" => prepare_json_rsa(payload, credentials),
        "json_3des_sha256" => prepare_json_3des(definition, payload, credentials),
        "json_aes_sha256" => prepare_json_aes(payload, credentials),
        protocol => Err(format!("接口调试不支持协议：{protocol}")),
    }
}

pub(crate) fn process_response(
    definition: &ApiDefinition,
    response_body: &str,
    credentials: &ApiCredentials,
) -> Value {
    let parsed = match serde_json::from_str::<Value>(response_body) {
        Ok(value) => value,
        Err(_) => {
            return json!({
                "parsed": Value::Null,
                "processed": response_body,
                "verification": {
                    "status": "skipped",
                    "message": "响应不是 JSON，未执行验签或解密"
                }
            });
        }
    };

    let analyzed = match definition.protocol.as_str() {
        "form_md5_fields" => analyze_form_response(definition, &parsed, credentials, false),
        "form_sm_fields" | "form_sm_sign" => {
            analyze_form_response(definition, &parsed, credentials, true)
        }
        "form_sm_envelope" => analyze_form_sm_envelope(&parsed, credentials),
        "json_sm_envelope" => analyze_json_sm(&parsed, credentials),
        "json_rsa_aes" => analyze_json_rsa(&parsed, credentials),
        "json_3des_sha256" => analyze_json_3des(&parsed, credentials),
        "json_aes_sha256" => analyze_json_aes(&parsed, credentials),
        _ => Ok((
            Value::Null,
            verification("skipped", "当前协议没有响应处理规则", None),
        )),
    };

    match analyzed {
        Ok((processed, verification)) => json!({
            "parsed": parsed,
            "processed": processed,
            "verification": verification,
        }),
        Err(error) => json!({
            "parsed": parsed,
            "processed": Value::Null,
            "verification": {
                "status": "error",
                "message": error,
            }
        }),
    }
}

fn prepare_form_md5(
    definition: &ApiDefinition,
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_credential(&credentials.md5_key, "MD5 签名密钥")?;
    let mut params = payload_to_strings(payload)?;
    let has_encrypted_values = definition
        .fields
        .iter()
        .any(|field| field.encrypt && !pair_value(&params, &field.name).is_empty());
    if has_encrypted_values {
        require_credential(&credentials.triple_des_key, "3DES 密钥")?;
    }
    for field in definition.fields.iter().filter(|field| field.encrypt) {
        if let Some((_, value)) = params.iter_mut().find(|(name, _)| name == &field.name) {
            if !value.is_empty() {
                *value = crypto::triple_des_encrypt(&credentials.triple_des_key, value)?;
            }
        }
    }
    let sign_values = definition
        .fields
        .iter()
        .filter(|field| field.sign)
        .map(|field| pair_value(&params, &field.name).to_string())
        .collect::<Vec<_>>();
    let sign_source = format!("&{}&{}", sign_values.join("&"), credentials.md5_key);
    params.push(("sign".to_string(), crypto::md5_hex(sign_source.trim())));

    let product = product_id(definition);
    if matches!(
        product,
        "atupay" | "authentication" | "largepayment" | "quickpay"
    ) {
        params.push(("signatureType".to_string(), "MD5".to_string()));
    } else if product == "merchanttransfer" {
        params.push(("signType".to_string(), "MD5".to_string()));
    }

    Ok(form_request(
        params,
        format!("&{}&***", sign_values.join("&")),
    ))
}

fn prepare_form_sm_fields(
    definition: &ApiDefinition,
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_sm_keys(credentials)?;
    let mut params = payload_to_strings(payload)?;
    let sm4_key = crypto::random_key(16);
    for field in definition.fields.iter().filter(|field| field.encrypt) {
        if let Some((_, value)) = params.iter_mut().find(|(name, _)| name == &field.name) {
            if !value.is_empty() {
                *value = crypto::sm4_encrypt(&sm4_key, value)?;
            }
        }
    }
    let sign_source = ordered_field_sign_source(definition, &params);
    let sign = crypto::sm2_sign(&credentials.merchant_private_key, &sign_source)?;
    let encryption_key = crypto::sm2_encrypt(&credentials.platform_public_key, &sm4_key)?;
    params.push(("encryptionKey".to_string(), encryption_key));
    params.push(("signatureType".to_string(), "SM3WITHSM2".to_string()));
    params.push(("sign".to_string(), sign));
    Ok(form_request(params, sign_source))
}

fn prepare_form_sm_sign(
    definition: &ApiDefinition,
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_credential(&credentials.merchant_private_key, "商户 SM2 私钥")?;
    require_credential(&credentials.platform_public_key, "合利宝 SM2 公钥")?;
    let mut params = payload_to_strings(payload)?;
    let sign_source = ordered_field_sign_source(definition, &params);
    params.push((
        "sign".to_string(),
        crypto::sm2_sign(&credentials.merchant_private_key, &sign_source)?,
    ));
    Ok(form_request(params, sign_source))
}

fn prepare_form_sm_envelope(
    definition: &ApiDefinition,
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_sm_keys(credentials)?;
    let plaintext = serde_json::to_string(payload)
        .map_err(|error| format!("序列化国密请求数据失败：{error}"))?;
    let sm4_key = crypto::random_key(16);
    let body = crypto::sm4_encrypt(&sm4_key, &plaintext)?;
    let sign_source = format!("{}&{}", body, credentials.customer_number);
    let params = vec![
        (
            "interfaceName".to_string(),
            definition.interface_name.clone(),
        ),
        ("body".to_string(), body),
        (
            "merchantNo".to_string(),
            credentials.customer_number.clone(),
        ),
        (
            "sign".to_string(),
            crypto::sm2_sign(&credentials.merchant_private_key, &sign_source)?,
        ),
        ("signType".to_string(), "SM3WITHSM2".to_string()),
        (
            "encryptionKey".to_string(),
            crypto::sm2_encrypt(&credentials.platform_public_key, &sm4_key)?,
        ),
        (
            "orderNo".to_string(),
            payload.get("orderNo").map(value_text).unwrap_or_default(),
        ),
    ];
    Ok(form_request(params, sign_source))
}

fn prepare_json_sm(
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_sm_keys(credentials)?;
    let plaintext = serde_json::to_string(payload)
        .map_err(|error| format!("序列化国密请求数据失败：{error}"))?;
    let sm4_key = crypto::random_key(16);
    let data = crypto::sm4_encrypt(&sm4_key, &plaintext)?;
    let envelope = json!({
        "data": data,
        "customerNumber": credentials.customer_number,
        "encryptionKey": crypto::sm2_encrypt(&credentials.platform_public_key, &sm4_key)?,
        "signType": "SM3WITHSM2",
        "sign": crypto::sm2_sign(&credentials.merchant_private_key, &data)?,
        "timestamp": chrono::Local::now().format("%Y%m%d%H%M%S").to_string(),
        "version": "1.0",
    });
    Ok(json_request(envelope, data))
}

fn prepare_json_rsa(
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_credential(&credentials.merchant_private_key, "商户 RSA 私钥")?;
    require_credential(&credentials.platform_public_key, "合利宝 RSA 公钥")?;
    let plaintext = serde_json::to_string(payload)
        .map_err(|error| format!("序列化 RSA 请求数据失败：{error}"))?;
    let aes_key = crypto::random_key(16);
    let data = crypto::aes_ecb_encrypt(&aes_key, &plaintext, false)?;
    let envelope = json!({
        "data": data,
        "customerNumber": credentials.customer_number,
        "encryptionKey": crypto::rsa_encrypt(&credentials.platform_public_key, &aes_key)?,
        "signType": "MD5WITHRSA",
        "sign": crypto::rsa_md5_sign(&credentials.merchant_private_key, &data)?,
        "timestamp": chrono::Local::now().format("%Y%m%d%H%M%S").to_string(),
        "version": "1.0",
    });
    Ok(json_request(envelope, data))
}

fn prepare_json_3des(
    definition: &ApiDefinition,
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_credential(&credentials.triple_des_key, "3DES 密钥")?;
    require_credential(&credentials.sha256_key, "SHA-256 签名密钥")?;
    if credentials.triple_des_key.as_bytes().len() != 24 {
        return Err("航采通 3DES 密钥必须是 24 字节".to_string());
    }
    let plaintext = serde_json::to_string(payload)
        .map_err(|error| format!("序列化 3DES 请求数据失败：{error}"))?;
    let data = crypto::triple_des_encrypt(&credentials.triple_des_key, &plaintext)?;
    let sign_source = format!(
        "{}&{}&{}",
        credentials.customer_number, data, credentials.sha256_key
    );
    let envelope = json!({
        "data": data,
        "merchantNo": credentials.customer_number,
        "signType": "SHA256",
        "sign": crypto::sha256_hex(&sign_source),
    });
    let display = format!("{}&{}&***", credentials.customer_number, data);
    if definition.content_type == "form" {
        let fields = payload_to_strings(&envelope)?;
        Ok(form_request(fields, display))
    } else {
        Ok(json_request(envelope, display))
    }
}

fn prepare_json_aes(
    payload: &Value,
    credentials: &ApiCredentials,
) -> Result<PreparedRequest, String> {
    require_credential(&credentials.aes_key, "AES 密钥")?;
    require_credential(&credentials.sha256_key, "SHA-256 签名密钥")?;
    let plaintext = serde_json::to_string(payload)
        .map_err(|error| format!("序列化 AES 请求数据失败：{error}"))?;
    let content = crypto::aes_ecb_encrypt(&credentials.aes_key, &plaintext, true)?;
    let sign_source = build_sha_sign_source(&credentials.sha256_key, payload)?;
    let display_source = build_sha_sign_source("***", payload)?;
    let envelope = json!({
        "platMerchantNo": payload.get("platMerchantNo").map(value_text).unwrap_or_default(),
        "merchantNo": payload.get("merchantNo").map(value_text).unwrap_or_default(),
        "orderNo": payload.get("orderNo").map(value_text).unwrap_or_default(),
        "productCode": payload.get("productCode").map(value_text).unwrap_or_default(),
        "certFlag": "",
        "sign": crypto::sha256_hex(&sign_source),
        "content": content,
    });
    Ok(json_request(envelope, display_source))
}

fn analyze_form_response(
    definition: &ApiDefinition,
    parsed: &Value,
    credentials: &ApiCredentials,
    use_sm2: bool,
) -> Result<(Value, Value), String> {
    if definition.response_sign_fields.is_empty() {
        return Ok((
            parsed.clone(),
            verification("skipped", "该业务类型未提供响应签名字段规则", None),
        ));
    }
    let object = parsed
        .as_object()
        .ok_or_else(|| "表单协议响应不是 JSON 对象".to_string())?;
    let signature = object.get("sign").map(value_text).unwrap_or_default();
    if signature.is_empty() {
        return Ok((
            parsed.clone(),
            verification("skipped", "响应未包含 sign 字段", None),
        ));
    }
    let values = definition
        .response_sign_fields
        .iter()
        .map(|field| object.get(field).map(value_text).unwrap_or_default())
        .collect::<Vec<_>>();
    let source = format!("&{}", values.join("&"));
    let (valid, display_source) = if use_sm2 {
        require_credential(&credentials.platform_public_key, "合利宝 SM2 公钥")?;
        (
            crypto::sm2_verify(&credentials.platform_public_key, &source, &signature)?,
            source,
        )
    } else {
        require_credential(&credentials.md5_key, "MD5 签名密钥")?;
        let actual = format!("{}&{}", source, credentials.md5_key);
        (
            crypto::md5_hex(actual.trim()).eq_ignore_ascii_case(&signature),
            format!("{}&***", source),
        )
    };
    Ok((
        parsed.clone(),
        verification(
            if valid { "passed" } else { "failed" },
            if valid {
                "响应验签通过"
            } else {
                "响应验签未通过"
            },
            Some(display_source),
        ),
    ))
}

fn analyze_form_sm_envelope(
    parsed: &Value,
    credentials: &ApiCredentials,
) -> Result<(Value, Value), String> {
    let data = string_field(parsed, "data");
    let sign = string_field(parsed, "sign");
    if data.is_empty() || sign.is_empty() {
        return Ok((
            parsed.clone(),
            verification("skipped", "响应未同时包含 data 和 sign", None),
        ));
    }
    require_sm_keys(credentials)?;
    let valid = crypto::sm2_verify(&credentials.platform_public_key, &data, &sign)?;
    let encryption_key = string_field(parsed, "encryptionKey");
    let processed = if encryption_key.is_empty() {
        parse_json_or_string(&data)
    } else {
        let sm4_key = crypto::sm2_decrypt(&credentials.merchant_private_key, &encryption_key)?;
        parse_json_or_string(&crypto::sm4_decrypt(&sm4_key, &data)?)
    };
    Ok((
        processed,
        verification(
            if valid { "passed" } else { "failed" },
            if valid {
                "响应 SM2 验签通过"
            } else {
                "响应 SM2 验签未通过"
            },
            Some(data),
        ),
    ))
}

fn analyze_json_sm(parsed: &Value, credentials: &ApiCredentials) -> Result<(Value, Value), String> {
    let data = string_field(parsed, "data");
    let sign = string_field(parsed, "sign");
    if data.is_empty() || sign.is_empty() {
        return Ok((
            parsed.clone(),
            verification("skipped", "响应未同时包含 data 和 sign", None),
        ));
    }
    require_credential(&credentials.platform_public_key, "合利宝 SM2 公钥")?;
    let valid = crypto::sm2_verify(&credentials.platform_public_key, &data, &sign)?;
    Ok((
        parse_json_or_string(&data),
        verification(
            if valid { "passed" } else { "failed" },
            if valid {
                "响应 SM2 验签通过"
            } else {
                "响应 SM2 验签未通过"
            },
            Some(data),
        ),
    ))
}

fn analyze_json_rsa(
    parsed: &Value,
    credentials: &ApiCredentials,
) -> Result<(Value, Value), String> {
    let data = string_field(parsed, "data");
    let sign = string_field(parsed, "sign");
    if data.is_empty() || sign.is_empty() {
        return Ok((
            parsed.clone(),
            verification("skipped", "响应未同时包含 data 和 sign", None),
        ));
    }
    require_credential(&credentials.platform_public_key, "合利宝 RSA 公钥")?;
    let valid = crypto::rsa_md5_verify(&credentials.platform_public_key, &data, &sign)?;
    Ok((
        parse_json_or_string(&data),
        verification(
            if valid { "passed" } else { "failed" },
            if valid {
                "响应 RSA 验签通过"
            } else {
                "响应 RSA 验签未通过"
            },
            Some(data),
        ),
    ))
}

fn analyze_json_3des(
    parsed: &Value,
    credentials: &ApiCredentials,
) -> Result<(Value, Value), String> {
    let data = string_field(parsed, "data");
    let sign = string_field(parsed, "sign");
    if data.is_empty() || sign.is_empty() {
        return Ok((
            parsed.clone(),
            verification("skipped", "响应未同时包含 data 和 sign", None),
        ));
    }
    require_credential(&credentials.sha256_key, "SHA-256 签名密钥")?;
    require_credential(&credentials.triple_des_key, "3DES 密钥")?;
    let source = format!(
        "{}&{}&{}",
        credentials.customer_number, data, credentials.sha256_key
    );
    let valid = crypto::sha256_hex(&source).eq_ignore_ascii_case(&sign);
    let plaintext = crypto::triple_des_decrypt(&credentials.triple_des_key, &data)?;
    Ok((
        parse_json_or_string(&plaintext),
        verification(
            if valid { "passed" } else { "failed" },
            if valid {
                "响应 SHA-256 验签通过"
            } else {
                "响应 SHA-256 验签未通过"
            },
            Some(format!("{}&{}&***", credentials.customer_number, data)),
        ),
    ))
}

fn analyze_json_aes(
    parsed: &Value,
    credentials: &ApiCredentials,
) -> Result<(Value, Value), String> {
    let content = string_field(parsed, "content");
    let sign = string_field(parsed, "sign");
    if content.is_empty() {
        return Ok((
            parsed.clone(),
            verification("skipped", "响应未包含 content 字段", None),
        ));
    }
    require_credential(&credentials.aes_key, "AES 密钥")?;
    require_credential(&credentials.sha256_key, "SHA-256 签名密钥")?;
    let plaintext = crypto::aes_ecb_decrypt(&credentials.aes_key, &content, true)?;
    let processed = parse_json_or_string(&plaintext);
    if sign.is_empty() {
        return Ok((
            processed,
            verification("skipped", "响应未包含 sign 字段，已完成 AES 解密", None),
        ));
    }
    let mut sign_data = processed.clone();
    if let Some(object) = sign_data.as_object_mut() {
        let keys = object.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            if object.get(&key).is_some_and(Value::is_object) {
                let nested_source = build_sha_sign_source(&credentials.sha256_key, &object[&key])?;
                object.insert(key, Value::String(crypto::sha256_hex(&nested_source)));
            }
        }
    }
    let source = build_sha_sign_source(&credentials.sha256_key, &sign_data)?;
    let display = build_sha_sign_source("***", &sign_data)?;
    let valid = crypto::sha256_hex(&source).eq_ignore_ascii_case(&sign);
    Ok((
        processed,
        verification(
            if valid { "passed" } else { "failed" },
            if valid {
                "响应 SHA-256 验签通过"
            } else {
                "响应 SHA-256 验签未通过"
            },
            Some(display),
        ),
    ))
}

fn require_sm_keys(credentials: &ApiCredentials) -> Result<(), String> {
    require_credential(&credentials.merchant_private_key, "商户 SM2 私钥")?;
    require_credential(&credentials.platform_public_key, "合利宝 SM2 公钥")
}

fn payload_to_strings(payload: &Value) -> Result<Vec<(String, String)>, String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "接口请求参数必须是 JSON 对象".to_string())?;
    Ok(object
        .iter()
        .map(|(name, value)| (name.clone(), value_text(value)))
        .collect())
}

fn ordered_field_sign_source(definition: &ApiDefinition, params: &[(String, String)]) -> String {
    let values = definition
        .fields
        .iter()
        .filter(|field| field.sign)
        .map(|field| pair_value(params, &field.name))
        .collect::<Vec<_>>();
    format!("&{}", values.join("&"))
}

fn pair_value<'a>(params: &'a [(String, String)], name: &str) -> &'a str {
    params
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
        .unwrap_or_default()
}

fn form_request(params: Vec<(String, String)>, sign_source_display: String) -> PreparedRequest {
    let display_body = Value::Object(
        params
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    );
    PreparedRequest {
        request_body: RequestBody::Form(params),
        display_body,
        sign_source_display,
    }
}

fn json_request(body: Value, sign_source_display: String) -> PreparedRequest {
    PreparedRequest {
        request_body: RequestBody::Json(body.clone()),
        display_body: body,
        sign_source_display,
    }
}

fn product_id(definition: &ApiDefinition) -> &str {
    definition.id.split('.').next().unwrap_or_default()
}

fn string_field(value: &Value, name: &str) -> String {
    value.get(name).map(value_text).unwrap_or_default()
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        value => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn parse_json_or_string(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
}

fn verification(status: &str, message: &str, source: Option<String>) -> Value {
    json!({
        "status": status,
        "message": message,
        "sign_source": source,
    })
}

fn build_sha_sign_source(secret: &str, value: &Value) -> Result<String, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "SHA-256 签名参数必须是 JSON 对象".to_string())?;
    let sorted = object.iter().collect::<BTreeMap<_, _>>();
    let mut source = secret.to_string();
    for (key, value) in sorted {
        let Some(value) = canonical_sha_value(secret, value)? else {
            continue;
        };
        source.push(',');
        source.push_str(key);
        source.push('=');
        source.push_str(&value);
    }
    source.push(',');
    source.push_str(secret);
    Ok(source)
}

fn canonical_sha_value(secret: &str, value: &Value) -> Result<Option<String>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(value) if value.trim().is_empty() => Ok(None),
        Value::String(value) => {
            let trimmed = value.trim();
            if (trimmed.starts_with('[') || trimmed.starts_with('{'))
                && serde_json::from_str::<Value>(trimmed).is_ok()
            {
                return canonical_sha_value(secret, &serde_json::from_str(trimmed).unwrap());
            }
            Ok(Some(value.clone()))
        }
        Value::Object(_) => Ok(Some(build_sha_sign_source(secret, value)?)),
        Value::Array(values) => Ok(Some(canonical_sha_array(values)?)),
        value => Ok(Some(value_text(value))),
    }
}

fn canonical_sha_array(values: &[Value]) -> Result<String, String> {
    let mut values = values
        .iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| "SHA-256 数组参数必须由 JSON 对象组成".to_string())?;
            let index = object
                .get("index")
                .map(value_text)
                .ok_or_else(|| "SHA-256 数组明细缺少 index".to_string())?
                .parse::<i64>()
                .map_err(|_| "SHA-256 数组明细 index 不是整数".to_string())?;
            Ok((index, canonical_json_object(object)?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    values.sort_by_key(|(index, _)| *index);
    Ok(format!(
        "[{}]",
        values
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn canonical_json_object(object: &Map<String, Value>) -> Result<String, String> {
    let sorted = object.iter().collect::<BTreeMap<_, _>>();
    let mut entries = Vec::new();
    for (key, value) in sorted {
        if value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty()) {
            continue;
        }
        let key = serde_json::to_string(key)
            .map_err(|error| format!("序列化 SHA-256 字段名失败：{error}"))?;
        let value = serde_json::to_string(value)
            .map_err(|error| format!("序列化 SHA-256 字段值失败：{error}"))?;
        entries.push(format!("{key}:{value}"));
    }
    Ok(format!("{{{}}}", entries.join(",")))
}

