//! 国密证书读取：将 SM2/PFX 私钥与 X.509 公钥证书转换为 Base64 文本，供本地密钥直接粘贴。
//! 移植自 YunsToolKit 的现成实现，密钥仅在内存中处理，不落盘、不回显日志。
use std::{fs, path::Path};

use base64::{engine::general_purpose, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockModeDecrypt, KeyIvInit};
use p12_keystore::Pkcs12Archive;
use serde::Deserialize;
use serde_json::{json, Value};
use sm3::{Digest, Sm3};
use sm4::Sm4;
use x509_cert::{
    der::{Decode, DecodePem},
    Certificate,
};
use yasna::models::ObjectIdentifier;
use zeroize::{Zeroize, Zeroizing};

const EC_PUBLIC_KEY_OID: &[u64] = &[1, 2, 840, 10045, 2, 1];
const SM2_CURVE_OID: &[u64] = &[1, 2, 156, 10197, 1, 301];
const EC_PUBLIC_KEY_OID_TEXT: &str = "1.2.840.10045.2.1";
const SM2_CURVE_OID_TEXT: &str = "1.2.156.10197.1.301";

#[derive(Deserialize)]
struct ExtractParams {
    #[serde(default)]
    private_key_path: String,
    #[serde(default)]
    public_cert_path: String,
    #[serde(default)]
    password: String,
}

pub(crate) fn extract_keys(params: Value) -> Result<Value, String> {
    let params: ExtractParams =
        serde_json::from_value(params).map_err(|error| format!("国密证书读取参数无效：{error}"))?;
    let ExtractParams {
        private_key_path,
        public_cert_path,
        password,
    } = params;
    let password = Zeroizing::new(password);
    let private_path = private_key_path.trim();
    let public_path = public_cert_path.trim();

    if private_path.is_empty() && public_path.is_empty() {
        return Err("国密证书读取至少需要选择一个私钥或公钥证书文件".to_string());
    }
    if !private_path.is_empty() && password.is_empty() {
        return Err("读取商户私钥需要填写私钥密码".to_string());
    }

    let (private_key_base64, private_key_format) = if private_path.is_empty() {
        (None, None)
    } else {
        let path = Path::new(private_path);
        let bytes = read_file(path, "商户私钥")?;
        let format = if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("pfx") || extension.eq_ignore_ascii_case("p12")
            }) {
            "pfx"
        } else {
            "sm2"
        };
        let scalar = if format == "pfx" {
            extract_pfx_scalar(&bytes, &password)?
        } else {
            extract_sm2_scalar(&bytes, &password)?
        };
        (
            Some(general_purpose::STANDARD.encode(scalar.as_slice())),
            Some(format),
        )
    };

    let (public_key_base64, public_key_format) = if public_path.is_empty() {
        (None, None)
    } else {
        let bytes = read_file(Path::new(public_path), "平台公钥证书")?;
        let public_key = extract_public_key(&bytes)?;
        (
            Some(general_purpose::STANDARD.encode(public_key)),
            Some("x509"),
        )
    };

    Ok(json!({
        "private_key_base64": private_key_base64,
        "public_key_base64": public_key_base64,
        "private_key_format": private_key_format,
        "public_key_format": public_key_format,
    }))
}

fn read_file(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    if !path.is_file() {
        return Err(format!("{label}文件不存在：{}", path.display()));
    }
    fs::read(path).map_err(|error| format!("读取{label}文件失败（{}）：{error}", path.display()))
}

fn extract_sm2_scalar(data: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let encrypted = extract_sm2_encrypted_data(data).or_else(|_| {
        let decoded = decode_base64(data, "SM2 文件")?;
        extract_sm2_encrypted_data(&decoded)
    })?;
    if !(32..=64).contains(&encrypted.len()) {
        return Err("SM2 文件中的加密私钥长度必须在 32 至 64 字节之间".to_string());
    }

    let ciphertext = if matches!(encrypted.len(), 32 | 48) {
        encrypted
    } else {
        match decode_base64(&encrypted, "SM2 加密私钥") {
            Ok(decoded) => decoded,
            Err(_) if encrypted.len() % 16 == 0 => encrypted,
            Err(error) => return Err(error),
        }
    };
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err("SM2 文件中的加密私钥不是有效的 SM4-CBC 密文".to_string());
    }

    let mut hasher = Sm3::new();
    hasher.update(password.as_bytes());
    hasher.update([0, 0, 0, 1]);
    let mut derived = hasher.finalize();
    let mut iv = [0u8; 16];
    let mut key = [0u8; 16];
    iv.copy_from_slice(&derived[..16]);
    key.copy_from_slice(&derived[16..]);

    let decrypted = cbc::Decryptor::<Sm4>::new(&key.into(), &iv.into())
        .decrypt_padded_vec::<Pkcs7>(&ciphertext)
        .map_err(|_| "SM2 私钥解密失败，请检查文件和密码是否正确".to_string());
    key.zeroize();
    iv.zeroize();
    derived.as_mut_slice().zeroize();

    let decrypted = Zeroizing::new(decrypted?);
    if decrypted.len() < 32 {
        return Err("SM2 私钥解密结果长度不足 32 字节".to_string());
    }
    normalize_scalar(&decrypted[..32])
}

fn extract_sm2_encrypted_data(data: &[u8]) -> Result<Vec<u8>, String> {
    yasna::parse_ber(data, |reader| {
        reader.read_sequence(|root| {
            root.next().read_der()?;
            let encrypted = root.next().read_sequence(|private_info| {
                private_info.next().read_der()?;
                private_info.next().read_der()?;
                let encrypted = private_info.next().read_bytes()?;
                consume_remaining(private_info)?;
                Ok(encrypted)
            })?;
            root.next().read_der()?;
            consume_remaining(root)?;
            Ok(encrypted)
        })
    })
    .map_err(|_| "SM2 文件的 ASN.1 结构无效或不完整".to_string())
}

fn consume_remaining(reader: &mut yasna::BERReaderSeq<'_, '_>) -> yasna::ASN1Result<()> {
    while reader.read_optional(|reader| reader.read_der())?.is_some() {}
    Ok(())
}

fn extract_pfx_scalar(data: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let archive = Pkcs12Archive::from_pkcs12(data, password)
        .map_err(|_| "PFX 私钥读取失败，请检查文件格式和密码是否正确".to_string())?;
    let key = archive
        .keys
        .first()
        .ok_or_else(|| "PFX 文件中没有找到私钥条目".to_string())?;
    extract_pkcs8_ec_scalar(key.key.as_der())
}

fn extract_pkcs8_ec_scalar(data: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    let (algorithm, ec_private_key) = yasna::parse_der(data, |reader| {
        reader.read_sequence(|private_key_info| {
            private_key_info.next().read_i64()?;
            let algorithm = private_key_info.next().read_sequence(|algorithm| {
                let oid = algorithm.next().read_oid()?;
                consume_remaining(algorithm)?;
                Ok(oid)
            })?;
            let private_key = private_key_info.next().read_bytes()?;
            consume_remaining(private_key_info)?;
            Ok((algorithm, private_key))
        })
    })
    .map_err(|_| "PFX 文件中的私钥不是有效的 PKCS#8 结构".to_string())?;

    if !is_ec_oid(&algorithm) {
        return Err(format!(
            "PFX 文件中的私钥不是 EC 私钥（算法 OID：{algorithm}）"
        ));
    }

    let scalar = yasna::parse_der(&ec_private_key, |reader| {
        reader.read_sequence(|ec_private_key| {
            let version = ec_private_key.next().read_i64()?;
            if version != 1 {
                return Err(yasna::ASN1Error::new(yasna::ASN1ErrorKind::Invalid));
            }
            let scalar = ec_private_key.next().read_bytes()?;
            consume_remaining(ec_private_key)?;
            Ok(scalar)
        })
    })
    .map_err(|_| "PFX 文件中的 EC 私钥结构无效".to_string())?;
    normalize_scalar(&scalar)
}

fn is_ec_oid(oid: &ObjectIdentifier) -> bool {
    oid.as_ref() == EC_PUBLIC_KEY_OID || oid.as_ref() == SM2_CURVE_OID
}

fn normalize_scalar(bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    let first_non_zero = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len());
    let scalar = &bytes[first_non_zero..];
    if scalar.is_empty() {
        return Err("私钥标量不能为零".to_string());
    }
    if scalar.len() > 32 {
        return Err(format!("私钥标量长度异常：{} 字节", scalar.len()));
    }
    Ok(Zeroizing::new(scalar.to_vec()))
}

fn extract_public_key(data: &[u8]) -> Result<Vec<u8>, String> {
    let certificate = if data
        .windows(b"-----BEGIN".len())
        .any(|part| part == b"-----BEGIN")
    {
        Certificate::from_pem(data).map_err(|_| "公钥证书 PEM 内容无效".to_string())?
    } else if let Ok(certificate) = Certificate::from_der(data) {
        certificate
    } else {
        let decoded = decode_base64(data, "公钥证书")?;
        Certificate::from_der(&decoded)
            .map_err(|_| "公钥证书不是有效的 X.509 DER 或 Base64 内容".to_string())?
    };

    let subject_public_key_info = certificate.tbs_certificate().subject_public_key_info();
    let algorithm_oid = subject_public_key_info.algorithm.oid.to_string();
    if algorithm_oid != EC_PUBLIC_KEY_OID_TEXT && algorithm_oid != SM2_CURVE_OID_TEXT {
        return Err(format!("公钥证书不是 EC 证书（算法 OID：{algorithm_oid}）"));
    }
    let subject_public_key = subject_public_key_info
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| "公钥证书中的 EC 公钥位串存在未对齐数据".to_string())?;
    if subject_public_key.len() != 65 || subject_public_key[0] != 0x04 {
        return Err("公钥证书中未找到 65 字节未压缩 EC 公钥".to_string());
    }
    Ok(subject_public_key.to_vec())
}

fn decode_base64(data: &[u8], label: &str) -> Result<Vec<u8>, String> {
    let compact = data
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    general_purpose::STANDARD
        .decode(&compact)
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(&compact))
        .map_err(|_| format!("{label}不是有效的 Base64 内容"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::{BlockModeEncrypt, KeyIvInit};
    use serde_json::json;
    use std::fs;
    use uuid::Uuid;

    fn sm2_fixture(password: &str, scalar: &[u8; 32]) -> Vec<u8> {
        let mut hasher = Sm3::new();
        hasher.update(password.as_bytes());
        hasher.update([0, 0, 0, 1]);
        let derived = hasher.finalize();
        let ciphertext = cbc::Encryptor::<Sm4>::new(
            derived[16..].try_into().unwrap(),
            derived[..16].try_into().unwrap(),
        )
        .encrypt_padded_vec::<Pkcs7>(scalar);

        yasna::construct_der(|writer| {
            writer.write_sequence(|root| {
                root.next().write_i64(1);
                root.next().write_sequence(|private_info| {
                    private_info.next().write_i64(1);
                    private_info.next().write_null();
                    private_info.next().write_bytes(&ciphertext);
                });
                root.next().write_sequence(|public_info| {
                    public_info.next().write_null();
                });
            });
        })
    }

    #[test]
    fn extracts_custom_sm2_private_key_and_releases_file() {
        let password = "test-password";
        let mut scalar = [0x5au8; 32];
        scalar[0] = 0;
        let root = std::env::temp_dir().join(format!("paysubmit-sm2-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("merchant.sm2");
        fs::write(&path, sm2_fixture(password, &scalar)).unwrap();

        let result = extract_keys(json!({
            "private_key_path": path,
            "public_cert_path": "",
            "password": password,
        }))
        .unwrap();

        assert_eq!(result["private_key_format"], "sm2");
        assert_eq!(
            general_purpose::STANDARD
                .decode(result["private_key_base64"].as_str().unwrap())
                .unwrap(),
            scalar[1..]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn decodes_base64_wrapped_sm2_container() {
        let password = "test-password";
        let scalar = [0x23u8; 32];
        let encoded = general_purpose::STANDARD.encode(sm2_fixture(password, &scalar));

        let extracted = extract_sm2_scalar(encoded.as_bytes(), password).unwrap();

        assert_eq!(extracted.as_slice(), scalar);
    }

    #[test]
    fn rejects_wrong_sm2_password_without_exposing_it() {
        let fixture = sm2_fixture("correct-password", &[0x41u8; 32]);
        let error = extract_sm2_scalar(&fixture, "wrong-password").unwrap_err();

        assert!(error.contains("密码"));
        assert!(!error.contains("wrong-password"));
    }
}
