use serde_json::Value;

use crate::PublicSeamError;

/// RFC 8785 JCS plus SHA-256 schema fingerprint in public seam wire form.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SchemaFingerprint(String);

impl SchemaFingerprint {
    /// Computes `fp_schema_sha256_<hex>` from a JSON value's JCS canonical bytes.
    ///
    /// This is intentionally separate from `leaven-kernel::Fingerprint`, which
    /// is a BLAKE3 behavior/cache primitive.
    pub fn for_json_value(value: &Value) -> Result<Self, PublicSeamError> {
        let digest = jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
            PublicSeamError::Fingerprint {
                message: error.to_string(),
            }
        })?;
        Ok(Self(format!("fp_schema_sha256_{digest}")))
    }

    /// Returns the wire fingerprint string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
