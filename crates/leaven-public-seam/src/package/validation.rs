use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use jsonschema::{Retrieve, Uri};
use serde_json::Value;

use super::support::{conformance_case_id, ensure_exists, read_json, read_yaml};
use super::{
    CAPABILITY_EXAMPLE, ConformanceTestCase, ConformanceTestDenominator, ConformanceTestKind,
    ContractInventory, Manifest, PublicSeamPackage, REFLECT_PROPOSE_EXAMPLE, ValidatedExample,
    ValidationReport,
};
use crate::{ConformanceMatrix, OutputRecordDocument, PublicSeamError};

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
