//! Public-seam stage payload projection for the GEPA Git-program agentic bridge.

use std::fmt::Display;

use leaven_core::{
    CausalInputs, InfoRef, OptimizationProblem, ProposalBatch, ProposalBatchSemantics,
    ProposalEffect,
};
use leaven_gepa::{ReflectiveCase, ReflectiveSideInfoValue, ReflectiveValue};
use leaven_kernel::{CandidateId, RunId};
use serde_json::{Value, json};

use crate::GitProgramGepaReflectionInput;

/// Public-seam projection context shared by one reflect-then-propose attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProgramPublicSeamStageContext {
    run: String,
    reflect_stage_call_id: String,
    propose_stage_call_id: String,
    base_revision: String,
    surface_fingerprint: String,
    query_policy_fingerprint: String,
    capability_fingerprint: String,
    change_schema: String,
    reflection_read_receipt: String,
}

impl GitProgramPublicSeamStageContext {
    /// Creates public-seam stage context for a GEPA Git-program reflection attempt.
    #[must_use]
    pub fn new(
        run: RunId,
        base_revision: impl Into<String>,
        surface_fingerprint: impl Into<String>,
        query_policy_fingerprint: impl Into<String>,
        capability_fingerprint: impl Into<String>,
        change_schema: impl Into<String>,
    ) -> Self {
        Self {
            run: uuid_ref("run", run.as_uuid()),
            reflect_stage_call_id: "sc_gepa_git_reflect".to_owned(),
            propose_stage_call_id: "sc_gepa_git_propose".to_owned(),
            base_revision: base_revision.into(),
            surface_fingerprint: surface_fingerprint.into(),
            query_policy_fingerprint: query_policy_fingerprint.into(),
            capability_fingerprint: capability_fingerprint.into(),
            change_schema: change_schema.into(),
            reflection_read_receipt: "qrec_gepa_git_reflection_sources".to_owned(),
        }
    }

    /// Overrides stage-call ids for deterministic tests or persisted sessions.
    #[must_use]
    pub fn with_stage_call_ids(
        mut self,
        reflect_stage_call_id: impl Into<String>,
        propose_stage_call_id: impl Into<String>,
    ) -> Self {
        self.reflect_stage_call_id = reflect_stage_call_id.into();
        self.propose_stage_call_id = propose_stage_call_id.into();
        self
    }

    /// Overrides the read receipt that proves the reflection result is source-backed.
    #[must_use]
    pub fn with_reflection_read_receipt(mut self, receipt: impl Into<String>) -> Self {
        self.reflection_read_receipt = receipt.into();
        self
    }

    /// Builds the reflector-stage payload from a pre-built GEPA reflection input.
    pub fn reflect_request<Part>(&self, input: &GitProgramGepaReflectionInput<Part>) -> Value
    where
        Part: serde::Serialize,
    {
        let source_refs = info_refs(&input.source_refs, input.parent);
        json!({
            "schema_version": "leaven.stage_payloads.v1",
            "role": "reflector",
            "run": self.run,
            "stage_call_id": self.reflect_stage_call_id,
            "base_revision": self.base_revision,
            "parent": candidate_ref(input.parent),
            "part": serde_json::to_value(&input.part).unwrap_or(Value::Null),
            "part_label": input.part_label,
            "surface_fingerprint": self.surface_fingerprint,
            "examples": input.examples.iter().map(|example| reflective_example(example, &source_refs)).collect::<Vec<_>>(),
            "source_refs": source_refs,
            "attempt_index": input.attempt_index.unwrap_or(0),
            "target_safety": "target_safe_projection",
            "query_policy_fingerprint": self.query_policy_fingerprint,
            "capability_fingerprint": self.capability_fingerprint
        })
    }

    /// Builds a proposer-stage payload that consumes the exact reflection result.
    pub fn propose_request<Part>(
        &self,
        input: &GitProgramGepaReflectionInput<Part>,
        reflection_result: &Value,
    ) -> Value {
        json!({
            "schema_version": "leaven.stage_payloads.v1",
            "role": "proposer",
            "run": self.run,
            "stage_call_id": self.propose_stage_call_id,
            "base_revision": self.base_revision,
            "parent": candidate_ref(input.parent),
            "surface_fingerprint": self.surface_fingerprint,
            "reflection_result": reflection_result,
            "allowed_effects": ["change", "change_from_workspace_diff"],
            "allowed_change_schemas": [self.change_schema],
            "source_refs": info_refs(&input.source_refs, input.parent),
            "query_policy_fingerprint": self.query_policy_fingerprint,
            "capability_fingerprint": self.capability_fingerprint
        })
    }

    /// Builds the public-seam handoff and submission projection for a parsed proposal batch.
    pub fn project<Part, P>(
        &self,
        input: &GitProgramGepaReflectionInput<Part>,
        reflection_result: &Value,
        batch: &ProposalBatch<P>,
    ) -> GitProgramPublicSeamStageProjection
    where
        Part: serde::Serialize,
        P: OptimizationProblem,
    {
        let reflect_request = self.reflect_request(input);
        let propose_request = self.propose_request(input, reflection_result);
        let stage_receipts = self.stage_receipts(reflection_result);
        let handoff = json!({
            "reflect_request": reflect_request,
            "reflection_result": reflection_result,
            "propose_request": propose_request,
            "stage_receipts": stage_receipts
        });
        let submission_plan = self.proposal_submission_plan(batch);
        GitProgramPublicSeamStageProjection {
            handoff,
            submission_plan,
        }
    }

    fn stage_receipts(&self, reflection_result: &Value) -> Value {
        let fingerprint = stage_payload_fingerprint(reflection_result);
        json!([
            {
                "kind": "stage_receipt",
                "id": reflect_stage_receipt(&self.reflect_stage_call_id),
                "stage_call_id": self.reflect_stage_call_id,
                "stage_role": "reflector",
                "produces": {
                    "kind": "reflection_result",
                    "fingerprint": fingerprint
                }
            },
            {
                "kind": "stage_receipt",
                "id": propose_stage_receipt(&self.propose_stage_call_id),
                "stage_call_id": self.propose_stage_call_id,
                "stage_role": "proposer",
                "consumes": [
                    {
                        "kind": "reflection_result",
                        "fingerprint": fingerprint,
                        "receipt": reflect_stage_receipt(&self.reflect_stage_call_id)
                    }
                ]
            }
        ])
    }

    fn proposal_submission_plan<P>(&self, batch: &ProposalBatch<P>) -> Value
    where
        P: OptimizationProblem,
    {
        json!({
            "schema_version": "leaven.plan.v1",
            "plan_id": "gepa_git_stage_submission",
            "consistency": {
                "kind": "latest_at_start"
            },
            "mode": {
                "kind": "dry_run"
            },
            "ops": [
                {
                    "kind": "write",
                    "name": "proposal_batch",
                    "idempotency_key": "gepa-git-stage-submission",
                    "write": {
                        "kind": "submit_proposal_batch",
                        "semantics": batch_semantics(batch),
                        "proposals": batch.proposals.iter().map(|proposal| {
                            json!({
                                "effect": proposal_effect(&proposal.effect, &self.surface_fingerprint, &self.change_schema),
                                "causal": {
                                    "inputs": causal_inputs(proposal.provenance.causal())
                                },
                                "informed_by": {
                                    "kind": "literal",
                                    "value": informed_by_value(proposal.provenance.informed_by_refs(), &self.propose_stage_call_id, &self.reflection_read_receipt)
                                },
                                "read_receipts": [self.reflection_read_receipt]
                            })
                        }).collect::<Vec<_>>()
                    }
                }
            ],
            "return": ["proposal_batch"],
            "commit": {
                "kind": "graph_writes_atomic",
                "on_stale": "reject"
            }
        })
    }
}

/// A runtime-produced reflection result for the public seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProgramPublicSeamReflectionResult {
    value: Value,
}

impl GitProgramPublicSeamReflectionResult {
    /// Builds a source-backed reflection result for one GEPA Git-program attempt.
    #[must_use]
    pub fn source_backed(
        summary: impl Into<String>,
        source_refs: impl IntoIterator<Item = String>,
        read_receipts: impl IntoIterator<Item = String>,
        surface_fingerprint: impl Into<String>,
        part_label: impl Into<String>,
    ) -> Self {
        let surface_fingerprint = surface_fingerprint.into();
        let part_label = part_label.into();
        let source_refs = source_refs.into_iter().collect::<Vec<_>>();
        Self {
            value: json!({
                "schema_version": "leaven.stage_payloads.v1",
                "role": "reflection_result",
                "summary": summary.into(),
                "failure_modes": [
                    {
                        "label": "git_program_regression",
                        "description": "The current Git program behavior needs a targeted edit.",
                        "severity": "high",
                        "source_refs": source_refs
                    }
                ],
                "surface_suggestions": [
                    {
                        "surface_fingerprint": surface_fingerprint,
                        "part_label": part_label,
                        "diagnosis": "The selected program part should change.",
                        "suggested_direction": "Edit the selected file and preserve the surrounding repo shape.",
                        "constraints": ["do not read hidden targets"],
                        "source_refs": source_refs
                    }
                ],
                "negative_constraints": ["do not read case targets"],
                "positive_constraints": ["preserve the Git program artifact shape"],
                "source_refs": source_refs,
                "read_receipts": read_receipts.into_iter().collect::<Vec<_>>(),
                "data_classes": ["optimizer.visible"],
                "confidence": 0.82
            }),
        }
    }

    /// Returns the JSON value written by the reflection stage.
    #[must_use]
    pub fn into_value(self) -> Value {
        self.value
    }
}

/// Public-seam documents emitted by one separated reflect-then-propose attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProgramPublicSeamStageProjection {
    /// Reflect request, reflection result, propose request, and stage receipts.
    pub handoff: Value,
    /// Proposal submission plan that cites the proposer stage receipt.
    pub submission_plan: Value,
}

fn reflective_example(example: &ReflectiveCase, fallback_source_refs: &[String]) -> Value {
    let run = example.runs.first();
    let output = run
        .and_then(|run| run.produced.as_ref())
        .map(reflective_value)
        .unwrap_or(Value::Null);
    let feedback = run.map_or_else(String::new, |run| run.feedback.clone());
    let score = run.and_then(|run| run.score).unwrap_or(0.0);
    let source_refs = example.source_refs.iter().map(info_ref).collect::<Vec<_>>();
    let source_refs = if source_refs.is_empty() {
        fallback_source_refs.to_vec()
    } else {
        source_refs
    };
    json!({
        "case": example.case_id.map(case_ref).unwrap_or_else(|| "case_gepa_git_reflection".to_owned()),
        "input": reflective_value(&example.input),
        "output": output,
        "score": {
            "value": score,
            "output": {
                "kind": "text",
                "summary": reflective_value_summary(&output),
                "value": reflective_value_summary(&output),
                "visibility": "public",
                "data_classes": ["candidate.output"]
            }
        },
        "feedback": feedback,
        "side_info": reflective_side_info(run.map_or(&[][..], |run| run.side_info.as_slice())),
        "source_refs": source_refs,
        "data_classes": ["case.input", "candidate.output", "optimizer.visible"],
        "evidence_visibility": "score_and_feedback"
    })
}

fn reflective_value(value: &ReflectiveValue) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn reflective_value_summary(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "no output".to_owned(),
        other => other.to_string(),
    }
}

fn reflective_side_info(values: &[(String, ReflectiveSideInfoValue)]) -> Value {
    let entries = values
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                serde_json::to_value(value).unwrap_or(Value::Null),
            )
        })
        .collect();
    Value::Object(entries)
}

fn batch_semantics<P: OptimizationProblem>(batch: &ProposalBatch<P>) -> &'static str {
    match batch.semantics {
        ProposalBatchSemantics::Alternatives | ProposalBatchSemantics::CandidatePool => {
            "alternatives"
        }
    }
}

fn proposal_effect<P: OptimizationProblem>(
    effect: &ProposalEffect<P>,
    surface_fingerprint: &str,
    change_schema: &str,
) -> Value {
    match effect {
        ProposalEffect::Create { .. } => json!({
            "kind": "create",
            "artifact_type": "git_program",
            "artifact_schema": change_schema,
            "artifact": {
                "kind": "literal",
                "value": {"source": "typed_git_program_artifact"}
            }
        }),
        ProposalEffect::Change { target, .. } => json!({
            "kind": "change_from_workspace_diff",
            "target": candidate_ref(*target),
            "workspace": "ws_gepa_git_propose",
            "roots": ["/"],
            "parser": "leaven.git_program.workspace_diff.v1",
            "surface_fingerprint": surface_fingerprint,
            "change_schema": change_schema
        }),
    }
}

fn causal_inputs(causal: &CausalInputs) -> Vec<String> {
    causal.iter().map(candidate_ref).collect()
}

fn informed_by_value(
    refs: &[InfoRef],
    propose_stage_call_id: &str,
    reflection_read_receipt: &str,
) -> Vec<String> {
    refs.iter()
        .map(info_ref)
        .chain([
            propose_stage_receipt(propose_stage_call_id),
            reflection_read_receipt.to_owned(),
        ])
        .collect()
}

fn info_refs(refs: &[InfoRef], fallback_parent: CandidateId) -> Vec<String> {
    let refs = refs.iter().map(info_ref).collect::<Vec<_>>();
    if refs.is_empty() {
        vec![candidate_ref(fallback_parent)]
    } else {
        refs
    }
}

fn info_ref(reference: &InfoRef) -> String {
    match reference {
        InfoRef::Candidate(candidate) => candidate_ref(*candidate),
        InfoRef::Proposal(proposal) => uuid_ref("prop", proposal.as_uuid()),
        InfoRef::Assessment(assessment) => uuid_ref("assess", assessment.as_uuid()),
        InfoRef::External(external) => format!("external:{}:{}", external.kind, external.id),
    }
}

fn candidate_ref(candidate: CandidateId) -> String {
    uuid_ref("cand", candidate.as_uuid())
}

fn case_ref(case: leaven_kernel::CaseId) -> String {
    format!("case_{}", case.0)
}

fn reflect_stage_receipt(stage_call_id: &str) -> String {
    format!("stagerec_{}", stage_call_id.trim_start_matches("sc_"))
}

fn propose_stage_receipt(stage_call_id: &str) -> String {
    format!("stagerec_{}", stage_call_id.trim_start_matches("sc_"))
}

fn stage_payload_fingerprint(value: &Value) -> String {
    format!(
        "fp_stage_payload_sha256_{}",
        jcs_canonicalize::sha256_jcs_hex(value).expect("stage payload JSON is canonicalizable")
    )
}

fn uuid_ref(prefix: &str, uuid: impl Display) -> String {
    format!("{prefix}_{uuid}")
}
