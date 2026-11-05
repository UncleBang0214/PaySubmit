use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};
use base64::{engine::general_purpose, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use des::TdesEde3;
use md5::{Digest as _, Md5};
use rand::{distributions::Alphanumeric, distributions::DistString, rngs::OsRng};
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey},
    traits::PublicKeyParts,
    Pkcs1v15Encrypt, Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey,
};
use sha2::Sha256;
use sm2::{
    dsa::{
        signature::{Signer, Verifier},
        Signature, SigningKey, VerifyingKey,
    },
    elliptic_curve::common::getrandom::SysRng,
    pke::{DecryptingKey, EncryptingKey},
    PublicKey as Sm2PublicKey, SecretKey as Sm2SecretKey,
};
use sm4::Sm4;

const SM2_IDENTITY: &str = "1234567812345678";
const SM4_IV_BASE64: &str = "AQ4Zvt54xKn9QaW86ZzWdg==";

/// 校验用户输入的凭据格式；错误仅描述字段，不包含敏感输入。
/// 合利宝既有 1024 位也有 2048 位的密钥，校验只是拦住明显无效的内容，
/// 不能把平台实际下发的老密钥挡在门外（1024 位 RSA 是对方协议的既成事实）。
const MIN_RSA_BITS: usize = 1024;

pub(crate) fn validate_credential_field(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 65_536 || value.contains('\0') {
        return Err("配置内容不能为空、包含空字符或超过64KiB".to_string());
    }
    match name {
        "customer_number" => {
            if value.len() > 128 || value.chars().any(char::is_control) {
                return Err("商户编号不能超过128字节或包含控制字符".to_string());
            }
        }
        "md5_key" | "sha256_key" => {
            if value.chars().any(char::is_control) {
                return Err("签名密钥不能包含换行或控制字符".to_string());
            }
        }
        "triple_des_key" => {
            if !matches!(value.len(), 16 | 24) {
                return Err("3DES密钥必须是16或24字节的文本".to_string());
            }
        }
        "aes_key" => {
            if !matches!(decode_base64(value, "AES密钥")?.len(), 16 | 24 | 32) {
                return Err("AES密钥必须是Base64编码，解码后为16、24或32字节".to_string());
            }
        }
        "rsa_private_key" => {
            let key = match decode_rsa_private_key(value) {
                Ok(key) => key,
                Err(reason) => {
                    // 最常见的手误：把平台公钥粘进了“商户RSA私钥”。
                    if decode_rsa_public_key(value).is_ok() {
                        return Err(
                            "这里是公钥，商户RSA私钥请填商户自己的私钥（PKCS#1 或 PKCS#8），两者不能互换"
                                .to_string(),
                        );
                    }
                    return Err(reason);
                }
            };
            key.validate()
                .map_err(|_| "私钥内容校验失败，可能已损坏".to_string())?;
            if key.n().bits() < MIN_RSA_BITS {
                return Err(format!(
                    "当前 {} 位，至少需要 {MIN_RSA_BITS} 位",
                    key.n().bits()
                ));
            }
        }
        "rsa_public_key" => {
            let key = decode_rsa_public_key(value)?;
            if key.n().bits() < MIN_RSA_BITS {
                return Err(format!(
                    "当前 {} 位，至少需要 {MIN_RSA_BITS} 位",
                    key.n().bits()
                ));
            }
        }
        "sm2_private_key" => {
            let scalar = zeroize::Zeroizing::new(decode_sm2_scalar(value)?);
            Sm2SecretKey::from_slice(scalar.as_ref()).map_err(|_| "商户SM2私钥无效".to_string())?;
        }
        "sm2_public_key" => {
            decode_sm2_public_key(value)?;
        }
        _ => return Err("不支持的接口凭据配置字段".to_string()),
    }
    Ok(())
}

pub(crate) fn random_key(length: usize) -> String {
    Alphanumeric.sample_string(&mut OsRng, length)
}

pub(crate) fn md5_hex(data: &str) -> String {
    hex::encode(Md5::digest(data.as_bytes()))
}

pub(crate) fn sha256_hex(data: &str) -> String {
    hex::encode(Sha256::digest(data.as_bytes()))
}

pub(crate) fn triple_des_encrypt(key: &str, data: &str) -> Result<String, String> {
    let key = normalize_triple_des_key(key.as_bytes())?;
    let cipher = TdesEde3::new_from_slice(&key)
        .map_err(|_| "3DES 密钥无法初始化，请检查密钥长度".to_string())?;
    let mut output = pkcs7_pad(data.as_bytes(), 8);
    for block in output.as_chunks_mut::<8>().0 {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
    Ok(general_purpose::STANDARD.encode(output))
}

pub(crate) fn triple_des_decrypt(key: &str, data: &str) -> Result<String, String> {
    let key = normalize_triple_des_key(key.as_bytes())?;
    let cipher = TdesEde3::new_from_slice(&key)
        .map_err(|_| "3DES 密钥无法初始化，请检查密钥长度".to_string())?;
    let mut output = decode_base64(data, "3DES 密文")?;
    if output.is_empty() || output.len() % 8 != 0 {
        return Err("3DES 密文长度无效".to_string());
    }
    for block in output.as_chunks_mut::<8>().0 {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    let output = pkcs7_unpad(&output, 8)?;
    String::from_utf8(output.to_vec()).map_err(|_| "3DES 解密结果不是 UTF-8 文本".to_string())
}

pub(crate) fn aes_ecb_encrypt(
    key: &str,
    data: &str,
    key_is_base64: bool,
) -> Result<String, String> {
    let key = if key_is_base64 {
        decode_base64(key, "AES 密钥")?
    } else {
        key.as_bytes().to_vec()
    };
    let mut output = pkcs7_pad(data.as_bytes(), 16);
    match key.len() {
        16 => encrypt_aes_blocks(&Aes128::new_from_slice(&key).unwrap(), &mut output),
        24 => encrypt_aes_blocks(&Aes192::new_from_slice(&key).unwrap(), &mut output),
        32 => encrypt_aes_blocks(&Aes256::new_from_slice(&key).unwrap(), &mut output),
        length => {
            return Err(format!(
                "AES 密钥解码后为 {length} 字节，只支持 16、24 或 32 字节"
            ))
        }
    }
    Ok(general_purpose::STANDARD.encode(output))
}

pub(crate) fn aes_ecb_decrypt(
    key: &str,
    data: &str,
    key_is_base64: bool,
) -> Result<String, String> {
    let key = if key_is_base64 {
        decode_base64(key, "AES 密钥")?
    } else {
        key.as_bytes().to_vec()
    };
    let mut output = decode_base64(data, "AES 密文")?;
    if output.is_empty() || output.len() % 16 != 0 {
        return Err("AES 密文长度无效".to_string());
    }
    match key.len() {
        16 => decrypt_aes_blocks(&Aes128::new_from_slice(&key).unwrap(), &mut output),
        24 => decrypt_aes_blocks(&Aes192::new_from_slice(&key).unwrap(), &mut output),
        32 => decrypt_aes_blocks(&Aes256::new_from_slice(&key).unwrap(), &mut output),
        length => {
            return Err(format!(
                "AES 密钥解码后为 {length} 字节，只支持 16、24 或 32 字节"
            ))
        }
    }
    let output = pkcs7_unpad(&output, 16)?;
    String::from_utf8(output.to_vec()).map_err(|_| "AES 解密结果不是 UTF-8 文本".to_string())
}

pub(crate) fn sm4_encrypt(key: &str, data: &str) -> Result<String, String> {
    let iv = decode_base64(SM4_IV_BASE64, "SM4 IV")?;
    let encryptor = cbc::Encryptor::<Sm4>::new_from_slices(key.as_bytes(), &iv)
        .map_err(|_| "SM4 密钥必须是 16 字节".to_string())?;
    let encrypted = encryptor.encrypt_padded_vec::<Pkcs7>(data.as_bytes());
    Ok(general_purpose::STANDARD.encode(encrypted))
}

pub(crate) fn sm4_decrypt(key: &str, data: &str) -> Result<String, String> {
    let iv = decode_base64(SM4_IV_BASE64, "SM4 IV")?;
    let decryptor = cbc::Decryptor::<Sm4>::new_from_slices(key.as_bytes(), &iv)
        .map_err(|_| "SM4 密钥必须是 16 字节".to_string())?;
    let encrypted = decode_base64(data, "SM4 密文")?;
    let decrypted = decryptor
        .decrypt_padded_vec::<Pkcs7>(&encrypted)
        .map_err(|_| "SM4 解密失败，请检查密钥、密文或填充".to_string())?;
    String::from_utf8(decrypted).map_err(|_| "SM4 解密结果不是 UTF-8 文本".to_string())
}

pub(crate) fn sm2_sign(private_key: &str, data: &str) -> Result<String, String> {
    let scalar = decode_sm2_scalar(private_key)?;
    let signing_key = SigningKey::from_slice(SM2_IDENTITY, &scalar)
        .map_err(|_| "商户 SM2 私钥无效".to_string())?;
    let signature: Signature = signing_key.sign(data.as_bytes());
    Ok(general_purpose::STANDARD.encode(signature.to_der().to_vec()))
}

pub(crate) fn sm2_verify(public_key: &str, data: &str, signature: &str) -> Result<bool, String> {
    let public_key = decode_sm2_public_key(public_key)?;
    let verifying_key = VerifyingKey::new(SM2_IDENTITY, public_key)
        .map_err(|_| "合利宝 SM2 公钥无效".to_string())?;
    let signature = decode_base64(signature, "SM2 签名")?;
    let signature =
        Signature::from_der(&signature).map_err(|_| "SM2 签名 DER 格式无效".to_string())?;
    Ok(verifying_key.verify(data.as_bytes(), &signature).is_ok())
}

pub(crate) fn sm2_encrypt(public_key: &str, data: &str) -> Result<String, String> {
    let public_key = decode_sm2_public_key(public_key)?;
    let encrypting_key = EncryptingKey::new(public_key);
    let encrypted = encrypting_key
        .encrypt(&mut SysRng, data.as_bytes())
        .map_err(|_| "SM2 加密失败".to_string())?;
    Ok(general_purpose::STANDARD.encode(encrypted))
}

pub(crate) fn sm2_decrypt(private_key: &str, data: &str) -> Result<String, String> {
    let scalar = decode_sm2_scalar(private_key)?;
    let secret_key =
        Sm2SecretKey::from_slice(&scalar).map_err(|_| "商户 SM2 私钥无效".to_string())?;
    let decrypting_key = DecryptingKey::new(secret_key);
    let encrypted = decode_base64(data, "SM2 密文")?;
    let decrypted = decrypting_key
        .decrypt(&encrypted)
        .map_err(|_| "SM2 解密失败".to_string())?;
    String::from_utf8(decrypted).map_err(|_| "SM2 解密结果不是 UTF-8 文本".to_string())
}

pub(crate) fn rsa_md5_sign(private_key: &str, data: &str) -> Result<String, String> {
    let private_key = decode_rsa_private_key(private_key)?;
    let digest = Md5::digest(data.as_bytes());
    let signature = private_key
        .sign(Pkcs1v15Sign::new::<Md5>(), &digest)
        .map_err(|_| "RSA MD5 签名失败".to_string())?;
    Ok(general_purpose::STANDARD.encode(signature))
}

pub(crate) fn rsa_md5_verify(
    public_key: &str,
    data: &str,
    signature: &str,
) -> Result<bool, String> {
    let public_key = decode_rsa_public_key(public_key)?;
    let signature = decode_base64(signature, "RSA 签名")?;
    let digest = Md5::digest(data.as_bytes());
    Ok(public_key
        .verify(Pkcs1v15Sign::new::<Md5>(), &digest, &signature)
        .is_ok())
}

pub(crate) fn rsa_encrypt(public_key: &str, data: &str) -> Result<String, String> {
    let public_key = decode_rsa_public_key(public_key)?;
    let block_size = public_key.size().saturating_sub(11);
    if block_size == 0 {
        return Err("RSA 公钥长度无效".to_string());
    }
    let mut output = Vec::new();
    for block in data.as_bytes().chunks(block_size) {
        let encrypted = public_key
            .encrypt(&mut OsRng, Pkcs1v15Encrypt, block)
            .map_err(|_| "RSA 加密失败".to_string())?;
        output.extend_from_slice(&encrypted);
    }
    Ok(general_purpose::STANDARD.encode(output))
}

fn normalize_triple_des_key(key: &[u8]) -> Result<[u8; 24], String> {
    if key.len() < 8 {
        return Err("3DES 密钥长度不能少于 8 字节".to_string());
    }
    let mut normalized = [0u8; 24];
    for (index, byte) in normalized.iter_mut().enumerate() {
        *byte = key[index % key.len()];
    }
    Ok(normalized)
}

fn encrypt_aes_blocks<C: BlockEncrypt>(cipher: &C, output: &mut [u8]) {
    for block in output.as_chunks_mut::<16>().0 {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
}

fn decrypt_aes_blocks<C: BlockDecrypt>(cipher: &C, output: &mut [u8]) {
    for block in output.as_chunks_mut::<16>().0 {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
}

fn pkcs7_pad(data: &[u8], block_size: usize) -> Vec<u8> {
    let padding = block_size - data.len() % block_size;
    let mut output = Vec::with_capacity(data.len() + padding);
    output.extend_from_slice(data);
    output.resize(data.len() + padding, padding as u8);
    output
}

fn pkcs7_unpad(data: &[u8], block_size: usize) -> Result<&[u8], String> {
    let padding = data.last().copied().map(usize::from).unwrap_or_default();
    if padding == 0 || padding > block_size || padding > data.len() {
        return Err("PKCS7 填充无效".to_string());
    }
    if data[data.len() - padding..]
        .iter()
        .any(|byte| usize::from(*byte) != padding)
    {
        return Err("PKCS7 填充无效".to_string());
    }
    Ok(&data[..data.len() - padding])
}

/// 去掉复制粘贴常带进来的不可见字符，它们会让 Base64 解码直接失败。
fn strip_invisible(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !matches!(
                character,
                '\u{feff}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}'
            )
        })
        .collect()
}

/// 还原 Java 源码里的字符串字面量拼接：`"段一" +  "段二"`。
/// 只在整个输入除了引号、加号、分号和空白之外没有别的字符时才生效，
/// 避免把真正含引号的密钥改坏；这些字符本来也不可能出现在 Base64 里。
fn join_java_literals(value: &str) -> Option<String> {
    let mut joined = String::new();
    let mut quotes = 0usize;
    let mut inside = false;
    let mut stray = false;
    for character in value.chars() {
        match character {
            '"' if !inside => {
                inside = true;
                quotes += 1;
            }
            '"' if inside => inside = false,
            _ if inside => joined.push(character),
            character if character.is_whitespace() || character == '+' || character == ';' => {}
            _ => stray = true,
        }
    }
    if quotes >= 2 && !inside && !stray {
        Some(joined)
    } else {
        None
    }
}

fn decode_base64(value: &str, label: &str) -> Result<Vec<u8>, String> {
    let cleaned = strip_invisible(value);
    let compact = cleaned
        .lines()
        .filter(|line| !line.trim_start().starts_with("-----"))
        .flat_map(|line| line.chars())
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let attempt = |candidate: &str| {
        general_purpose::STANDARD
            .decode(candidate)
            .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(candidate))
    };
    if let Ok(bytes) = attempt(&compact) {
        return Ok(bytes);
    }
    if let Some(joined) = join_java_literals(&cleaned) {
        let compact_joined = joined
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        if let Ok(bytes) = attempt(&compact_joined) {
            return Ok(bytes);
        }
    }
    Err(format!("{label}不是有效的 Base64 内容"))
}

fn decode_sm2_scalar(value: &str) -> Result<[u8; 32], String> {
    let bytes = decode_base64(value, "商户 SM2 私钥")?;
    if bytes.is_empty() || bytes.len() > 32 {
        return Err("商户 SM2 私钥必须是最多 32 字节的标量".to_string());
    }
    let mut scalar = [0u8; 32];
    scalar[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(scalar)
}

fn decode_sm2_public_key(value: &str) -> Result<Sm2PublicKey, String> {
    let bytes = decode_base64(value, "合利宝 SM2 公钥")?;
    Sm2PublicKey::from_sec1_bytes(&bytes)
        .map_err(|_| "合利宝 SM2 公钥必须是 Base64 编码的 SEC1 公钥".to_string())
}

fn decode_rsa_private_key(value: &str) -> Result<RsaPrivateKey, String> {
    let trimmed = strip_invisible(value.trim());
    let trimmed = trimmed.as_str();
    if trimmed.contains("BEGIN") {
        if let Ok(key) = RsaPrivateKey::from_pkcs8_pem(trimmed) {
            return Ok(key);
        }
        if let Ok(key) = RsaPrivateKey::from_pkcs1_pem(trimmed) {
            return Ok(key);
        }
    }
    let bytes = decode_base64(trimmed, "RSA 私钥")?;
    RsaPrivateKey::from_pkcs8_der(&bytes)
        .or_else(|_| RsaPrivateKey::from_pkcs1_der(&bytes))
        .map_err(|_| "不是有效的 PKCS#8 或 PKCS#1 私钥".to_string())
}

fn decode_rsa_public_key(value: &str) -> Result<RsaPublicKey, String> {
    let trimmed = strip_invisible(value.trim());
    let trimmed = trimmed.as_str();
    if trimmed.contains("BEGIN") {
        if let Ok(key) = RsaPublicKey::from_public_key_pem(trimmed) {
            return Ok(key);
        }
        if let Ok(key) = RsaPublicKey::from_pkcs1_pem(trimmed) {
            return Ok(key);
        }
    }
    let bytes = decode_base64(trimmed, "RSA 公钥")?;
    RsaPublicKey::from_public_key_der(&bytes)
        .or_else(|_| RsaPublicKey::from_pkcs1_der(&bytes))
        .map_err(|_| "不是有效的 SPKI 或 PKCS#1 公钥".to_string())
}
