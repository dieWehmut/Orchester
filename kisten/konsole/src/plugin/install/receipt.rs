use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use orchester_verzeichnis::PluginInfo;
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_MEMBER_BYTES: u64 = 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;
const STATIC_MEMBERS: [&str; 2] = ["package.json", "orchester-plugin.json"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReceiptError {
    #[error("plugin ownership receipt is invalid")]
    Invalid,
}

pub fn stage(path: &Path, package: &Path, info: &PluginInfo) -> Result<(), ReceiptError> {
    let fingerprint = fingerprint(package, info.name())?;
    let value = serde_json::json!({
        "schemaVersion": 1,
        "name": info.name(),
        "packageName": info.package_name(),
        "version": info.version(),
        "fingerprint": fingerprint,
    });
    let mut bytes = serde_json::to_vec_pretty(&value).map_err(|_| ReceiptError::Invalid)?;
    bytes.push(b'\n');
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| ReceiptError::Invalid)?;
    file.write_all(&bytes).map_err(|_| ReceiptError::Invalid)?;
    file.sync_all().map_err(|_| ReceiptError::Invalid)
}

pub fn fingerprint(package: &Path, name: &str) -> Result<String, ReceiptError> {
    let manifest = format!("manifests/{name}.toml");
    let members = [STATIC_MEMBERS[0], STATIC_MEMBERS[1], manifest.as_str()];
    let mut hasher = Sha256::new();
    for relative in members {
        let path = package.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|_| ReceiptError::Invalid)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > MAX_MEMBER_BYTES
        {
            return Err(ReceiptError::Invalid);
        }
        let mut file = fs::File::open(path).map_err(|_| ReceiptError::Invalid)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        Read::by_ref(&mut file)
            .take(MAX_MEMBER_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ReceiptError::Invalid)?;
        if bytes.len() as u64 != metadata.len() {
            return Err(ReceiptError::Invalid);
        }
        hasher.update((relative.len() as u64).to_be_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    let mut output = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(&mut output, "{byte:02x}").map_err(|_| ReceiptError::Invalid)?;
    }
    Ok(output)
}

pub fn verify(path: &Path, package: &Path, info: &PluginInfo) -> Result<(), ReceiptError> {
    let receipt = read(path, info.name())?;
    if receipt.package_name != info.package_name()
        || receipt.version != info.version()
        || receipt.fingerprint != fingerprint(package, info.name())?
    {
        return Err(ReceiptError::Invalid);
    }
    Ok(())
}

pub fn validate_stale(path: &Path, name: &str) -> Result<(), ReceiptError> {
    read(path, name).map(|_| ())
}

struct Receipt {
    package_name: String,
    version: String,
    fingerprint: String,
}

fn read(path: &Path, expected_name: &str) -> Result<Receipt, ReceiptError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ReceiptError::Invalid)?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > MAX_RECEIPT_BYTES {
        return Err(ReceiptError::Invalid);
    }
    let mut file = fs::File::open(path).map_err(|_| ReceiptError::Invalid)?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_RECEIPT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReceiptError::Invalid)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(ReceiptError::Invalid);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| ReceiptError::Invalid)?;
    let object = value.as_object().ok_or(ReceiptError::Invalid)?;
    let allowed = [
        "schemaVersion",
        "name",
        "packageName",
        "version",
        "fingerprint",
    ];
    if object.len() != allowed.len() || !object.keys().all(|key| allowed.contains(&key.as_str())) {
        return Err(ReceiptError::Invalid);
    }
    let schema = object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64);
    let name = object.get("name").and_then(serde_json::Value::as_str);
    let package_name = object
        .get("packageName")
        .and_then(serde_json::Value::as_str);
    let version = object.get("version").and_then(serde_json::Value::as_str);
    let fingerprint = object
        .get("fingerprint")
        .and_then(serde_json::Value::as_str);
    let (Some(package_name), Some(version), Some(fingerprint)) =
        (package_name, version, fingerprint)
    else {
        return Err(ReceiptError::Invalid);
    };
    let expected_package = format!("@orchester/{expected_name}");
    if schema != Some(1)
        || name != Some(expected_name)
        || package_name != expected_package.as_str()
        || version.is_empty()
        || version.len() > 64
        || fingerprint.len() != 64
        || !fingerprint
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(ReceiptError::Invalid);
    }
    Ok(Receipt {
        package_name: package_name.to_owned(),
        version: version.to_owned(),
        fingerprint: fingerprint.to_owned(),
    })
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
