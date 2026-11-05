//! 接口配置的业务归属来自内置入口，不受用户修改请求地址或界面筛选影响。
use serde_json::{json, Value};

use crate::feature_api_debug::{load_catalog, ApiDefinition};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CredentialRoute {
    pub(crate) domain: &'static str,
    pub(crate) combo: &'static str,
}

/// 一个产品可以同时拥有国内、跨境入口；必须逐接口判定，不能只看产品ID。
pub(crate) fn credential_route(
    product_id: &str,
    definition: &ApiDefinition,
) -> Result<CredentialRoute, String> {
    let domain = match product_id {
        "CrossBorder" => "cross_border",
        "LargePayment"
            if definition.id.starts_with("largepayment.")
                && definition
                    .id
                    .rsplit('.')
                    .next()
                    .is_some_and(|entry| entry.starts_with("cross_border_large_payment_")) =>
        {
            "cross_border"
        }
        "ATUPay"
        | "QuickPay"
        | "LargePayment"
        | "MerchantAccountQuery"
        | "MerchantSettlement"
        | "PublicProduct"
        | "Transfer"
        | "AccountPay"
        | "DelayedSplit"
        | "MerchantTransfer"
        | "DirectPay"
        | "Authentication"
        | "UnionCashier"
        | "UniversalCashier"
        | "TravelAgentPay"
        | "B2BAutoTicket"
        | "NDC"
        | "NDC_CZ"
        | "UPToken" => "domestic",
        _ => return Err("接口尚未定义配置业务域，请更新接口目录的配置映射".to_string()),
    };
    let combo = match definition.protocol.as_str() {
        "form_md5_fields" if domain == "domestic" => "des_md5",
        "form_sm_fields" | "form_sm_sign" | "form_sm_envelope" | "json_sm_envelope" => "sm",
        "json_rsa_aes" if domain == "domestic" => "rsa_aes",
        "json_aes_sha256" if domain == "cross_border" => "aes_sha256",
        "json_3des_sha256" if domain == "domestic" => "des_sha256",
        _ => return Err("接口协议与业务域尚未建立配置映射，已阻止混用密钥".to_string()),
    };
    Ok(CredentialRoute { domain, combo })
}

fn combo_fields(combo: &str) -> &'static [&'static str] {
    match combo {
        "des_md5" => &["triple_des_key", "md5_key"],
        "sm" => &["sm2_private_key", "sm2_public_key"],
        "rsa_aes" => &["rsa_private_key", "rsa_public_key"],
        "aes_sha256" => &["aes_key", "sha256_key"],
        "des_sha256" => &["triple_des_key", "sha256_key"],
        _ => &[],
    }
}

pub(crate) fn catalog() -> Result<Value, String> {
    let catalog = load_catalog()?;
    let mut domains = Vec::new();
    for (domain, name, combos) in [
        (
            "domestic",
            "国内线上",
            &[
                ("des_md5", "3DES+MD5"),
                ("sm", "国密"),
                ("rsa_aes", "AES+RSA"),
                ("des_sha256", "3DES+SHA256"),
            ][..],
        ),
        (
            "cross_border",
            "跨境",
            &[("sm", "国密"), ("aes_sha256", "AES+SHA256")][..],
        ),
    ] {
        let mut entries = Vec::new();
        for &(combo, label) in combos {
            let mut products = Vec::new();
            for product in &catalog.products {
                let mut applicable = false;
                for definition in &product.interfaces {
                    if credential_route(&product.id, definition)?
                        == (CredentialRoute { domain, combo })
                    {
                        applicable = true;
                    }
                }
                if applicable {
                    products.push(json!({ "id": product.id, "name": product.name }));
                }
            }
            entries.push(json!({
                "id": combo,
                "name": label,
                "fields": combo_fields(combo),
                "scope": if combo == "sm" { "domain" } else { "product" },
                "products": products,
            }));
        }
        domains.push(json!({ "id": domain, "name": name, "combos": entries }));
    }
    Ok(json!({ "domains": domains }))
}

/// UI传入的目标必须与执行路径的同一映射一致，不能给不适用的产品写入配置。
pub(crate) fn validate_target(params: &Value) -> Result<(), String> {
    let object = params
        .as_object()
        .ok_or_else(|| "接口配置参数必须是对象".to_string())?;
    let domain = object
        .get("domain")
        .and_then(Value::as_str)
        .ok_or_else(|| "接口配置缺少业务域".to_string())?;
    let combo = object
        .get("combo")
        .and_then(Value::as_str)
        .ok_or_else(|| "接口配置缺少协议组合".to_string())?;
    let scope = object
        .get("scope")
        .and_then(Value::as_str)
        .ok_or_else(|| "接口配置缺少目标作用域".to_string())?;
    let supported = match domain {
        "domestic" => matches!(combo, "des_md5" | "sm" | "rsa_aes" | "des_sha256"),
        "cross_border" => matches!(combo, "sm" | "aes_sha256"),
        _ => false,
    };
    if !supported {
        return Err("该业务域不支持所选协议组合，请重新选择".to_string());
    }
    if combo == "sm" {
        if scope == "global" {
            return Ok(());
        }
        return Err("国密配置按业务域共享，目标作用域必须为global".to_string());
    }
    if scope == "global" {
        return Err("非国密配置必须选择具体产品，禁止域级回退".to_string());
    }
    let product = load_catalog()?
        .products
        .iter()
        .find(|product| product.id == scope)
        .ok_or_else(|| "产品不存在，请刷新接口配置目录后重试".to_string())?;
    for definition in &product.interfaces {
        let route = credential_route(&product.id, definition)?;
        if route.domain == domain && route.combo == combo {
            return Ok(());
        }
    }
    Err("该产品不适用当前业务域及协议组合，请重新选择".to_string())
}
