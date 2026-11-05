#![allow(dead_code)]
// 编译期默认凭据全部为空：PaySubmit不内置任何商户编号或密钥，
// 所有凭据只能由用户在本机界面填写并加密保存。此文件不含任何真实密钥。

pub(crate) const API_CUSTOMER_NUMBER: &str = "";
pub(crate) const API_MD5_KEY: &str = "";
pub(crate) const API_SHA256_KEY: &str = "";
pub(crate) const API_TRIPLE_DES_KEY: &str = "";
pub(crate) const API_AES_KEY: &str = "";
pub(crate) const API_MERCHANT_PRIVATE_KEY: &str = "";
pub(crate) const API_PLATFORM_PUBLIC_KEY: &str = "";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ApiCredentialConfig {
    pub(crate) customer_number: &'static str,
    pub(crate) md5_key: &'static str,
    pub(crate) sha256_key: &'static str,
    pub(crate) triple_des_key: &'static str,
    pub(crate) aes_key: &'static str,
    pub(crate) merchant_private_key: &'static str,
    pub(crate) platform_public_key: &'static str,
}

impl ApiCredentialConfig {
    pub(crate) const EMPTY: Self = Self {
        customer_number: "",
        md5_key: "",
        sha256_key: "",
        triple_des_key: "",
        aes_key: "",
        merchant_private_key: "",
        platform_public_key: "",
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ApiProductCredentialConfig {
    pub(crate) product_id: &'static str,
    pub(crate) credentials: ApiCredentialConfig,
}

// 开源项目不内置任何产品级默认凭据。仅当用户后续需要区分产品默认值时，再于此处显式追加。
pub(crate) const API_PRODUCT_CREDENTIALS: &[ApiProductCredentialConfig] = &[];

// 内置业务接口地址（按需求在代码 config 中固定，不经界面或接口目录读取）。
// 注意：以下为实际对接渠道地址，对外开源发布前请自行评估是否脱敏。
pub(crate) const API_BALANCE_ENDPOINT: &str =
    "https://pay.trx.helipay.com/trx/merchant/interface.action";
pub(crate) const API_SETTLEMENT_ENDPOINT: &str =
    "https://transfer.trx.helipay.com/trx/transfer/interface.action";

/// 按内置接口 ID 返回固定请求地址；未知接口返回 None，回退到接口目录中的地址。
pub(crate) fn builtin_endpoint(api_id: &str) -> Option<&'static str> {
    match api_id {
        "public.md5.balance_query" => Some(API_BALANCE_ENDPOINT),
        "public.md5.settlement_apply" => Some(API_SETTLEMENT_ENDPOINT),
        _ => None,
    }
}

pub(crate) fn api_credentials_for_product(product_id: &str) -> ApiCredentialConfig {
    let product = API_PRODUCT_CREDENTIALS
        .iter()
        .find(|entry| entry.product_id.eq_ignore_ascii_case(product_id))
        .map(|entry| entry.credentials)
        .unwrap_or(ApiCredentialConfig::EMPTY);

    ApiCredentialConfig {
        customer_number: configured_value(product.customer_number, API_CUSTOMER_NUMBER),
        md5_key: configured_value(product.md5_key, API_MD5_KEY),
        sha256_key: configured_value(product.sha256_key, API_SHA256_KEY),
        triple_des_key: configured_value(product.triple_des_key, API_TRIPLE_DES_KEY),
        aes_key: configured_value(product.aes_key, API_AES_KEY),
        merchant_private_key: configured_value(
            product.merchant_private_key,
            API_MERCHANT_PRIVATE_KEY,
        ),
        platform_public_key: configured_value(product.platform_public_key, API_PLATFORM_PUBLIC_KEY),
    }
}

fn configured_value(product_value: &'static str, default_value: &'static str) -> &'static str {
    if product_value.trim().is_empty() {
        default_value
    } else {
        product_value
    }
}
