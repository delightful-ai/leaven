use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use jsonschema::{Retrieve, Uri};
use serde::Deserialize;
use serde_json::Value;

use crate::{ConformanceMatrix, PublicSeamError};

const ACTIVE_PACKAGE_RELATIVE: &str = "docs/specs/public-seam-v1";
const CAPABILITY_EXAMPLE: &str = "evaluator_capability.v0.3.example.json";
const REFLECT_PROPOSE_EXAMPLE: &str = "reflect_then_propose.example.json";

/// Manifest for the locked active public seam package.
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Lock status.
    pub status: String,
    /// Goal gate file.
    pub goal_gate: String,
    /// Conformance matrix file.
    pub conformance_matrix: String,
    /// Active schema file names.
    pub schemas: Vec<String>,
    /// Active profile paths.
    pub profiles: Vec<String>,
    /// Watch V1 status.
    pub watch_status: String,
    /// Legacy worker protocol status.
    pub worker_protocol_status: String,
    /// MCP status.
    pub mcp_status: String,
    /// Locked decisions carried by the manifest.
    pub key_decisions: Vec<String>,
    /// Notes listed by the manifest.
    pub notes: Vec<String>,
}

/// Loaded active public seam package.
#[derive(Clone, Debug)]
pub struct PublicSeamPackage {
    root: PathBuf,
    repo_root: PathBuf,
    manifest: Manifest,
}

/// Contract file inventory derived from the manifest.
#[derive(Clone, Debug)]
pub struct ContractInventory {
    /// Active schema paths.
    pub schema_paths: Vec<PathBuf>,
    /// Goal gate path.
    pub goal_gate: PathBuf,
    /// Conformance matrix path.
    pub matrix: PathBuf,
    /// Profile paths.
    pub profiles: Vec<PathBuf>,
    /// Schema file names included in the harness denominator.
    pub schemas_used_by_harness: BTreeSet<String>,
}

/// Contract package validation report.
#[derive(Clone, Debug)]
pub struct ValidationReport {
    /// Schema file names that compiled.
    pub compiled_schemas: Vec<String>,
    /// Examples and nested example values validated against active schemas.
    pub validated_examples: Vec<ValidatedExample>,
}

/// One validated example value.
#[derive(Clone, Debug)]
pub struct ValidatedExample {
    /// Example file path.
    pub example: PathBuf,
    /// Schema file name used for validation.
    pub schema: String,
    /// JSON pointer within the example file.
    pub pointer: String,
}

/// Locked V1 runtime scope implied by manifest markers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct V1Scope {
    /// Whether MCP-over-ACP is enabled in V1.
    pub mcp_over_acp_enabled: bool,
    /// Whether `watch.v1` runtime behavior is enabled in V1.
    pub watch_runtime_enabled: bool,
    /// Whether deprecated `worker_protocol.v1` runtime behavior is enabled.
    pub legacy_worker_protocol_enabled: bool,
    /// Worker transport selected by V1.
    pub worker_transport: &'static str,
}

impl PublicSeamPackage {
    /// Loads the active package from a repository root.
    pub fn active_from_repo(root: impl AsRef<Path>) -> Result<Self, PublicSeamError> {
        Self::from_path(root.as_ref().join(ACTIVE_PACKAGE_RELATIVE))
    }

    /// Loads a package path, refusing anything other than the active V1 package.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PublicSeamError> {
        let root = path.as_ref().to_path_buf();
        if !is_active_package_path(&root) || !is_canonical_active_package(&root) {
            return Err(PublicSeamError::InactivePackage { path: root });
        }
        let manifest = read_manifest(&root.join("manifest.json"))?;
        if manifest.name != "leaven-public-seam-v1" || manifest.status != "locked" {
            return Err(PublicSeamError::InactivePackage { path: root });
        }
        let repo_root = root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or_else(|| PublicSeamError::InactivePackage { path: root.clone() })?
            .to_path_buf();
        Ok(Self {
            root,
            repo_root,
            manifest,
        })
    }

    /// Active package root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Active manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Builds and checks the active contract inventory from the manifest.
    pub fn inventory(&self) -> Result<ContractInventory, PublicSeamError> {
        self.inventory_for_manifest(&self.manifest)
    }

    /// Builds inventory using an override manifest value, for negative tests.
    pub fn inventory_with_manifest_override(
        &self,
        manifest: Value,
    ) -> Result<ContractInventory, PublicSeamError> {
        let manifest = serde_json::from_value::<Manifest>(manifest).map_err(|error| {
            PublicSeamError::InvalidManifest {
                message: error.to_string(),
            }
        })?;
        self.inventory_for_manifest(&manifest)
    }

    /// Loads one active schema by manifest file name.
    pub fn schema_json(&self, name: &str) -> Result<Value, PublicSeamError> {
        if !self.manifest.schemas.iter().any(|schema| schema == name) {
            return Err(PublicSeamError::MissingContractFile {
                path: self.root.join("schemas").join(name),
            });
        }
        read_json(self.root.join("schemas").join(name))
    }

    /// Compiles a JSON Schema value as Draft 2020-12.
    pub fn compile_schema_value(&self, name: &str, value: &Value) -> Result<(), PublicSeamError> {
        jsonschema::draft202012::meta::validate(value).map_err(|error| {
            PublicSeamError::InvalidSchema {
                name: name.to_owned(),
                message: error.to_string(),
            }
        })?;
        jsonschema::draft202012::options()
            .with_retriever(self.schema_retriever()?)
            .build(value)
            .map_err(|error| PublicSeamError::InvalidSchema {
                name: name.to_owned(),
                message: error.to_string(),
            })?;
        Ok(())
    }

    /// Compiles every active schema and validates the active examples.
    pub fn validate_contract_package(&self) -> Result<ValidationReport, PublicSeamError> {
        let inventory = self.inventory()?;
        let mut compiled_schemas = Vec::new();
        for name in &self.manifest.schemas {
            let schema = self.schema_json(name)?;
            self.compile_schema_value(name, &schema)?;
            compiled_schemas.push(name.clone());
        }

        let mut validated_examples = Vec::new();
        let capability = self.root.join("examples").join(CAPABILITY_EXAMPLE);
        let capability_value = read_json(&capability)?;
        self.validate_value_against_schema(
            &capability,
            "leaven.capability.v1.schema.json",
            "",
            &capability_value,
        )?;
        validated_examples.push(ValidatedExample {
            example: capability,
            schema: "leaven.capability.v1.schema.json".to_owned(),
            pointer: String::new(),
        });

        let reflect_propose = self.root.join("examples").join(REFLECT_PROPOSE_EXAMPLE);
        let reflect_value = read_json(&reflect_propose)?;
        for pointer in ["/reflect_request", "/reflection_result", "/propose_request"] {
            let value = reflect_value.pointer(pointer).ok_or_else(|| {
                PublicSeamError::ExampleValidation {
                    example: reflect_propose.clone(),
                    schema: "leaven.stage_payloads.v1.schema.json".to_owned(),
                    pointer: pointer.to_owned(),
                    message: "example pointer missing".to_owned(),
                }
            })?;
            self.validate_value_against_schema(
                &reflect_propose,
                "leaven.stage_payloads.v1.schema.json",
                pointer,
                value,
            )?;
            validated_examples.push(ValidatedExample {
                example: reflect_propose.clone(),
                schema: "leaven.stage_payloads.v1.schema.json".to_owned(),
                pointer: pointer.to_owned(),
            });
        }

        debug_assert_eq!(inventory.schema_paths.len(), compiled_schemas.len());
        Ok(ValidationReport {
            compiled_schemas,
            validated_examples,
        })
    }

    /// Loads the conformance matrix.
    pub fn conformance_matrix(&self) -> Result<ConformanceMatrix, PublicSeamError> {
        let path = self.root.join(&self.manifest.conformance_matrix);
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

    /// Checks that matrix spec references resolve in the repository.
    pub fn validate_matrix_references(
        &self,
        matrix: &ConformanceMatrix,
    ) -> Result<(), PublicSeamError> {
        for row in &matrix.rows {
            for reference in &row.spec_refs {
                self.ensure_matrix_reference(&row.id, reference)?;
            }
            for reference in row
                .implementation_evidence
                .iter()
                .chain(row.review_evidence.iter())
            {
                self.ensure_matrix_reference(&row.id, reference)?;
            }
        }
        Ok(())
    }

    fn ensure_matrix_reference(
        &self,
        row_id: &str,
        reference: &str,
    ) -> Result<(), PublicSeamError> {
        let path_part = reference
            .split_once("::")
            .map_or(reference, |(path, _)| path);
        let path = self.repo_root.join(path_part);
        if path.exists() {
            Ok(())
        } else {
            Err(PublicSeamError::InvalidMatrix {
                message: format!("row `{row_id}` references missing `{reference}`"),
            })
        }
    }

    /// Returns the locked V1 scope, refusing manifest drift.
    pub fn v1_scope(&self) -> Result<V1Scope, PublicSeamError> {
        if self.manifest.mcp_status != "not_in_v1" {
            return Err(PublicSeamError::InvalidScope {
                message: "manifest.mcp_status must remain not_in_v1".to_owned(),
            });
        }
        if self.manifest.watch_status != "deferred_to_v1.1" {
            return Err(PublicSeamError::InvalidScope {
                message: "manifest.watch_status must remain deferred_to_v1.1".to_owned(),
            });
        }
        if self.manifest.worker_protocol_status != "deprecated_replaced_by_acp_profile" {
            return Err(PublicSeamError::InvalidScope {
                message: "manifest.worker_protocol_status must remain deprecated".to_owned(),
            });
        }
        Ok(V1Scope {
            mcp_over_acp_enabled: false,
            watch_runtime_enabled: false,
            legacy_worker_protocol_enabled: false,
            worker_transport: "acp_profile",
        })
    }

    fn inventory_for_manifest(
        &self,
        manifest: &Manifest,
    ) -> Result<ContractInventory, PublicSeamError> {
        let goal_gate = self.root.join(&manifest.goal_gate);
        let matrix = self.root.join(&manifest.conformance_matrix);
        ensure_exists(&goal_gate)?;
        ensure_exists(&matrix)?;

        let mut schema_paths = Vec::new();
        let mut schemas_used_by_harness = BTreeSet::new();
        for schema in &manifest.schemas {
            let path = self.root.join("schemas").join(schema);
            ensure_exists(&path)?;
            schema_paths.push(path);
            schemas_used_by_harness.insert(schema.clone());
        }

        let mut profiles = Vec::new();
        for profile in &manifest.profiles {
            let path = self.root.join("profiles").join(profile);
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

    fn schema_retriever(&self) -> Result<SchemaRetriever, PublicSeamError> {
        let mut schemas = HashMap::new();
        for name in &self.manifest.schemas {
            let value = self.schema_json(name)?;
            schemas.insert(name.clone(), value.clone());
            if let Some(id) = value.get("$id").and_then(Value::as_str) {
                schemas.insert(id.to_owned(), value);
            }
        }
        Ok(SchemaRetriever { schemas })
    }

    fn validate_value_against_schema(
        &self,
        example: &Path,
        schema: &str,
        pointer: &str,
        value: &Value,
    ) -> Result<(), PublicSeamError> {
        let schema_value = self.schema_json(schema)?;
        let validator = jsonschema::draft202012::options()
            .with_retriever(self.schema_retriever()?)
            .build(&schema_value)
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

fn is_active_package_path(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("public-seam-v1")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("specs")
        && path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("docs")
}

fn is_canonical_active_package(path: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    let Ok(active) = source_repo_root()
        .join(ACTIVE_PACKAGE_RELATIVE)
        .canonicalize()
    else {
        return false;
    };
    path == active
}

fn source_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("leaven-public-seam lives under workspace/crates")
        .to_path_buf()
}

fn read_manifest(path: &Path) -> Result<Manifest, PublicSeamError> {
    read_json_value(path).and_then(|value| {
        serde_json::from_value(value).map_err(|error| PublicSeamError::InvalidManifest {
            message: error.to_string(),
        })
    })
}

fn read_json(path: impl AsRef<Path>) -> Result<Value, PublicSeamError> {
    read_json_value(path.as_ref())
}

fn read_json_value(path: &Path) -> Result<Value, PublicSeamError> {
    let text = read_to_string(path)?;
    serde_json::from_str(&text).map_err(|source| PublicSeamError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_yaml<T>(path: &Path) -> Result<T, PublicSeamError>
where
    T: for<'de> Deserialize<'de>,
{
    let text = read_to_string(path)?;
    serde_yml::from_str(&text).map_err(|source| PublicSeamError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

fn read_to_string(path: &Path) -> Result<String, PublicSeamError> {
    fs::read_to_string(path).map_err(|source| PublicSeamError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_exists(path: &Path) -> Result<(), PublicSeamError> {
    if path.exists() {
        Ok(())
    } else {
        Err(PublicSeamError::MissingContractFile {
            path: path.to_path_buf(),
        })
    }
}
