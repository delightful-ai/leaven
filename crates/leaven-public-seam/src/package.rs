use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    CallAuthorityError, CallAuthorityReport, ConformanceMatrix, DeferredWatchReplacement,
    EvaluationJobDocument, EvaluationRequestReceiptDocument, EvidenceEnvelopeDocument,
    OutputRecordDocument, PinnedDialectEvaluator, PlanDocument, PlanResultDocument,
    ProposalAuthorityReport, PublicSeamError, ReflectProposeHandoffDocument,
    ReflectProposeSubmissionDocument, StagePayloadDocument,
};

mod evidence_audit;
mod scope;
mod support;
mod validation;

pub use scope::{AuthorizedWorkerTransport, V1Scope, WorkerTransportKind, WorkerTransportRequest};

use support::{is_active_package_path, is_canonical_active_package, read_manifest};

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

/// Parsed executable conformance-test denominator from the active notes file.
#[derive(Clone, Debug)]
pub struct ConformanceTestDenominator {
    /// Required accept/reject cases.
    pub cases: Vec<ConformanceTestCase>,
}

/// One conformance-test denominator case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceTestCase {
    /// Stable generated id for the prose case.
    pub id: String,
    /// Whether this is an accept or reject case.
    pub kind: ConformanceTestKind,
    /// Source prose line.
    pub text: String,
}

impl ConformanceTestCase {
    /// Returns true when this case requires denial/negative proof.
    pub fn is_negative(&self) -> bool {
        self.kind == ConformanceTestKind::Reject
    }
}

/// Conformance-test case polarity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConformanceTestKind {
    /// A forbidden behavior must be rejected.
    Reject,
    /// An allowed behavior must be accepted.
    Accept,
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
        validation::inventory_for_manifest(self, &self.manifest)
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
        validation::inventory_for_manifest(self, &manifest)
    }

    /// Loads one active schema by manifest file name.
    pub fn schema_json(&self, name: &str) -> Result<Value, PublicSeamError> {
        validation::schema_json(self, name)
    }

    /// Compiles a JSON Schema value as Draft 2020-12.
    pub fn compile_schema_value(&self, name: &str, value: &Value) -> Result<(), PublicSeamError> {
        validation::compile_schema_value(self, name, value)
    }

    /// Compiles every active schema and validates the active examples.
    pub fn validate_contract_package(&self) -> Result<ValidationReport, PublicSeamError> {
        validation::validate_contract_package(self)
    }

    /// Loads the conformance matrix.
    pub fn conformance_matrix(&self) -> Result<ConformanceMatrix, PublicSeamError> {
        validation::conformance_matrix(self)
    }

    /// Loads the active conformance-test denominator from the manifest notes.
    pub fn conformance_test_denominator(
        &self,
    ) -> Result<ConformanceTestDenominator, PublicSeamError> {
        validation::conformance_test_denominator(self)
    }

    /// Checks that matrix spec references resolve in the repository.
    pub fn validate_matrix_references(
        &self,
        matrix: &ConformanceMatrix,
    ) -> Result<(), PublicSeamError> {
        evidence_audit::validate_matrix_references(self, matrix)
    }

    /// Audits proven row evidence so schema-only or topology-only closeouts cannot pass.
    pub fn audit_conformance_evidence(
        &self,
        matrix: &ConformanceMatrix,
    ) -> Result<(), PublicSeamError> {
        evidence_audit::audit_conformance_evidence(self, matrix)
    }

    /// Validates an arbitrary value against one active package schema.
    pub fn validate_arbitrary_value(
        &self,
        schema: &str,
        pointer: &str,
        value: &Value,
    ) -> Result<(), PublicSeamError> {
        validation::validate_arbitrary_value(self, schema, pointer, value)
    }

    /// Projects a reusable evidence output record through the public-seam wire shape.
    ///
    /// Blob-backed records must provide public blob identity and audit metadata.
    pub fn project_output_record(
        &self,
        record: &leaven_evidence::OutputRecord,
        blob: Option<&crate::PublicBlobRef>,
    ) -> Result<OutputRecordDocument, PublicSeamError> {
        let value = crate::output::output_record_wire_value(record, blob)?;
        self.validate_output_record_value(&value)?;
        OutputRecordDocument::from_schema_valid_value(value)
    }

    /// Validates an inline reusable evidence output record through the public-seam wire shape.
    ///
    /// Use [`Self::project_output_record`] for blob-backed records.
    pub fn validate_output_record(
        &self,
        record: &leaven_evidence::OutputRecord,
    ) -> Result<OutputRecordDocument, PublicSeamError> {
        self.project_output_record(record, None)
    }

    /// Validates an arbitrary value against `common.schema.json#/$defs/OutputRecord`.
    pub fn validate_output_record_value(
        &self,
        value: &Value,
    ) -> Result<OutputRecordDocument, PublicSeamError> {
        validation::validate_output_record_value(self, value)
    }

    /// Validates a Plan IR document through the active V1 schema and semantic seam checks.
    pub fn validate_plan_document(&self, value: &Value) -> Result<PlanDocument, PublicSeamError> {
        self.validate_arbitrary_value("leaven.plan.v1.schema.json", "/plan", value)?;
        PlanDocument::from_schema_valid_value(value)
    }

    /// Runs a representative Plan IR document through the advanced public-seam harness.
    ///
    /// This path validates the active V1 plan schema before honoring the
    /// declared execution mode, lowering typed Let/Call/Write operations when
    /// effects are allowed, delegating graph reads, non-capability-bound
    /// effects, or replay lookups to `host`, and validating the produced Plan
    /// Result through the active V1 result schema.
    ///
    /// Capability-scoped reads and externally owned execution effects such as
    /// `case_query`, `workspace_query`, `agent_run`, and `sandbox_exec` must use
    /// [`Self::execute_plan_document_with_capability`] so authority is checked
    /// before host reads, agent sessions, or sandbox commands can run.
    pub fn execute_plan_document<H: crate::PlanExecutionHost>(
        &self,
        value: &Value,
        context: &crate::PlanExecutionContext,
        host: &mut H,
    ) -> Result<crate::PlanExecutionReport, PublicSeamError> {
        let plan_document = self.validate_plan_document(value)?;
        let result = crate::plan_execution::execute_plan(value, &plan_document, context, host)?;
        let document = self.validate_plan_result_document(&result)?;
        if plan_document.mode_kind() != "replay" {
            crate::plan_execution::validate_plan_result_receipts(
                value,
                &plan_document,
                context,
                &result,
            )?;
        }
        Ok(crate::PlanExecutionReport::new(result, document))
    }

    /// Executes a Plan IR document after authorizing capability-scoped reads and effects.
    pub fn execute_plan_document_with_capability<H: crate::PlanExecutionHost>(
        &self,
        value: &Value,
        context: &crate::PlanExecutionContext,
        capability: &crate::CapabilityDocument,
        host: &mut H,
    ) -> Result<crate::PlanExecutionReport, PublicSeamError> {
        let plan_document = self.validate_plan_document(value)?;
        crate::execution_authority::validate(value, capability)?;
        let result = crate::plan_execution::execute_plan_with_capability(
            value,
            &plan_document,
            context,
            capability,
            host,
        )?;
        let document = self.validate_plan_result_document(&result)?;
        if plan_document.mode_kind() != "replay" {
            crate::plan_execution::validate_plan_result_receipts(
                value,
                &plan_document,
                context,
                &result,
            )?;
        }
        Ok(crate::PlanExecutionReport::new(result, document))
    }

    /// Validates a Plan Result against the Plan IR receipt preimages owned by the seam harness.
    pub fn validate_plan_execution_result(
        &self,
        plan: &Value,
        context: &crate::PlanExecutionContext,
        result: &Value,
    ) -> Result<PlanResultDocument, PublicSeamError> {
        let plan_document = self.validate_plan_document(plan)?;
        let result_document = self.validate_plan_result_document(result)?;
        crate::plan_execution::validate_plan_result_receipts(
            plan,
            &plan_document,
            context,
            result,
        )?;
        Ok(result_document)
    }

    /// Validates proposal writes against a capability document's effect, schema, surface, and apply authority.
    pub fn validate_proposal_authority_document(
        &self,
        plan: &Value,
        capability: &crate::CapabilityDocument,
    ) -> Result<ProposalAuthorityReport, PublicSeamError> {
        self.validate_plan_document(plan)?;
        crate::proposal_authority::validate(plan, capability)
    }

    /// Validates plan call input data classes against declared and capability-level denials.
    pub fn validate_call_authority_document(
        &self,
        plan: &Value,
        capability: &crate::CapabilityDocument,
    ) -> Result<CallAuthorityReport, CallAuthorityError> {
        self.validate_plan_document(plan)
            .map_err(CallAuthorityError::from)?;
        crate::call_authority::validate(plan, capability)
    }

    /// Validates a Leaven ACP profile document through the active V1 schema and semantic checks.
    pub fn validate_acp_profile_document(
        &self,
        value: &Value,
    ) -> Result<crate::AcpProfileDocument, PublicSeamError> {
        self.validate_arbitrary_value("leaven.acp_profile.v1.schema.json", "/acp_profile", value)?;
        crate::AcpProfileDocument::from_schema_valid_value(value)
    }

    /// Validates an ACP JSON-RPC request carrying locked Plan IR params.
    pub fn validate_acp_jsonrpc_request_document(
        &self,
        profile: &crate::AcpProfileDocument,
        value: &Value,
    ) -> Result<crate::AcpJsonRpcRequestDocument, PublicSeamError> {
        let params = value
            .get("params")
            .ok_or_else(|| PublicSeamError::InvalidScope {
                message: "ACP JSON-RPC request must carry Plan IR params".to_owned(),
            })?;
        self.validate_plan_document(params)?;
        crate::AcpJsonRpcRequestDocument::from_plan_valid_value(profile, value)
    }

    /// Validates an ACP JSON-RPC response carrying a locked extension result.
    pub fn validate_acp_jsonrpc_response_document(
        &self,
        request: &crate::AcpJsonRpcRequestDocument,
        value: &Value,
    ) -> Result<crate::AcpJsonRpcResponseDocument, PublicSeamError> {
        let result = value
            .get("result")
            .ok_or_else(|| PublicSeamError::InvalidScope {
                message: "ACP JSON-RPC response must carry extension result".to_owned(),
            })?;
        let extension = self.validate_acp_extension_result_document(result)?;
        crate::AcpJsonRpcResponseDocument::from_extension_result_value(request, &extension, value)
    }

    /// Answers an ACP permission request through programmatic capability grant checks.
    pub fn authorize_acp_permission(
        &self,
        profile: &crate::AcpProfileDocument,
        capability: &crate::CapabilityDocument,
        session: &crate::AcpAuthenticatedSession,
        request: crate::AcpPermissionRequest,
    ) -> crate::AcpPermissionDecision {
        crate::acp_profile::authorize_permission(profile, capability, session, request)
    }

    /// Resolves ACP authenticate through the public-seam capability registry.
    pub fn authenticate_acp_session(
        &self,
        profile: &crate::AcpProfileDocument,
        registry: &crate::CapabilityRegistry,
        request: crate::AcpAuthenticateRequest,
    ) -> Result<crate::AcpAuthenticatedSession, PublicSeamError> {
        crate::acp_profile::authenticate(profile, registry, request)
    }

    /// Validates a Leaven ACP extension result envelope.
    pub fn validate_acp_extension_result_document(
        &self,
        value: &Value,
    ) -> Result<crate::AcpExtensionResultDocument, PublicSeamError> {
        let synthetic = crate::AcpExtensionResultDocument::synthetic_plan_result(value)?;
        self.validate_arbitrary_value(
            "leaven.plan_result.v1.schema.json",
            "/acp_extension_result",
            &synthetic,
        )?;
        if synthetic["values"]["primary"]["kind"].as_str() != Some("extension") {
            PlanResultDocument::from_schema_valid_value_allowing_request_evaluation(&synthetic)?;
        }
        crate::AcpExtensionResultDocument::from_value(value)
    }

    /// Validates a Plan Result document through the active V1 schema and semantic seam checks.
    pub fn validate_plan_result_document(
        &self,
        value: &Value,
    ) -> Result<PlanResultDocument, PublicSeamError> {
        self.validate_arbitrary_value("leaven.plan_result.v1.schema.json", "/plan_result", value)?;
        PlanResultDocument::from_schema_valid_value(value)
    }

    /// Validates an Evidence Envelope document through the active V1 schema and semantic seam checks.
    pub fn validate_evidence_envelope_document(
        &self,
        value: &Value,
    ) -> Result<EvidenceEnvelopeDocument, PublicSeamError> {
        self.validate_arbitrary_value(
            "leaven.evidence_envelope.v1.schema.json",
            "/evidence_envelope",
            value,
        )?;
        EvidenceEnvelopeDocument::from_schema_valid_value(value)
    }

    /// Validates an Evaluation Job document through the active V1 schema and semantic seam checks.
    pub fn validate_evaluation_job_document(
        &self,
        value: &Value,
    ) -> Result<EvaluationJobDocument, PublicSeamError> {
        self.validate_arbitrary_value(
            "leaven.evaluation_job.v1.schema.json",
            "/evaluation_job",
            value,
        )?;
        EvaluationJobDocument::from_schema_valid_value(value)
    }

    /// Validates an evaluation-request receipt Plan Result against a validated Evaluation Job.
    pub fn validate_evaluation_request_receipt_document(
        &self,
        job: &EvaluationJobDocument,
        value: &Value,
    ) -> Result<EvaluationRequestReceiptDocument, PublicSeamError> {
        self.validate_arbitrary_value("leaven.plan_result.v1.schema.json", "/plan_result", value)?;
        PlanResultDocument::from_schema_valid_value_allowing_request_evaluation(value)?;
        EvaluationRequestReceiptDocument::from_plan_result(job, value)
    }

    /// Validates a stage payload through the active V1 schema and semantic seam checks.
    pub fn validate_stage_payload_document(
        &self,
        value: &Value,
    ) -> Result<StagePayloadDocument, PublicSeamError> {
        self.validate_arbitrary_value(
            "leaven.stage_payloads.v1.schema.json",
            "/stage_payload",
            value,
        )?;
        StagePayloadDocument::from_schema_valid_value(value)
    }

    /// Validates a reflect-then-propose handoff through active V1 stage schemas and semantic checks.
    pub fn validate_reflect_propose_handoff_document(
        &self,
        value: &Value,
    ) -> Result<ReflectProposeHandoffDocument, PublicSeamError> {
        let reflect = value.pointer("/reflect_request").ok_or_else(|| {
            PublicSeamError::InvalidStagePayload {
                message: "reflect/propose handoff must carry /reflect_request".to_owned(),
            }
        })?;
        let reflection = value.pointer("/reflection_result").ok_or_else(|| {
            PublicSeamError::InvalidStagePayload {
                message: "reflect/propose handoff must carry /reflection_result".to_owned(),
            }
        })?;
        let propose = value.pointer("/propose_request").ok_or_else(|| {
            PublicSeamError::InvalidStagePayload {
                message: "reflect/propose handoff must carry /propose_request".to_owned(),
            }
        })?;
        self.validate_arbitrary_value(
            "leaven.stage_payloads.v1.schema.json",
            "/reflect_request",
            reflect,
        )?;
        self.validate_arbitrary_value(
            "leaven.stage_payloads.v1.schema.json",
            "/reflection_result",
            reflection,
        )?;
        self.validate_arbitrary_value(
            "leaven.stage_payloads.v1.schema.json",
            "/propose_request",
            propose,
        )?;
        ReflectProposeHandoffDocument::from_schema_valid_values(value, reflect, reflection, propose)
    }

    /// Validates proposal writes against the exact separate reflect/propose stage handoff they cite.
    pub fn validate_reflect_propose_submission_document(
        &self,
        handoff: &Value,
        proposal_plan: &Value,
    ) -> Result<ReflectProposeSubmissionDocument, PublicSeamError> {
        let handoff_document = self.validate_reflect_propose_handoff_document(handoff)?;
        self.validate_plan_document(proposal_plan)?;
        ReflectProposeSubmissionDocument::from_valid_handoff_and_plan(
            handoff_document,
            handoff,
            proposal_plan,
        )
    }

    /// Validates the V1 deferred-watch marker and its finite-diff Plan IR replacement.
    pub fn validate_deferred_watch_replacement(
        &self,
        marker: &Value,
        plan: &Value,
    ) -> Result<DeferredWatchReplacement, PublicSeamError> {
        self.validate_arbitrary_value("leaven.watch.v1.schema.json", "/watch", marker)?;
        let plan = self
            .validate_plan_document(plan)
            .map_err(|error| match error {
                PublicSeamError::InvalidPlan { message } => {
                    PublicSeamError::InvalidWatch { message }
                }
                other => other,
            })?;
        DeferredWatchReplacement::from_plan(plan)
    }

    /// Returns the evaluator for pinned public-seam replay mini-languages.
    pub fn pinned_dialects(&self) -> PinnedDialectEvaluator {
        PinnedDialectEvaluator
    }
}
