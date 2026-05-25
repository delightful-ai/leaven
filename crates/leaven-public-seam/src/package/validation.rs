use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use jsonschema::{Retrieve, Uri};
use serde_json::Value;

use super::support::{
    conformance_case_id, ensure_exists, evidence_is_only_known_fake_passes, looks_like_denial_test,
    read_json, read_yaml,
};
use super::{
    CAPABILITY_EXAMPLE, ConformanceTestCase, ConformanceTestDenominator, ConformanceTestKind,
    ContractInventory, Manifest, PublicSeamPackage, REFLECT_PROPOSE_EXAMPLE, ValidatedExample,
    ValidationReport,
};
use crate::{
    ConformanceMatrix, ConformanceRow, MatrixRowStatus, OutputRecordDocument, PublicSeamError,
};

pub(super) fn inventory_for_manifest(
    package: &PublicSeamPackage,
    manifest: &Manifest,
) -> Result<ContractInventory, PublicSeamError> {
    let goal_gate = package.root.join(&manifest.goal_gate);
    let matrix = package.root.join(&manifest.conformance_matrix);
    ensure_exists(&goal_gate)?;
    ensure_exists(&matrix)?;

    let mut schema_paths = Vec::new();
    let mut schemas_used_by_harness = BTreeSet::new();
    for schema in &manifest.schemas {
        let path = package.root.join("schemas").join(schema);
        ensure_exists(&path)?;
        schema_paths.push(path);
        schemas_used_by_harness.insert(schema.clone());
    }

    let mut profiles = Vec::new();
    for profile in &manifest.profiles {
        let path = package.root.join("profiles").join(profile);
        ensure_exists(&path)?;
        profiles.push(path);
    }

    Ok(ContractInventory {
        schema_paths,
        goal_gate,
        matrix,
        profiles,
        schemas_used_by_harness,
    })
}

pub(super) fn schema_json(
    package: &PublicSeamPackage,
    name: &str,
) -> Result<Value, PublicSeamError> {
    if !package.manifest.schemas.iter().any(|schema| schema == name) {
        return Err(PublicSeamError::MissingContractFile {
            path: package.root.join("schemas").join(name),
        });
    }
    read_json(package.root.join("schemas").join(name))
}

pub(super) fn compile_schema_value(
    package: &PublicSeamPackage,
    name: &str,
    value: &Value,
) -> Result<(), PublicSeamError> {
    jsonschema::draft202012::meta::validate(value).map_err(|error| {
        PublicSeamError::InvalidSchema {
            name: name.to_owned(),
            message: error.to_string(),
        }
    })?;
    jsonschema::draft202012::options()
        .with_retriever(schema_retriever(package)?)
        .build(value)
        .map_err(|error| PublicSeamError::InvalidSchema {
            name: name.to_owned(),
            message: error.to_string(),
        })?;
    Ok(())
}

pub(super) fn validate_contract_package(
    package: &PublicSeamPackage,
) -> Result<ValidationReport, PublicSeamError> {
    let inventory = package.inventory()?;
    let mut compiled_schemas = Vec::new();
    for name in &package.manifest.schemas {
        let schema = package.schema_json(name)?;
        package.compile_schema_value(name, &schema)?;
        compiled_schemas.push(name.clone());
    }

    let mut validated_examples = Vec::new();
    let capability = package.root.join("examples").join(CAPABILITY_EXAMPLE);
    let capability_value = read_json(&capability)?;
    validate_value_against_schema(
        &capability,
        "leaven.capability.v1.schema.json",
        "",
        &capability_value,
        package,
    )?;
    validated_examples.push(ValidatedExample {
        example: capability,
        schema: "leaven.capability.v1.schema.json".to_owned(),
        pointer: String::new(),
    });

    let reflect_propose = package.root.join("examples").join(REFLECT_PROPOSE_EXAMPLE);
    let reflect_value = read_json(&reflect_propose)?;
    for pointer in ["/reflect_request", "/reflection_result", "/propose_request"] {
        let value =
            reflect_value
                .pointer(pointer)
                .ok_or_else(|| PublicSeamError::ExampleValidation {
                    example: reflect_propose.clone(),
                    schema: "leaven.stage_payloads.v1.schema.json".to_owned(),
                    pointer: pointer.to_owned(),
                    message: "example pointer missing".to_owned(),
                })?;
        validate_value_against_schema(
            &reflect_propose,
            "leaven.stage_payloads.v1.schema.json",
            pointer,
            value,
            package,
        )?;
        validated_examples.push(ValidatedExample {
            example: reflect_propose.clone(),
            schema: "leaven.stage_payloads.v1.schema.json".to_owned(),
            pointer: pointer.to_owned(),
        });
    }
    package.validate_reflect_propose_handoff_document(&reflect_value)?;

    debug_assert_eq!(inventory.schema_paths.len(), compiled_schemas.len());
    Ok(ValidationReport {
        compiled_schemas,
        validated_examples,
    })
}

pub(super) fn conformance_matrix(
    package: &PublicSeamPackage,
) -> Result<ConformanceMatrix, PublicSeamError> {
    let path = package.root.join(&package.manifest.conformance_matrix);
    let matrix = read_yaml::<ConformanceMatrix>(&path)?;
    if matrix.rows.is_empty() {
        return Err(PublicSeamError::InvalidMatrix {
            message: "matrix has no rows".to_owned(),
        });
    }
    let mut ids = BTreeSet::new();
    for row in &matrix.rows {
        if !ids.insert(row.id.clone()) {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("duplicate row id `{}`", row.id),
            });
        }
    }
    Ok(matrix)
}

pub(super) fn conformance_test_denominator(
    package: &PublicSeamPackage,
) -> Result<ConformanceTestDenominator, PublicSeamError> {
    let note = package
        .manifest
        .notes
        .iter()
        .find(|note| note.ends_with("CONFORMANCE_TESTS_v0.3.md"))
        .ok_or_else(|| PublicSeamError::InvalidManifest {
            message: "manifest does not list CONFORMANCE_TESTS_v0.3.md".to_owned(),
        })?;
    let mut path = package.root.join(note);
    if !path.exists() {
        path = package.root.join("notes").join(note);
    }
    let source = fs::read_to_string(&path).map_err(|source| PublicSeamError::Io {
        path: path.clone(),
        source,
    })?;
    let mut cases = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (kind, text) = if let Some(rest) = line.strip_prefix("Reject ") {
            (ConformanceTestKind::Reject, rest)
        } else if let Some(rest) = line.strip_prefix("Accept ") {
            (ConformanceTestKind::Accept, rest)
        } else {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("unrecognized conformance test line `{line}`"),
            });
        };
        cases.push(ConformanceTestCase {
            id: conformance_case_id(kind, text),
            kind,
            text: line.to_owned(),
        });
    }
    Ok(ConformanceTestDenominator { cases })
}

pub(super) fn audit_conformance_evidence(
    package: &PublicSeamPackage,
    matrix: &ConformanceMatrix,
) -> Result<(), PublicSeamError> {
    package.validate_matrix_references(matrix)?;
    let denominator = package.conformance_test_denominator()?;
    let mut mapped_cases = BTreeSet::new();
    for row in &matrix.rows {
        for case_id in &row.conformance_tests {
            mapped_cases.insert(case_id.as_str());
        }
    }
    for case in &denominator.cases {
        if !mapped_cases.contains(case.id.as_str()) {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!(
                    "conformance test `{}` is not mapped to a matrix row",
                    case.id
                ),
            });
        }
    }
    for row in &matrix.rows {
        validate_blocked_status(row)?;
        if row.status != MatrixRowStatus::Proven {
            validate_non_proven_evidence(row)?;
            continue;
        }
        if row.implementation_evidence.is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` is proven without implementation evidence", row.id),
            });
        }
        if row.review_evidence.is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` is proven without review evidence", row.id),
            });
        }
        ensure_row_has_closeout_review(package, row)?;
        if row.fake_pass_rejected.trim().is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` does not name the fake pass it rejects", row.id),
            });
        }
        if evidence_is_only_known_fake_passes(&row.implementation_evidence) {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!(
                    "row `{}` implementation evidence is only schema/example/topology/matrix proof",
                    row.id
                ),
            });
        }
        if row.positive_test_evidence.is_empty() {
            return Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{}` lacks positive test evidence", row.id),
            });
        }
        for reference in &row.positive_test_evidence {
            ensure_test_reference(package, &row.id, reference)?;
        }
        if row.minimum_closeout_level.requires_denial_evidence() {
            if row.negative_test_evidence.is_empty() {
                return Err(PublicSeamError::InvalidMatrix {
                    message: format!("row `{}` lacks negative test evidence", row.id),
                });
            }
            for reference in &row.negative_test_evidence {
                ensure_test_reference(package, &row.id, reference)?;
            }
            for reference in &row.negative_test_evidence {
                ensure_denial_test_reference(&row.id, reference)?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_matrix_references(
    package: &PublicSeamPackage,
    matrix: &ConformanceMatrix,
) -> Result<(), PublicSeamError> {
    for row in &matrix.rows {
        for reference in &row.spec_refs {
            ensure_matrix_reference(package, &row.id, reference)?;
        }
        for reference in row
            .implementation_evidence
            .iter()
            .chain(row.partial_contract_implementation_evidence.iter())
            .chain(row.review_evidence.iter())
        {
            ensure_matrix_reference(package, &row.id, reference)?;
        }
        for reference in row
            .positive_test_evidence
            .iter()
            .chain(row.negative_test_evidence.iter())
            .chain(row.partial_contract_test_evidence.iter())
        {
            ensure_test_reference(package, &row.id, reference)?;
        }
    }
    Ok(())
}

pub(super) fn ensure_matrix_reference(
    package: &PublicSeamPackage,
    row_id: &str,
    reference: &str,
) -> Result<(), PublicSeamError> {
    let path_part = reference
        .split_once("::")
        .map_or(reference, |(path, _)| path);
    let path = package.repo_root.join(path_part);
    if path.exists() {
        Ok(())
    } else {
        Err(PublicSeamError::InvalidMatrix {
            message: format!("row `{row_id}` references missing `{reference}`"),
        })
    }
}

pub(super) fn ensure_test_reference(
    package: &PublicSeamPackage,
    row_id: &str,
    reference: &str,
) -> Result<(), PublicSeamError> {
    let (path_part, symbol) =
        reference
            .split_once("::")
            .ok_or_else(|| PublicSeamError::InvalidMatrix {
                message: format!("row `{row_id}` test evidence `{reference}` has no symbol"),
            })?;
    let path = package.repo_root.join(path_part);
    let source = fs::read_to_string(&path).map_err(|source| PublicSeamError::InvalidMatrix {
        message: format!(
            "row `{row_id}` test evidence `{reference}` could not be read at `{}`: {source}",
            path.display()
        ),
    })?;
    let symbol = symbol.rsplit("::").next().unwrap_or(symbol);
    if source.contains(&format!("fn {symbol}(")) {
        Ok(())
    } else {
        Err(PublicSeamError::InvalidMatrix {
            message: format!("row `{row_id}` test evidence `{reference}` is missing"),
        })
    }
}

pub(super) fn validate_arbitrary_value(
    package: &PublicSeamPackage,
    schema: &str,
    pointer: &str,
    value: &Value,
) -> Result<(), PublicSeamError> {
    validate_value_against_schema(&package.root.join(schema), schema, pointer, value, package)
}

pub(super) fn validate_value_against_schema(
    example: &Path,
    schema: &str,
    pointer: &str,
    value: &Value,
    package: &PublicSeamPackage,
) -> Result<(), PublicSeamError> {
    let schema_value = package.schema_json(schema)?;
    validate_value_against_schema_value(example, schema, pointer, &schema_value, value, package)
}

pub(super) fn validate_value_against_schema_value(
    example: &Path,
    schema: &str,
    pointer: &str,
    schema_value: &Value,
    value: &Value,
    package: &PublicSeamPackage,
) -> Result<(), PublicSeamError> {
    let validator = jsonschema::draft202012::options()
        .with_retriever(schema_retriever(package)?)
        .build(schema_value)
        .map_err(|error| PublicSeamError::InvalidSchema {
            name: schema.to_owned(),
            message: error.to_string(),
        })?;
    validator
        .validate(value)
        .map_err(|error| PublicSeamError::ExampleValidation {
            example: example.to_path_buf(),
            schema: schema.to_owned(),
            pointer: pointer.to_owned(),
            message: error.to_string(),
        })
}

pub(super) fn validate_output_record_value(
    package: &PublicSeamPackage,
    value: &Value,
) -> Result<OutputRecordDocument, PublicSeamError> {
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "https://schemas.leaven.dev/v1/v0.3/common.schema.json#/$defs/OutputRecord"
    });
    validate_value_against_schema_value(
        &package.root.join("schemas/common.schema.json"),
        "common.schema.json#/$defs/OutputRecord",
        "/output_record",
        &schema,
        value,
        package,
    )?;
    OutputRecordDocument::from_schema_valid_value(value.clone())
}

fn validate_blocked_status(row: &ConformanceRow) -> Result<(), PublicSeamError> {
    if row.status != MatrixRowStatus::Blocked && !row.blocked_on.is_empty() {
        return Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{}` carries blocked_on prerequisites but is not blocked",
                row.id
            ),
        });
    }
    if row.status == MatrixRowStatus::Blocked
        && (row.blocked_on.is_empty()
            || row
                .blocked_on
                .iter()
                .any(|prerequisite| prerequisite.trim().is_empty()))
    {
        return Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{}` is blocked without concrete blocked_on prerequisites",
                row.id
            ),
        });
    }
    Ok(())
}

fn validate_non_proven_evidence(row: &ConformanceRow) -> Result<(), PublicSeamError> {
    if !row.positive_test_evidence.is_empty()
        || !row.negative_test_evidence.is_empty()
        || !row.implementation_evidence.is_empty()
    {
        return Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{}` is not proven but uses closeout evidence fields instead of partial_contract evidence",
                row.id
            ),
        });
    }
    Ok(())
}

fn ensure_row_has_closeout_review(
    package: &PublicSeamPackage,
    row: &ConformanceRow,
) -> Result<(), PublicSeamError> {
    for reference in &row.review_evidence {
        let path_part = reference
            .split_once("::")
            .map_or(reference.as_str(), |(path, _)| path);
        let path = package.repo_root.join(path_part);
        let source =
            fs::read_to_string(&path).map_err(|source| PublicSeamError::InvalidMatrix {
                message: format!(
                    "row `{}` review evidence `{reference}` could not be read at `{}`: {source}",
                    row.id,
                    path.display()
                ),
            })?;
        if review_text_signs_off_row(&source, &row.id) {
            return Ok(());
        }
    }
    Err(PublicSeamError::InvalidMatrix {
        message: format!(
            "row `{}` is proven without row-specific adversarial sign-off review evidence",
            row.id
        ),
    })
}

fn review_text_signs_off_row(source: &str, row_id: &str) -> bool {
    if !source.contains(row_id) {
        return false;
    }
    for line in source.lines() {
        if review_text_block_signs_off_row(line, row_id) {
            return true;
        }
    }
    if signed_off_rows_section_contains_row(source, row_id) {
        return true;
    }
    if explicit_signoff_section_contains_row(source, row_id) {
        return true;
    }
    if review_text_has_single_row_scope(source, row_id)
        && !review_text_has_row_specific_closeout_rejection(source, row_id)
    {
        return source.lines().any(|line| {
            review_text_has_document_verdict(line) && {
                let normalized = line.to_ascii_lowercase();
                !review_text_block_rejects_closeout(&normalized)
            }
        });
    }
    false
}

fn review_text_has_row_specific_closeout_rejection(source: &str, row_id: &str) -> bool {
    source.lines().any(|line| {
        line.contains(row_id) && review_text_block_rejects_closeout(&line.to_ascii_lowercase())
    })
}

fn review_text_block_signs_off_row(block: &str, row_id: &str) -> bool {
    if !block.contains(row_id) {
        return false;
    }
    let normalized = block.to_ascii_lowercase();
    let row = format!("`{}`", row_id).to_ascii_lowercase();
    let has_signoff = normalized.contains(&format!("{row}: signed off"))
        || normalized.contains(&format!("{row}: sign off"))
        || normalized.contains(&format!("{row}: sign-off"))
        || normalized.contains(&format!("{row}: signoff"))
        || normalized.contains(&format!("{row}: sign off"))
        || normalized.contains(&format!("{row}: sign-off"))
        || normalized.contains(&format!("{row}: sign off"))
        || normalized.contains(&format!("{row}: may be promoted"))
        || normalized.contains(&format!("{row}: can be promoted"))
        || normalized.contains(&format!("{row}: can move to proven"))
        || normalized.contains(&format!("{row}: can move to `proven`"))
        || normalized.contains(&format!("{row}: can be marked proven"))
        || normalized.contains(&format!("{row} may be marked proven"))
        || normalized.contains(&format!("{row} may move to proven"))
        || normalized.contains(&format!("{row}: proven"))
        || normalized.contains(&format!("sign off on promoting {row}"))
        || normalized.contains(&format!("signed off for {row}"))
        || normalized.contains(&format!("sign-off for {row}"))
        || normalized.contains(&format!("promote {row} to `proven`"))
        || normalized.contains(&format!("promote {row} to proven"))
        || normalized.contains(&format!("no blocking findings remain for {row}"))
        || normalized.contains(&format!("{row} has no blocking finding"));
    if !has_signoff {
        return false;
    }
    !review_text_block_rejects_closeout(&normalized)
}

fn review_text_block_rejects_closeout(normalized: &str) -> bool {
    [
        "partial evidence only",
        "partial pending-row evidence",
        "pending-row evidence only",
        "row remains pending",
        "must remain pending",
        "remain pending",
        "remains pending",
        "remain `pending`",
        "remains `pending`",
        "not full row closeout",
        "not full",
        "not row closeout",
        "not proven",
        "do not mark",
        "not allowed",
        "not as sign-off",
        "no row was promoted",
        "pending after review",
        "still pending",
        "required before",
        "still required",
        "before the row can move",
        "does not sign off",
        "does not promote",
        "do not promote",
        "blocks row promotion",
        "block row promotion",
        "not promote",
        "no sign-off",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase))
}

fn signed_off_rows_section_contains_row(source: &str, row_id: &str) -> bool {
    let mut in_signed_off_rows = false;
    for line in source.lines() {
        let normalized = line.to_ascii_lowercase();
        if normalized.starts_with("## ") || normalized.starts_with("# ") {
            in_signed_off_rows = normalized.contains("signed off rows");
            continue;
        }
        if in_signed_off_rows && normalized.starts_with("## ") {
            in_signed_off_rows = false;
        }
        if in_signed_off_rows
            && line.contains(row_id)
            && !review_text_block_rejects_closeout(&normalized)
        {
            return true;
        }
    }
    false
}

fn explicit_signoff_section_contains_row(source: &str, row_id: &str) -> bool {
    let mut in_signoff_section = false;
    for line in source.lines() {
        let normalized = line.to_ascii_lowercase();
        let starts_section = normalized.contains("reviewer sign-off")
            || normalized.contains("sign-off:")
            || normalized.contains("sign off:")
            || normalized.contains("final verdict")
            || normalized.contains("final decision");
        if starts_section && !review_text_block_rejects_closeout(&normalized) {
            in_signoff_section = true;
            continue;
        }
        if in_signoff_section && normalized.starts_with("## ") {
            in_signoff_section = false;
        }
        if in_signoff_section
            && line.contains(row_id)
            && line.trim_start().starts_with("- `")
            && !review_text_block_rejects_closeout(&normalized)
        {
            return true;
        }
    }
    false
}

fn review_text_has_single_row_scope(source: &str, row_id: &str) -> bool {
    let first_lines = source.lines().take(20).collect::<Vec<_>>().join("\n");
    first_lines.contains(row_id)
}

fn review_text_has_document_verdict(source: &str) -> bool {
    let normalized = source.to_ascii_lowercase();
    normalized.contains("verdict: sign off")
        || normalized.contains("verdict: sign-off")
        || normalized.contains("may be promoted")
        || normalized.contains("can move to `proven`")
        || normalized.contains("can move to proven")
        || normalized.contains("final verdict")
        || normalized.contains("final decision")
}

fn ensure_denial_test_reference(row_id: &str, reference: &str) -> Result<(), PublicSeamError> {
    let (_, symbol) = reference
        .split_once("::")
        .ok_or_else(|| PublicSeamError::InvalidMatrix {
            message: format!("row `{row_id}` test evidence `{reference}` has no symbol"),
        })?;
    let symbol = symbol.rsplit("::").next().unwrap_or(symbol);
    if looks_like_denial_test(symbol) {
        Ok(())
    } else {
        Err(PublicSeamError::InvalidMatrix {
            message: format!(
                "row `{row_id}` negative test evidence `{reference}` does not look like denial evidence"
            ),
        })
    }
}

fn schema_retriever(package: &PublicSeamPackage) -> Result<SchemaRetriever, PublicSeamError> {
    let mut schemas = HashMap::new();
    for name in &package.manifest.schemas {
        let value = package.schema_json(name)?;
        schemas.insert(name.clone(), value.clone());
        if let Some(id) = value.get("$id").and_then(Value::as_str) {
            schemas.insert(id.to_owned(), value);
        }
    }
    Ok(SchemaRetriever { schemas })
}

#[derive(Clone, Debug)]
struct SchemaRetriever {
    schemas: HashMap<String, Value>,
}

impl Retrieve for SchemaRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.schemas
            .get(uri.as_str())
            .cloned()
            .ok_or_else(|| format!("schema not found: {uri}").into())
    }
}
