use std::path::PathBuf;

use leaven_public_seam::PublicSeamPackage;
use serde_json::Value;

pub fn package() -> PublicSeamPackage {
    PublicSeamPackage::active_from_repo(workspace_root()).unwrap()
}

pub fn workspace_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

pub fn prefixed_jcs_hash(prefix: &str, value: &Value) -> String {
    format!(
        "{prefix}{}",
        jcs_canonicalize::sha256_jcs_hex(value).unwrap()
    )
}
