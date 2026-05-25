use sha2::{Digest, Sha256};

use super::invalid_call;
use crate::PublicSeamError;

pub(super) fn push_unique_data_class(data_classes: &mut Vec<String>, data_class: &str) {
    if !data_classes.iter().any(|existing| existing == data_class) {
        data_classes.push(data_class.to_owned());
    }
}

pub(super) fn validate_stream_blob_ref(
    blob_ref: &serde_json::Value,
    bytes: &[u8],
    stream: &str,
) -> Result<(), PublicSeamError> {
    let object = blob_ref
        .as_object()
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must be an object")))?;
    let declared_bytes = object
        .get("bytes")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must carry bytes")))?;
    let actual_bytes = u64::try_from(bytes.len()).map_err(|_| {
        invalid_call(format!(
            "{stream} captured output is too large for public byte audit"
        ))
    })?;
    if declared_bytes != actual_bytes {
        return Err(invalid_call(format!(
            "{stream} blob ref bytes `{declared_bytes}` do not match captured output bytes `{actual_bytes}`"
        )));
    }
    let declared_sha = object
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must carry sha256")))?;
    let actual_sha = format!("{:x}", Sha256::digest(bytes));
    if declared_sha != actual_sha {
        return Err(invalid_call(format!(
            "{stream} blob ref sha256 does not match captured output"
        )));
    }
    Ok(())
}
