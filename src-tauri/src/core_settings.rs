//! Current-user settings. Only summaries may cross the command boundary.
//!
//! The caller supplies `app.path().app_config_dir()`. No scope or product ID is
//! ever interpreted as a path. The credential type deliberately has neither
//! `Debug` nor `Serialize`: plaintext must not enter logs or command responses.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use zeroize::{Zeroize, Zeroizing};

const SCHEMA_VERSION: u64 = 3;
const DOMAINS: [&str; 2] = ["domestic", "cross_border"];
const SETTINGS_FILE: &str = "user_settings.json";
const LOCK_FILE: &str = ".user_settings.lock";
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FIELD_BYTES: usize = 64 * 1024;

/// Runtime-only plaintext. Every owned credential is wiped when dropped.
#[derive(Clone, Default)]
pub(crate) struct UserApiCredentials {
    pub(crate) customer_number: Option<Zeroizing<String>>,
    pub(crate) md5_key: Option<Zeroizing<String>>,
    pub(crate) sha256_key: Option<Zeroizing<String>>,
    pub(crate) triple_des_key: Option<Zeroizing<String>>,
    pub(crate) aes_key: Option<Zeroizing<String>>,
    pub(crate) rsa_private_key: Option<Zeroizing<String>>,
    pub(crate) rsa_public_key: Option<Zeroizing<String>>,
    pub(crate) sm2_private_key: Option<Zeroizing<String>>,
    pub(crate) sm2_public_key: Option<Zeroizing<String>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    CustomerNumber,
    Md5Key,
    Sha256Key,
    TripleDesKey,
    AesKey,
    RsaPrivateKey,
    RsaPublicKey,
    Sm2PrivateKey,
    Sm2PublicKey,
}

impl Field {
    const ALL: [Self; 9] = [
        Self::CustomerNumber,
        Self::Md5Key,
        Self::Sha256Key,
        Self::TripleDesKey,
        Self::AesKey,
        Self::RsaPrivateKey,
        Self::RsaPublicKey,
        Self::Sm2PrivateKey,
        Self::Sm2PublicKey,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::CustomerNumber => "customer_number",
            Self::Md5Key => "md5_key",
            Self::Sha256Key => "sha256_key",
            Self::TripleDesKey => "triple_des_key",
            Self::AesKey => "aes_key",
            Self::RsaPrivateKey => "rsa_private_key",
            Self::RsaPublicKey => "rsa_public_key",
            Self::Sm2PrivateKey => "sm2_private_key",
            Self::Sm2PublicKey => "sm2_public_key",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::CustomerNumber => "商户编号",
            Self::Md5Key => "MD5签名密钥",
            Self::Sha256Key => "SHA256签名密钥",
            Self::TripleDesKey => "3DES加密密钥",
            Self::AesKey => "AES加密密钥",
            Self::RsaPrivateKey => "商户RSA私钥",
            Self::RsaPublicKey => "平台RSA公钥",
            Self::Sm2PrivateKey => "商户SM2私钥",
            Self::Sm2PublicKey => "平台SM2公钥",
        }
    }

    fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|field| field.name() == name)
            .ok_or_else(|| "用户配置包含未知字段".to_owned())
    }

    fn get(self, credentials: &UserApiCredentials) -> &Option<Zeroizing<String>> {
        match self {
            Self::CustomerNumber => &credentials.customer_number,
            Self::Md5Key => &credentials.md5_key,
            Self::Sha256Key => &credentials.sha256_key,
            Self::TripleDesKey => &credentials.triple_des_key,
            Self::AesKey => &credentials.aes_key,
            Self::RsaPrivateKey => &credentials.rsa_private_key,
            Self::RsaPublicKey => &credentials.rsa_public_key,
            Self::Sm2PrivateKey => &credentials.sm2_private_key,
            Self::Sm2PublicKey => &credentials.sm2_public_key,
        }
    }

    fn get_mut(self, credentials: &mut UserApiCredentials) -> &mut Option<Zeroizing<String>> {
        match self {
            Self::CustomerNumber => &mut credentials.customer_number,
            Self::Md5Key => &mut credentials.md5_key,
            Self::Sha256Key => &mut credentials.sha256_key,
            Self::TripleDesKey => &mut credentials.triple_des_key,
            Self::AesKey => &mut credentials.aes_key,
            Self::RsaPrivateKey => &mut credentials.rsa_private_key,
            Self::RsaPublicKey => &mut credentials.rsa_public_key,
            Self::Sm2PrivateKey => &mut credentials.sm2_private_key,
            Self::Sm2PublicKey => &mut credentials.sm2_public_key,
        }
    }
}

impl UserApiCredentials {
    fn summary(&self) -> Value {
        let mut summary = Map::new();
        for field in Field::ALL {
            let value = field.get(self);
            summary.insert(
                field.name().to_owned(),
                if matches!(field, Field::CustomerNumber) {
                    value
                        .as_ref()
                        .map(|value| Value::String(value.as_str().to_owned()))
                        .unwrap_or(Value::Null)
                } else {
                    Value::Bool(value.is_some())
                },
            );
        }
        Value::Object(summary)
    }

    fn with_overrides(&self, product: Option<&Self>) -> Self {
        let mut effective = Self::default();
        for field in Field::ALL {
            *field.get_mut(&mut effective) = product
                .and_then(|product| field.get(product).as_ref())
                .or_else(|| field.get(self).as_ref())
                .cloned();
        }
        effective
    }
}

/// One active scope owns one merchant number and one or more protocol key sets.
/// In schema v3 this means either one concrete non-SM product or one shared SM domain.
#[derive(Clone, Default)]
struct ApiScope {
    customer_number: Option<Zeroizing<String>>,
    combos: BTreeMap<String, UserApiCredentials>,
}

impl ApiScope {
    fn summary(&self) -> Value {
        let combos: Map<String, Value> = self
            .combos
            .iter()
            .map(|(combo, credentials)| (combo.clone(), credentials.summary()))
            .collect();
        json!({
            "customer_number": self.customer_number.as_ref().map(|value| value.as_str()),
            "combos": combos,
        })
    }
}

/// Frozen schema-v2 source. It is never consulted by execution and never mutated.
#[derive(Clone, Default)]
struct ApiDomain {
    defaults: ApiScope,
    products: BTreeMap<String, ApiScope>,
}

impl ApiDomain {
    fn summary(&self) -> Value {
        let mut summary = self.defaults.summary();
        let products: Map<String, Value> = self
            .products
            .iter()
            .map(|(product, scope)| (product.clone(), scope.summary()))
            .collect();
        summary["products"] = Value::Object(products);
        summary
    }
}

/// Keep untouched legacy ciphertext strings, including their original entropy namespaces.
#[derive(Clone)]
struct LegacyDisk {
    global: DiskScope,
    products: BTreeMap<String, DiskScope>,
}

#[derive(Clone, Default)]
struct Settings {
    revision: u64,
    // Schema-v1 compatibility/import sources only.
    global: UserApiCredentials,
    products: BTreeMap<String, UserApiCredentials>,
    legacy_disk: Option<LegacyDisk>,
    // Schema-v2 compatibility/import sources only; execution never consults these.
    frozen_domains: BTreeMap<String, ApiDomain>,
    frozen_domains_disk: Option<BTreeMap<String, DiskApiDomain>>,
    // Schema-v3 active model.
    api_products: BTreeMap<String, ApiScope>,
    sm_domains: BTreeMap<String, ApiScope>,
}

impl Settings {
    fn snapshot(&self) -> Value {
        let legacy_products: Map<String, Value> = self
            .products
            .iter()
            .map(|(product, credentials)| (product.clone(), credentials.summary()))
            .collect();
        let api_products: Map<String, Value> = self
            .api_products
            .iter()
            .map(|(product, scope)| (product.clone(), scope.summary()))
            .collect();
        let sm_domains: Map<String, Value> = DOMAINS
            .into_iter()
            .map(|domain| {
                (
                    domain.to_owned(),
                    self.sm_domains
                        .get(domain)
                        .map_or_else(|| ApiScope::default().summary(), ApiScope::summary),
                )
            })
            .collect();
        let frozen_domains: Map<String, Value> = DOMAINS
            .into_iter()
            .map(|domain| {
                (
                    domain.to_owned(),
                    self.frozen_domains
                        .get(domain)
                        .map_or_else(|| ApiDomain::default().summary(), ApiDomain::summary),
                )
            })
            .collect();
        let v1_available = Field::ALL
            .into_iter()
            .any(|field| field.get(&self.global).is_some())
            || self.products.values().any(|credentials| {
                Field::ALL
                    .into_iter()
                    .any(|field| field.get(credentials).is_some())
            });
        let v2_available = self.frozen_domains.values().any(|domain| {
            scope_has_non_sm(&domain.defaults) || domain.products.values().any(scope_has_non_sm)
        });
        json!({
            "revision": self.revision,
            "products": api_products,
            "sm_domains": sm_domains,
            "legacy_domains": frozen_domains,
            "legacy_v1": {
                "global": self.global.summary(),
                "products": legacy_products,
            },
            "legacy_available": v1_available || v2_available,
        })
    }
}

fn scope_has_non_sm(scope: &ApiScope) -> bool {
    scope.customer_number.is_some()
        || scope.combos.iter().any(|(combo, credentials)| {
            combo != "sm"
                && combo_fields(combo)
                    .is_ok_and(|fields| fields.iter().any(|field| field.get(credentials).is_some()))
        })
}

// Only encrypted values can inhabit these serializable types.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskSettings {
    schema_version: u64,
    revision: u64,
    global: DiskScope,
    products: BTreeMap<String, DiskScope>,
    // v2 domains become immutable import sources in v3.
    #[serde(default, deserialize_with = "deserialize_domains")]
    domains: Option<BTreeMap<String, DiskApiDomain>>,
    // Presence is mandatory for v3 and forbidden for v1/v2.
    #[serde(default, deserialize_with = "deserialize_api_scopes")]
    api_products: Option<BTreeMap<String, DiskApiScope>>,
    #[serde(default, deserialize_with = "deserialize_api_scopes")]
    sm_domains: Option<BTreeMap<String, DiskApiScope>>,
}

fn deserialize_domains<'de, D>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, DiskApiDomain>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    BTreeMap::deserialize(deserializer).map(Some)
}

fn deserialize_api_scopes<'de, D>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, DiskApiScope>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    BTreeMap::deserialize(deserializer).map(Some)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskScope {
    customer_number: Option<String>,
    encrypted_secrets: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskApiDomain {
    customer_number: Option<String>,
    combos: BTreeMap<String, DiskCombo>,
    products: BTreeMap<String, DiskApiScope>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskApiScope {
    customer_number: Option<String>,
    combos: BTreeMap<String, DiskCombo>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskCombo {
    encrypted_secrets: BTreeMap<String, String>,
}

/// A consumed command input is wiped even if validation or persistence fails.
struct SensitiveInput(Value);

impl Drop for SensitiveInput {
    fn drop(&mut self) {
        wipe_json(&mut self.0);
    }
}

fn wipe_json(value: &mut Value) {
    match value {
        Value::String(value) => value.zeroize(),
        Value::Array(values) => values.iter_mut().for_each(wipe_json),
        Value::Object(values) => {
            for (mut key, mut value) in std::mem::take(values) {
                key.zeroize();
                wipe_json(&mut value);
            }
        }
        _ => {}
    }
}

struct StoreInner {
    directory: PathBuf,
    settings: Settings,
}

/// Clones serialize in-process; a Windows byte-range lock serializes independent
/// stores/processes. Every writer reloads disk under that lock before merging.
#[derive(Clone)]
pub(crate) struct SettingsStore {
    inner: Arc<Mutex<StoreInner>>,
}

impl SettingsStore {
    pub(crate) fn open(directory: PathBuf) -> Result<Self, String> {
        let settings = read_settings(&directory)?.unwrap_or_default();
        Ok(Self {
            inner: Arc::new(Mutex::new(StoreInner {
                directory,
                settings,
            })),
        })
    }

    pub(crate) fn snapshot(&self) -> Result<Value, String> {
        let mut inner = self.lock()?;
        refresh(&mut inner)?;
        Ok(inner.settings.snapshot())
    }

    /// Legacy compatibility only; command callers should use `save_api`.
    #[allow(dead_code)]
    pub(crate) fn save(&self, params: Value) -> Result<Value, String> {
        let mut params = SensitiveInput(params);
        let object = params
            .0
            .as_object_mut()
            .ok_or_else(|| "保存用户配置的参数必须是对象".to_owned())?;
        check_keys(object, &["revision", "scope", "changes"])?;
        let revision = object
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| "保存用户配置必须提供有效revision".to_owned())?;
        let scope = object
            .get("scope")
            .and_then(Value::as_str)
            .ok_or_else(|| "保存用户配置必须提供scope".to_owned())?
            .to_owned();
        validate_scope(&scope)?;
        let changes = object
            .get_mut("changes")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "保存用户配置的changes必须是对象".to_owned())?;
        let mut validated = Vec::with_capacity(changes.len());
        for (name, value) in changes {
            let field = Field::parse(name)?;
            let value = match value {
                Value::Null => None,
                Value::String(value) => {
                    // Take ownership before validation so every failure wipes it.
                    let value = Zeroizing::new(std::mem::take(value));
                    validate_field(field, &value)?;
                    Some(value)
                }
                _ => return Err("用户配置字段只能是字符串或null".to_owned()),
            };
            validated.push((field, value));
        }

        let mut inner = self.lock()?;
        let _disk_lock = DiskWriteLock::acquire(&inner.directory)?;
        let current = current_for_write(&inner)?;
        if current.revision != revision {
            return Err("用户配置revision冲突，请重新读取配置后重试".to_owned());
        }
        let mut candidate = current;
        let target = if scope == "global" {
            &mut candidate.global
        } else {
            candidate.products.entry(scope.clone()).or_default()
        };
        for (field, value) in validated {
            *field.get_mut(target) = value;
        }
        let updated_disk = encrypt_scope(target, &scope)?;
        if let Some(legacy) = &mut candidate.legacy_disk {
            if scope == "global" {
                legacy.global = updated_disk;
            } else {
                legacy.products.insert(scope, updated_disk);
            }
        }
        candidate.revision = next_revision(candidate.revision)?;
        persist_settings(&inner.directory, &candidate)?;
        // No in-memory mutation occurs until the atomic filesystem commit.
        inner.settings = candidate;
        Ok(inner.settings.snapshot())
    }

    /// Save only the selected domain/combo. Null removes that exact override.
    pub(crate) fn save_api(&self, params: Value) -> Result<Value, String> {
        let mut params = SensitiveInput(params);
        let object = params
            .0
            .as_object_mut()
            .ok_or_else(|| "保存接口配置的参数必须是对象".to_owned())?;
        check_keys(object, &["revision", "domain", "combo", "scope", "changes"])?;
        let target = ApiTarget::parse(object)?;
        let changes = object
            .get_mut("changes")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "保存接口配置的changes必须是对象".to_owned())?;
        let mut validated = Vec::with_capacity(changes.len());
        for (name, value) in changes {
            let field = Field::parse(name)?;
            validate_combo_field(&target.combo, field)?;
            let value = match value {
                Value::Null => None,
                Value::String(value) => {
                    let value = Zeroizing::new(std::mem::take(value));
                    validate_api_field(&target.combo, field, &value)?;
                    Some(value)
                }
                _ => return Err("接口配置字段只能是字符串或null".to_owned()),
            };
            validated.push((field, value));
        }
        self.commit_api(target.revision, |candidate| {
            let scope = if target.combo == "sm" {
                candidate.sm_domains.entry(target.domain).or_default()
            } else {
                candidate.api_products.entry(target.scope).or_default()
            };
            for (field, value) in validated {
                if field == Field::CustomerNumber {
                    scope.customer_number = value;
                } else {
                    let credentials = scope.combos.entry(target.combo.clone()).or_default();
                    *field.get_mut(credentials) = value;
                }
            }
            Ok(())
        })
    }

    /// Explicit migration copies only still-missing values, without mutating either legacy source.
    pub(crate) fn import_legacy(&self, params: Value) -> Result<Value, String> {
        let params = SensitiveInput(params);
        let object = params
            .0
            .as_object()
            .ok_or_else(|| "导入旧接口配置的参数必须是对象".to_owned())?;
        check_keys(object, &["revision", "domain", "combo", "scope", "source"])?;
        let target = ApiTarget::parse(object)?;
        let source = object
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| "导入旧接口配置必须明确选择source".to_owned())?
            .to_owned();
        let source = LegacySource::parse(&source)?;
        if let LegacySource::V2Domain { domain, product } = &source {
            if target.combo == "sm" || target.scope == "global" {
                return Err("旧域配置只能逐产品显式导入非国密配置".to_owned());
            }
            if domain != &target.domain {
                return Err("旧域配置来源必须与目标接口业务域一致".to_owned());
            }
            if product
                .as_deref()
                .is_some_and(|product| product != target.scope)
            {
                return Err("旧域产品来源只能导入同一目标产品".to_owned());
            }
        }
        self.commit_api(target.revision, |candidate| {
            let legacy = match &source {
                LegacySource::V2Domain { domain, product } => {
                    let legacy_domain = candidate
                        .frozen_domains
                        .get(domain)
                        .ok_or_else(|| "所选旧域配置来源不存在".to_owned())?;
                    match product {
                        Some(product) => {
                            legacy_domain_effective(legacy_domain, product, &target.combo)?
                        }
                        None => legacy_domain_defaults(legacy_domain, &target.combo)?,
                    }
                }
                LegacySource::V1Global => candidate.global.clone(),
                LegacySource::V1Product(product) => candidate
                    .products
                    .get(product)
                    .ok_or_else(|| "所选旧接口配置来源不存在".to_owned())?
                    .clone(),
            };
            let scope = if target.combo == "sm" {
                candidate.sm_domains.entry(target.domain).or_default()
            } else {
                candidate.api_products.entry(target.scope).or_default()
            };
            let mut copied = 0;
            if scope.customer_number.is_none() {
                if let Some(value) = legacy.customer_number.as_ref() {
                    validate_api_field(&target.combo, Field::CustomerNumber, value)?;
                    scope.customer_number = Some(value.clone());
                    copied += 1;
                }
            }
            let credentials = scope.combos.entry(target.combo.clone()).or_default();
            for &field in combo_fields(&target.combo)? {
                if field.get(credentials).is_none() {
                    if let Some(value) = field.get(&legacy) {
                        validate_api_field(&target.combo, field, value)?;
                        *field.get_mut(credentials) = Some(value.clone());
                        copied += 1;
                    }
                }
            }
            if copied == 0 {
                return Err("所选旧配置没有可导入的缺失字段，目标已有配置不会被覆盖".to_owned());
            }
            Ok(())
        })
    }

    fn commit_api(
        &self,
        revision: u64,
        update: impl FnOnce(&mut Settings) -> Result<(), String>,
    ) -> Result<Value, String> {
        let mut inner = self.lock()?;
        let _disk_lock = DiskWriteLock::acquire(&inner.directory)?;
        let mut candidate = current_for_write(&inner)?;
        if candidate.revision != revision {
            return Err("用户配置revision冲突，请重新读取配置后重试".to_owned());
        }
        update(&mut candidate)?;
        candidate.revision = next_revision(candidate.revision)?;
        persist_settings(&inner.directory, &candidate)?;
        inner.settings = candidate;
        Ok(inner.settings.snapshot())
    }

    /// Active execution is deliberately disjoint: non-SM reads only its concrete
    /// product, while SM reads only the selected domain's shared scope.
    pub(crate) fn effective_api_credentials(
        &self,
        product_id: &str,
        domain: &str,
        combo: &str,
    ) -> Result<UserApiCredentials, String> {
        validate_domain_combo(domain, combo)?;
        validate_scope(product_id)?;
        let mut inner = self.lock()?;
        refresh(&mut inner)?;
        let scope = if combo == "sm" {
            inner.settings.sm_domains.get(domain)
        } else {
            inner.settings.api_products.get(product_id)
        };
        let Some(scope) = scope else {
            return Ok(UserApiCredentials::default());
        };
        let mut effective = UserApiCredentials {
            customer_number: scope.customer_number.clone(),
            ..Default::default()
        };
        let credentials = scope.combos.get(combo);
        for &field in combo_fields(combo)? {
            *field.get_mut(&mut effective) = credentials
                .and_then(|credentials| field.get(credentials).as_ref())
                .cloned();
        }
        Ok(effective)
    }

    /// Legacy compatibility only; new API execution must not call this method.
    #[allow(dead_code)]
    pub(crate) fn effective_credentials(
        &self,
        product_id: &str,
    ) -> Result<UserApiCredentials, String> {
        let mut inner = self.lock()?;
        refresh(&mut inner)?;
        Ok(inner
            .settings
            .global
            .with_overrides(inner.settings.products.get(product_id)))
    }

    fn lock(&self) -> Result<MutexGuard<'_, StoreInner>, String> {
        self.inner
            .lock()
            .map_err(|_| "用户配置状态锁已损坏，请重新启动应用后重试".to_owned())
    }
}

struct ApiTarget {
    revision: u64,
    domain: String,
    combo: String,
    scope: String,
}

impl ApiTarget {
    fn parse(object: &Map<String, Value>) -> Result<Self, String> {
        let revision = object
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| "接口配置必须提供有效revision".to_owned())?;
        let domain = object
            .get("domain")
            .and_then(Value::as_str)
            .ok_or_else(|| "接口配置必须提供domain".to_owned())?;
        let combo = object
            .get("combo")
            .and_then(Value::as_str)
            .ok_or_else(|| "接口配置必须提供combo".to_owned())?;
        validate_domain_combo(domain, combo)?;
        let scope = object
            .get("scope")
            .and_then(Value::as_str)
            .ok_or_else(|| "接口配置必须提供scope".to_owned())?;
        validate_scope(scope)?;
        if combo == "sm" {
            if scope != "global" {
                return Err("国密配置只能保存到业务域共享作用域".to_owned());
            }
        } else if scope == "global" {
            return Err("非国密配置必须保存到具体产品，禁止域级默认配置".to_owned());
        }
        Ok(Self {
            revision,
            domain: domain.to_owned(),
            combo: combo.to_owned(),
            scope: scope.to_owned(),
        })
    }
}

enum LegacySource {
    V1Global,
    V1Product(String),
    V2Domain {
        domain: String,
        product: Option<String>,
    },
}

impl LegacySource {
    fn parse(source: &str) -> Result<Self, String> {
        if source == "global" {
            return Ok(Self::V1Global);
        }
        if let Some(rest) = source.strip_prefix("domain:") {
            let mut parts = rest.split(":product:");
            let domain = parts.next().unwrap_or_default();
            validate_domain(domain)?;
            let product = match parts.next() {
                None => None,
                Some(product) => {
                    if parts.next().is_some() {
                        return Err("旧域配置source格式无效".to_owned());
                    }
                    validate_product_scope(product)?;
                    Some(product.to_owned())
                }
            };
            return Ok(Self::V2Domain {
                domain: domain.to_owned(),
                product,
            });
        }
        validate_product_scope(source)?;
        Ok(Self::V1Product(source.to_owned()))
    }
}

fn validate_domain(domain: &str) -> Result<(), String> {
    if !DOMAINS.contains(&domain) {
        return Err("接口配置domain无效".to_owned());
    }
    Ok(())
}

fn validate_domain_combo(domain: &str, combo: &str) -> Result<(), String> {
    validate_domain(domain)?;
    let valid = match domain {
        "domestic" => matches!(combo, "des_md5" | "sm" | "rsa_aes" | "des_sha256"),
        "cross_border" => matches!(combo, "sm" | "aes_sha256"),
        _ => false,
    };
    if !valid {
        return Err("接口配置domain与combo组合无效".to_owned());
    }
    Ok(())
}

fn legacy_domain_defaults(domain: &ApiDomain, combo: &str) -> Result<UserApiCredentials, String> {
    let mut effective = UserApiCredentials {
        customer_number: domain.defaults.customer_number.clone(),
        ..UserApiCredentials::default()
    };
    if let Some(defaults) = domain.defaults.combos.get(combo) {
        for &field in combo_fields(combo)? {
            *field.get_mut(&mut effective) = field.get(defaults).clone();
        }
    }
    Ok(effective)
}

fn legacy_domain_effective(
    domain: &ApiDomain,
    product_id: &str,
    combo: &str,
) -> Result<UserApiCredentials, String> {
    let product = domain.products.get(product_id);
    let defaults = domain.defaults.combos.get(combo);
    let overrides = product.and_then(|scope| scope.combos.get(combo));
    let mut effective = UserApiCredentials {
        customer_number: product
            .and_then(|scope| scope.customer_number.as_ref())
            .or(domain.defaults.customer_number.as_ref())
            .cloned(),
        ..Default::default()
    };
    for &field in combo_fields(combo)? {
        *field.get_mut(&mut effective) = overrides
            .and_then(|credentials| field.get(credentials).as_ref())
            .or_else(|| defaults.and_then(|credentials| field.get(credentials).as_ref()))
            .cloned();
    }
    Ok(effective)
}

fn combo_fields(combo: &str) -> Result<&'static [Field], String> {
    match combo {
        "des_md5" => Ok(&[Field::TripleDesKey, Field::Md5Key]),
        "sm" => Ok(&[Field::Sm2PrivateKey, Field::Sm2PublicKey]),
        "rsa_aes" => Ok(&[Field::RsaPrivateKey, Field::RsaPublicKey]),
        "aes_sha256" => Ok(&[Field::AesKey, Field::Sha256Key]),
        "des_sha256" => Ok(&[Field::TripleDesKey, Field::Sha256Key]),
        _ => Err("接口配置combo无效".to_owned()),
    }
}

fn validate_combo_field(combo: &str, field: Field) -> Result<(), String> {
    if field != Field::CustomerNumber && !combo_fields(combo)?.contains(&field) {
        return Err("接口配置字段不属于当前combo".to_owned());
    }
    Ok(())
}

fn validate_api_field(combo: &str, field: Field, value: &str) -> Result<(), String> {
    validate_combo_field(combo, field)?;
    validate_field(field, value)?;
    if combo == "des_sha256" && field == Field::TripleDesKey && value.len() != 24 {
        return Err("3DES/SHA256组合的3DES加密密钥必须为24字节".to_owned());
    }
    Ok(())
}

fn check_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("用户配置参数包含未知字段".to_owned());
    }
    Ok(())
}

fn validate_scope(scope: &str) -> Result<(), String> {
    if scope.trim().is_empty() || scope.len() > 256 || scope.chars().any(char::is_control) {
        return Err("用户配置scope无效".to_owned());
    }
    Ok(())
}

fn validate_product_scope(scope: &str) -> Result<(), String> {
    validate_scope(scope)?;
    if scope == "global" {
        return Err("用户配置产品scope无效，未重置原文件".to_owned());
    }
    Ok(())
}

fn validate_field(field: Field, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{}不能为空白；清除覆盖请传null", field.label()));
    }
    if value.len() > MAX_FIELD_BYTES {
        return Err(format!("{}超过允许长度", field.label()));
    }
    // The crypto helpers only ever return fixed sentences that describe the expected
    // shape (Base64/PEM/PKCS#1/PKCS#8/位数)，绝不回显提交内容；把这类提示透出给用户，
    // 否则“格式无效”无法指导用户改哪里。
    crate::feature_api_crypto::validate_credential_field(field.name(), value)
        .map_err(|reason| format!("{}：{reason}", field.label()))
}

fn next_revision(revision: u64) -> Result<u64, String> {
    revision
        .checked_add(1)
        .ok_or_else(|| "用户配置revision已达到上限，不能继续修改".to_owned())
}

fn refresh(inner: &mut StoreInner) -> Result<(), String> {
    let current = current_for_write(inner)?;
    inner.settings = current;
    Ok(())
}

fn current_for_write(inner: &StoreInner) -> Result<Settings, String> {
    match read_settings(&inner.directory)? {
        Some(current) => Ok(current),
        None if inner.settings.revision == 0 => Ok(Settings::default()),
        None => Err("已存在的用户配置文件丢失，请恢复文件后重试".to_owned()),
    }
}

fn read_settings(directory: &Path) -> Result<Option<Settings>, String> {
    let path = directory.join(SETTINGS_FILE);
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("读取文件", &error)),
    };
    // A bounded reader also handles files that grow after metadata is queried.
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error("读取文件", &error))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("用户配置文件超过允许大小，拒绝读取".to_owned());
    }
    let disk: DiskSettings = serde_json::from_slice(&bytes)
        .map_err(|_| "用户配置文件损坏或包含未知字段，未重置原文件".to_owned())?;
    let valid_schema = match disk.schema_version {
        1 => disk.domains.is_none() && disk.api_products.is_none() && disk.sm_domains.is_none(),
        2 => disk.domains.is_some() && disk.api_products.is_none() && disk.sm_domains.is_none(),
        SCHEMA_VERSION => {
            disk.domains.is_some() && disk.api_products.is_some() && disk.sm_domains.is_some()
        }
        _ => false,
    };
    if !valid_schema {
        return Err("用户配置schema版本不受支持，未修改原文件".to_owned());
    }
    if disk.revision == 0 {
        return Err("用户配置文件revision无效，未重置原文件".to_owned());
    }
    let schema_version = disk.schema_version;
    let legacy_disk = Some(LegacyDisk {
        global: disk.global.clone(),
        products: disk.products.clone(),
    });
    let global = decrypt_scope(disk.global, "global")?;
    let mut products = BTreeMap::new();
    for (product, credentials) in disk.products {
        validate_product_scope(&product)?;
        products.insert(product.clone(), decrypt_scope(credentials, &product)?);
    }

    let frozen_domains_disk = disk.domains.clone();
    let mut frozen_domains = BTreeMap::new();
    for (domain, stored) in disk.domains.unwrap_or_default() {
        validate_domain(&domain)?;
        let defaults = decrypt_v2_api_scope(
            DiskApiScope {
                customer_number: stored.customer_number,
                combos: stored.combos,
            },
            &domain,
            "global",
        )?;
        let mut domain_products = BTreeMap::new();
        for (product, scope) in stored.products {
            validate_product_scope(&product)?;
            domain_products.insert(
                product.clone(),
                decrypt_v2_api_scope(scope, &domain, &product)?,
            );
        }
        frozen_domains.insert(
            domain,
            ApiDomain {
                defaults,
                products: domain_products,
            },
        );
    }

    let mut api_products = BTreeMap::new();
    for (product, stored) in disk.api_products.unwrap_or_default() {
        validate_product_scope(&product)?;
        api_products.insert(product.clone(), decrypt_product_scope(stored, &product)?);
    }
    let mut sm_domains = BTreeMap::new();
    if schema_version == 2 {
        // Only the old domain default was shared. Product-level SM overrides stay
        // frozen and can never become active implicitly.
        for (domain, legacy) in &frozen_domains {
            let mut shared = ApiScope {
                customer_number: legacy.defaults.customer_number.clone(),
                combos: BTreeMap::new(),
            };
            if let Some(credentials) = legacy.defaults.combos.get("sm") {
                shared.combos.insert("sm".to_owned(), credentials.clone());
            }
            if shared.customer_number.is_some() || shared.combos.contains_key("sm") {
                sm_domains.insert(domain.clone(), shared);
            }
        }
    } else {
        for (domain, stored) in disk.sm_domains.unwrap_or_default() {
            validate_domain(&domain)?;
            sm_domains.insert(domain.clone(), decrypt_sm_domain_scope(stored, &domain)?);
        }
    }
    Ok(Some(Settings {
        revision: disk.revision,
        global,
        products,
        legacy_disk,
        frozen_domains,
        frozen_domains_disk,
        api_products,
        sm_domains,
    }))
}

fn encrypt_scope(credentials: &UserApiCredentials, scope: &str) -> Result<DiskScope, String> {
    let mut encrypted_secrets = BTreeMap::new();
    for field in Field::ALL.into_iter().skip(1) {
        if let Some(value) = field.get(credentials) {
            let encrypted = dpapi::protect(value.as_bytes(), &entropy(scope, field))?;
            encrypted_secrets.insert(field.name().to_owned(), STANDARD.encode(&*encrypted));
        }
    }
    Ok(DiskScope {
        customer_number: credentials
            .customer_number
            .as_ref()
            .map(|value| value.as_str().to_owned()),
        encrypted_secrets,
    })
}

fn decrypt_scope(disk: DiskScope, scope: &str) -> Result<UserApiCredentials, String> {
    let mut credentials = UserApiCredentials::default();
    if let Some(value) = disk.customer_number {
        let value = Zeroizing::new(value);
        validate_field(Field::CustomerNumber, &value)?;
        credentials.customer_number = Some(value);
    }
    for (name, ciphertext) in disk.encrypted_secrets {
        let field = Field::parse(&name)?;
        if matches!(field, Field::CustomerNumber) {
            return Err("用户配置密文字段无效，未重置原文件".to_owned());
        }
        let encrypted = Zeroizing::new(
            STANDARD
                .decode(ciphertext)
                .map_err(|_| "用户配置密文编码损坏，未重置原文件".to_owned())?,
        );
        let plaintext = dpapi::unprotect(&encrypted, &entropy(scope, field))?;
        let text = std::str::from_utf8(&plaintext)
            .map_err(|_| "用户配置解密内容无效，未重置原文件".to_owned())?;
        validate_field(field, text)?;
        *field.get_mut(&mut credentials) = Some(Zeroizing::new(text.to_owned()));
    }
    Ok(credentials)
}

fn v2_api_entropy(domain: &str, scope: &str, combo: &str, field: Field) -> Vec<u8> {
    format!(
        "PaySubmit.user-settings.v2.api\0{domain}\0{scope}\0{combo}\0{}",
        field.name()
    )
    .into_bytes()
}

fn v3_product_entropy(product: &str, combo: &str, field: Field) -> Vec<u8> {
    format!(
        "PaySubmit.user-settings.v3.api\0product\0{product}\0{combo}\0{}",
        field.name()
    )
    .into_bytes()
}

fn v3_sm_domain_entropy(domain: &str, field: Field) -> Vec<u8> {
    format!(
        "PaySubmit.user-settings.v3.api\0sm_domain\0{domain}\0sm\0{}",
        field.name()
    )
    .into_bytes()
}

fn decrypt_v2_api_scope(
    disk: DiskApiScope,
    domain: &str,
    scope_id: &str,
) -> Result<ApiScope, String> {
    decrypt_api_scope_with(disk, |combo, field| {
        validate_domain_combo(domain, combo)?;
        Ok(v2_api_entropy(domain, scope_id, combo, field))
    })
}

fn encrypt_product_scope(scope: &ApiScope, product: &str) -> Result<DiskApiScope, String> {
    encrypt_api_scope_with(scope, |combo, field| {
        if combo == "sm" {
            return Err("活动产品配置不能包含国密combo".to_owned());
        }
        combo_fields(combo)?;
        Ok(v3_product_entropy(product, combo, field))
    })
}

fn decrypt_product_scope(disk: DiskApiScope, product: &str) -> Result<ApiScope, String> {
    decrypt_api_scope_with(disk, |combo, field| {
        if combo == "sm" {
            return Err("活动产品配置不能包含国密combo".to_owned());
        }
        combo_fields(combo)?;
        Ok(v3_product_entropy(product, combo, field))
    })
}

fn encrypt_sm_domain_scope(scope: &ApiScope, domain: &str) -> Result<DiskApiScope, String> {
    encrypt_api_scope_with(scope, |combo, field| {
        if combo != "sm" {
            return Err("活动国密域配置只能包含sm combo".to_owned());
        }
        validate_domain_combo(domain, combo)?;
        Ok(v3_sm_domain_entropy(domain, field))
    })
}

fn decrypt_sm_domain_scope(disk: DiskApiScope, domain: &str) -> Result<ApiScope, String> {
    decrypt_api_scope_with(disk, |combo, field| {
        if combo != "sm" {
            return Err("活动国密域配置只能包含sm combo".to_owned());
        }
        validate_domain_combo(domain, combo)?;
        Ok(v3_sm_domain_entropy(domain, field))
    })
}

fn encrypt_api_scope_with<F>(scope: &ApiScope, entropy_for: F) -> Result<DiskApiScope, String>
where
    F: Fn(&str, Field) -> Result<Vec<u8>, String>,
{
    let mut combos = BTreeMap::new();
    for (combo, credentials) in &scope.combos {
        let mut encrypted_secrets = BTreeMap::new();
        for &field in combo_fields(combo)? {
            if let Some(value) = field.get(credentials) {
                validate_api_field(combo, field, value)?;
                let ciphertext = dpapi::protect(value.as_bytes(), &entropy_for(combo, field)?)?;
                encrypted_secrets.insert(field.name().to_owned(), STANDARD.encode(&*ciphertext));
            }
        }
        combos.insert(combo.clone(), DiskCombo { encrypted_secrets });
    }
    Ok(DiskApiScope {
        customer_number: scope
            .customer_number
            .as_ref()
            .map(|value| value.as_str().to_owned()),
        combos,
    })
}

fn decrypt_api_scope_with<F>(disk: DiskApiScope, entropy_for: F) -> Result<ApiScope, String>
where
    F: Fn(&str, Field) -> Result<Vec<u8>, String>,
{
    let mut scope = ApiScope::default();
    if let Some(value) = disk.customer_number {
        let value = Zeroizing::new(value);
        validate_field(Field::CustomerNumber, &value)?;
        scope.customer_number = Some(value);
    }
    for (combo, stored) in disk.combos {
        combo_fields(&combo)?;
        let mut credentials = UserApiCredentials::default();
        for (name, ciphertext) in stored.encrypted_secrets {
            let field = Field::parse(&name)?;
            if field == Field::CustomerNumber {
                return Err("接口配置商户编号必须保存在scope父节点".to_owned());
            }
            validate_combo_field(&combo, field)?;
            let ciphertext = Zeroizing::new(
                STANDARD
                    .decode(ciphertext)
                    .map_err(|_| "用户配置密文编码损坏，未重置原文件".to_owned())?,
            );
            let plaintext = dpapi::unprotect(&ciphertext, &entropy_for(&combo, field)?)?;
            let value = std::str::from_utf8(&plaintext)
                .map_err(|_| "用户配置解密内容无效，未重置原文件".to_owned())?;
            validate_api_field(&combo, field, value)?;
            *field.get_mut(&mut credentials) = Some(Zeroizing::new(value.to_owned()));
        }
        scope.combos.insert(combo, credentials);
    }
    Ok(scope)
}

fn entropy(scope: &str, field: Field) -> Vec<u8> {
    // Bind ciphertext to its schema, exact scope, and field. A ciphertext cannot
    // be transplanted from an RSA private key to a public key or another scope.
    format!("PaySubmit.user-settings.v1\0{scope}\0{}", field.name()).into_bytes()
}

fn persist_settings(directory: &Path, settings: &Settings) -> Result<(), String> {
    let legacy = match &settings.legacy_disk {
        Some(legacy) => legacy.clone(),
        None => {
            let mut products = BTreeMap::new();
            for (product, credentials) in &settings.products {
                products.insert(product.clone(), encrypt_scope(credentials, product)?);
            }
            LegacyDisk {
                global: encrypt_scope(&settings.global, "global")?,
                products,
            }
        }
    };
    // Frozen v2 bytes are preserved exactly. Re-encrypting them under another
    // namespace would both mutate the import source and weaken context binding.
    let domains = settings.frozen_domains_disk.clone().unwrap_or_default();
    let mut api_products = BTreeMap::new();
    for (product, scope) in &settings.api_products {
        validate_product_scope(product)?;
        api_products.insert(product.clone(), encrypt_product_scope(scope, product)?);
    }
    let mut sm_domains = BTreeMap::new();
    for (domain, scope) in &settings.sm_domains {
        validate_domain(domain)?;
        sm_domains.insert(domain.clone(), encrypt_sm_domain_scope(scope, domain)?);
    }
    let disk = DiskSettings {
        schema_version: SCHEMA_VERSION,
        revision: settings.revision,
        global: legacy.global,
        products: legacy.products,
        domains: Some(domains),
        api_products: Some(api_products),
        sm_domains: Some(sm_domains),
    };
    let bytes = Zeroizing::new(
        serde_json::to_vec_pretty(&disk)
            .map_err(|_| "用户配置编码失败，原配置未修改".to_owned())?,
    );
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("用户配置超过允许大小，原配置未修改".to_owned());
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".user_settings-")
        .suffix(".tmp")
        .tempfile_in(directory)
        .map_err(|error| io_error("创建临时文件", &error))?;
    temporary
        .write_all(&bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| io_error("写入临时文件", &error))?;
    // into_temp_path closes the writer before Windows renames the file. The
    // TempPath guard removes the temporary on EVERY failure path.
    let temporary = temporary.into_temp_path();
    atomic_replace(&temporary, &directory.join(SETTINGS_FILE))?;
    Ok(())
}

fn io_error(context: &str, error: &std::io::Error) -> String {
    // Never include paths, parser diagnostics, or user-controlled values.
    match error.raw_os_error() {
        Some(code) => format!("用户配置{context}失败（系统错误码{code}），原配置未修改"),
        None => format!("用户配置{context}失败，原配置未修改"),
    }
}

#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // Same-directory rename, without COPY_ALLOWED: no delete-first or fallback
    // copy can expose a partial configuration. Existing destination stays intact
    // on a failed rename. WRITE_THROUGH requests durable completion.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(io_error("原子替换文件", &std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(_source: &Path, _target: &Path) -> Result<(), String> {
    Err("用户配置安全存储仅支持Windows当前用户DPAPI".to_owned())
}

struct DiskWriteLock {
    _file: File,
}

#[cfg(windows)]
impl Drop for DiskWriteLock {
    fn drop(&mut self) {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{Storage::FileSystem::UnlockFileEx, System::IO::OVERLAPPED};
        let mut overlapped = OVERLAPPED::default();
        // Explicit unlock avoids relying on delayed OS cleanup on handle close.
        // Closing the owned File is still the final fallback after this Drop.
        unsafe {
            UnlockFileEx(self._file.as_raw_handle(), 0, 1, 0, &mut overlapped);
        }
    }
}

impl DiskWriteLock {
    #[cfg(windows)]
    fn acquire(directory: &Path) -> Result<Self, String> {
        use std::{
            os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
            time::Duration,
            time::Instant,
        };
        use windows_sys::Win32::{
            Foundation::ERROR_LOCK_VIOLATION,
            Storage::FileSystem::{
                LockFileEx, FILE_SHARE_READ, FILE_SHARE_WRITE, LOCKFILE_EXCLUSIVE_LOCK,
                LOCKFILE_FAIL_IMMEDIATELY,
            },
            System::IO::OVERLAPPED,
        };
        fs::create_dir_all(directory).map_err(|error| io_error("创建目录", &error))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            // The coordination inode must not be deleted/replaced while any
            // participant owns it; all writers share READ/WRITE but not DELETE.
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(directory.join(LOCK_FILE))
            .map_err(|error| io_error("打开写入锁", &error))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut overlapped = OVERLAPPED::default();
            // Byte 0 of a persistent lock file coordinates all cooperating
            // processes. Closing this File releases the lock even on errors.
            let result = unsafe {
                LockFileEx(
                    file.as_raw_handle(),
                    LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                    0,
                    1,
                    0,
                    &mut overlapped,
                )
            };
            if result != 0 {
                return Ok(Self { _file: file });
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_LOCK_VIOLATION as i32) {
                return Err(io_error("获取写入锁", &error));
            }
            if Instant::now() >= deadline {
                return Err("用户配置写入锁等待超时，请稍后重试".to_owned());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[cfg(not(windows))]
    fn acquire(_directory: &Path) -> Result<Self, String> {
        Err("用户配置安全存储仅支持Windows当前用户DPAPI".to_owned())
    }
}

#[cfg(windows)]
mod dpapi {
    use super::Zeroizing;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    use zeroize::Zeroize;

    struct LocalBlob(CRYPT_INTEGER_BLOB);

    impl Drop for LocalBlob {
        fn drop(&mut self) {
            if !self.0.pbData.is_null() {
                // DPAPI owns this allocation. Wipe BEFORE LocalFree; the guard
                // also handles any output allocated on an API error path.
                unsafe {
                    std::slice::from_raw_parts_mut(self.0.pbData, self.0.cbData as usize).zeroize();
                    LocalFree(self.0.pbData.cast());
                }
            }
        }
    }

    pub(super) fn protect(input: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        transform(input, entropy, false)
    }

    pub(super) fn unprotect(input: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        transform(input, entropy, true)
    }

    fn transform(
        input: &[u8],
        entropy: &[u8],
        decrypt: bool,
    ) -> Result<Zeroizing<Vec<u8>>, String> {
        let input_len =
            u32::try_from(input.len()).map_err(|_| "用户配置密文长度无效".to_owned())?;
        let entropy_len =
            u32::try_from(entropy.len()).map_err(|_| "用户配置加密上下文无效".to_owned())?;
        let input = CRYPT_INTEGER_BLOB {
            cbData: input_len,
            pbData: input.as_ptr().cast_mut(),
        };
        let entropy = CRYPT_INTEGER_BLOB {
            cbData: entropy_len,
            pbData: entropy.as_ptr().cast_mut(),
        };
        let mut output = LocalBlob(CRYPT_INTEGER_BLOB::default());
        // No LOCAL_MACHINE flag: only the current Windows user can decrypt.
        // No prompt structure and UI_FORBIDDEN on BOTH APIs forbid DPAPI UI.
        let result = unsafe {
            if decrypt {
                CryptUnprotectData(
                    &input,
                    std::ptr::null_mut(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output.0,
                )
            } else {
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output.0,
                )
            }
        };
        if result == 0 || output.0.pbData.is_null() || output.0.cbData == 0 {
            return Err(if decrypt {
                "用户配置DPAPI解密失败：文件损坏或不属于当前Windows用户，未重置原文件".to_owned()
            } else {
                "用户配置DPAPI加密失败，原配置未修改".to_owned()
            });
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(output.0.pbData, output.0.cbData as usize) };
        Ok(Zeroizing::new(bytes.to_vec()))
    }
}

#[cfg(not(windows))]
mod dpapi {
    use super::Zeroizing;

    pub(super) fn protect(_input: &[u8], _entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        Err("用户配置安全存储仅支持Windows当前用户DPAPI".to_owned())
    }

    pub(super) fn unprotect(_input: &[u8], _entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        Err("用户配置安全存储仅支持Windows当前用户DPAPI".to_owned())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn test_directory() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("paysubmit-settings-test-")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn saves_and_reloads_local_api_credentials_without_exposing_secrets() {
        let directory = test_directory();
        let store = SettingsStore::open(directory.path().to_path_buf()).unwrap();
        let saved = store
            .save_api(serde_json::json!({
                "revision": 0,
                "domain": "domestic",
                "combo": "des_md5",
                "scope": "ATUPay",
                "changes": { "customer_number": "C0001", "md5_key": "SYNTHETIC-SECRET" }
            }))
            .unwrap();
        assert_eq!(saved["products"]["ATUPay"]["customer_number"], "C0001");
        assert_eq!(
            saved["products"]["ATUPay"]["combos"]["des_md5"]["md5_key"],
            true
        );
        assert!(!saved.to_string().contains("SYNTHETIC-SECRET"));
        drop(store);

        let reopened = SettingsStore::open(directory.path().to_path_buf()).unwrap();
        let credentials = reopened
            .effective_api_credentials("ATUPay", "domestic", "des_md5")
            .unwrap();
        assert_eq!(
            credentials.md5_key.as_deref().map(String::as_str),
            Some("SYNTHETIC-SECRET")
        );
    }

    #[test]
    fn rejects_secret_formatted_keys_without_echoing_value() {
        let directory = test_directory();
        let store = SettingsStore::open(directory.path().to_path_buf()).unwrap();
        let error = store
            .save_api(serde_json::json!({
                "revision": 0,
                "domain": "domestic",
                "combo": "rsa_aes",
                "scope": "ATUPay",
                "changes": { "rsa_public_key": "DO-NOT-ECHO-FIXTURE" }
            }))
            .unwrap_err();
        assert!(!error.contains("DO-NOT-ECHO-FIXTURE"));
        assert!(error.contains("RSA"), "错误应指出字段：{error}");
    }
}
