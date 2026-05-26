use std::path::PathBuf;

use leaven_public_seam::PublicSeamPackage;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

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

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn plan_call_result_hash(name: &str, value: Value) -> String {
    prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_result.v1",
            "name": name,
            "value": value
        }),
    )
}

pub fn bind_plan_result_hashes(mut result: Value) -> Value {
    let values = result["values"].as_object().unwrap().clone();
    for receipt in result["receipts"].as_array_mut().unwrap() {
        let receipt_id = receipt["receipt"].as_str().unwrap();
        let Some((name, value)) = values.iter().find(|(_, value)| {
            value
                .as_object()
                .and_then(|object| object.get("receipt"))
                .and_then(Value::as_str)
                == Some(receipt_id)
        }) else {
            continue;
        };
        let schema_version = match receipt["kind"].as_str().unwrap() {
            "query" => "leaven.plan_query_result.v1",
            "call" => "leaven.plan_call_result.v1",
            "write" => "leaven.plan_write_result.v1",
            other => panic!("unexpected receipt kind {other}"),
        };
        let op_name = receipt["op_var"].as_str().unwrap_or(name);
        receipt["result_hash"] = json!(prefixed_jcs_hash(
            "fp_result_sha256_",
            &json!({
                "schema_version": schema_version,
                "name": op_name,
                "value": value
            }),
        ));
    }
    result
}

pub fn submit_assessments_request_hash(
    evaluation_request_id: Value,
    assessment_ids: Value,
) -> String {
    prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.submit_assessments_request.v1",
            "evaluation_request_id": evaluation_request_id,
            "assessment_ids": assessment_ids
        }),
    )
}
