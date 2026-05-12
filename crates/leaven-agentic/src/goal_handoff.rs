//! Pre-goal handoff signatures for persistent agent loops.

use leaven_agent::{AgentInstructions, AgentRunRequest, JsonSchemaRef, OutputContract};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::AgenticParseError;

const CHECK_OUTPUT_PATH: &str = ".leaven/goal/check.json";
const STAGE_OUTPUT_PATH: &str = ".leaven/goal/next-stage.json";

const GOAL_CHECK_SCHEMA: &str = r#"{
  "type": "object",
  "required": [
    "status",
    "intent_preservation",
    "satisfied_requirements",
    "missing_requirements",
    "proof_gaps",
    "misleading_proxy_proofs",
    "next_stage_hint"
  ],
  "properties": {
    "status": { "enum": ["satisfied", "not_satisfied", "blocked"] },
    "intent_preservation": { "type": "string" },
    "satisfied_requirements": { "type": "array", "items": { "type": "string" } },
    "missing_requirements": { "type": "array", "items": { "type": "string" } },
    "proof_gaps": { "type": "array", "items": { "type": "string" } },
    "misleading_proxy_proofs": { "type": "array", "items": { "type": "string" } },
    "next_stage_hint": { "type": "string" }
  }
}"#;

const STAGE_PLAN_SCHEMA: &str = r#"{
  "type": "object",
  "required": [
    "objective",
    "why_this_stage",
    "required_changes",
    "verification_commands",
    "jj_snapshot_labels",
    "stop_condition"
  ],
  "properties": {
    "objective": { "type": "string" },
    "why_this_stage": { "type": "string" },
    "required_changes": { "type": "array", "items": { "type": "string" } },
    "verification_commands": { "type": "array", "items": { "type": "string" } },
    "jj_snapshot_labels": { "type": "array", "items": { "type": "string" } },
    "stop_condition": { "type": "string" }
  }
}"#;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalHandoff {
    pub original_intent: String,
    pub designed_surface: String,
    pub intent_preservation: String,
    pub misleading_proxy_proofs: Vec<String>,
    pub spec_revisions_before_goal: Vec<String>,
    pub acceptance_path: Vec<String>,
    pub proof_denominator: String,
    pub explicit_non_goals: Vec<String>,
    pub decision: GoalHandoffDecision,
}

impl GoalHandoff {
    #[must_use]
    pub fn new(
        original_intent: impl Into<String>,
        designed_surface: impl Into<String>,
        intent_preservation: impl Into<String>,
        proof_denominator: impl Into<String>,
    ) -> Self {
        Self {
            original_intent: original_intent.into(),
            designed_surface: designed_surface.into(),
            intent_preservation: intent_preservation.into(),
            misleading_proxy_proofs: Vec::new(),
            spec_revisions_before_goal: Vec::new(),
            acceptance_path: Vec::new(),
            proof_denominator: proof_denominator.into(),
            explicit_non_goals: Vec::new(),
            decision: GoalHandoffDecision::ReadyForGoal,
        }
    }

    #[must_use]
    pub fn misleading_proxy_proof(mut self, proof: impl Into<String>) -> Self {
        self.misleading_proxy_proofs.push(proof.into());
        self
    }

    #[must_use]
    pub fn spec_revision_before_goal(mut self, revision: impl Into<String>) -> Self {
        self.spec_revisions_before_goal.push(revision.into());
        self
    }

    #[must_use]
    pub fn acceptance_step(mut self, step: impl Into<String>) -> Self {
        self.acceptance_path.push(step.into());
        self
    }

    #[must_use]
    pub fn explicit_non_goal(mut self, non_goal: impl Into<String>) -> Self {
        self.explicit_non_goals.push(non_goal.into());
        self
    }

    #[must_use]
    pub const fn decision(mut self, decision: GoalHandoffDecision) -> Self {
        self.decision = decision;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalHandoffDecision {
    ReadyForGoal,
    ReviseSpecFirst,
    RejectProxyGoal,
}

impl GoalHandoffDecision {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::ReadyForGoal => "ready_for_goal",
            Self::ReviseSpecFirst => "revise_spec_first",
            Self::RejectProxyGoal => "reject_proxy_goal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalSpecStatus {
    Satisfied,
    NotSatisfied,
    Blocked,
}

impl GoalSpecStatus {
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::NotSatisfied => "not_satisfied",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalSpecCheck {
    pub status: GoalSpecStatus,
    pub intent_preservation: String,
    pub satisfied_requirements: Vec<String>,
    pub missing_requirements: Vec<String>,
    pub proof_gaps: Vec<String>,
    pub misleading_proxy_proofs: Vec<String>,
    pub next_stage_hint: String,
}

impl GoalSpecCheck {
    #[must_use]
    pub const fn is_satisfied(&self) -> bool {
        matches!(self.status, GoalSpecStatus::Satisfied)
    }

    #[must_use]
    pub const fn is_blocked(&self) -> bool {
        matches!(self.status, GoalSpecStatus::Blocked)
    }

    #[must_use]
    pub const fn needs_next_stage(&self) -> bool {
        matches!(self.status, GoalSpecStatus::NotSatisfied)
    }

    #[must_use]
    pub fn summary(&self) -> String {
        let mut summary = String::new();
        append_value(&mut summary, "status", self.status.as_wire());
        append_value(
            &mut summary,
            "intent_preservation",
            &self.intent_preservation,
        );
        append_list(
            &mut summary,
            "satisfied_requirements",
            &self.satisfied_requirements,
        );
        append_list(
            &mut summary,
            "missing_requirements",
            &self.missing_requirements,
        );
        append_list(&mut summary, "proof_gaps", &self.proof_gaps);
        append_list(
            &mut summary,
            "misleading_proxy_proofs",
            &self.misleading_proxy_proofs,
        );
        append_value(&mut summary, "next_stage_hint", &self.next_stage_hint);
        summary
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalStagePlan {
    pub objective: String,
    pub why_this_stage: String,
    pub required_changes: Vec<String>,
    pub verification_commands: Vec<String>,
    pub jj_snapshot_labels: Vec<String>,
    pub stop_condition: String,
}

impl GoalStagePlan {
    #[must_use]
    pub fn execution_signature(&self, handoff: GoalHandoff) -> GoalExecutionSignature {
        GoalExecutionSignature::new(handoff, self.objective.clone())
            .with_stage_reason(self.why_this_stage.clone())
            .with_required_changes(self.required_changes.clone())
            .with_verification_commands(self.verification_commands.clone())
            .with_jj_snapshot_labels(self.jj_snapshot_labels.clone())
            .with_stop_condition(self.stop_condition.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalLoop {
    pub handoff: GoalHandoff,
    pub goal: String,
    pub spec_paths: Vec<String>,
    pub audit_paths: Vec<String>,
    pub latest_jj_revision: Option<String>,
    pub evaluation_summary: Option<String>,
}

impl GoalLoop {
    #[must_use]
    pub fn new(handoff: GoalHandoff, goal: impl Into<String>) -> Self {
        Self {
            handoff,
            goal: goal.into(),
            spec_paths: Vec::new(),
            audit_paths: Vec::new(),
            latest_jj_revision: None,
            evaluation_summary: None,
        }
    }

    #[must_use]
    pub fn spec_path(mut self, path: impl Into<String>) -> Self {
        self.spec_paths.push(path.into());
        self
    }

    #[must_use]
    pub fn spec_paths(mut self, paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.spec_paths.extend(collect_strings(paths));
        self
    }

    #[must_use]
    pub fn audit_path(mut self, path: impl Into<String>) -> Self {
        self.audit_paths.push(path.into());
        self
    }

    #[must_use]
    pub fn audit_paths(mut self, paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.audit_paths.extend(collect_strings(paths));
        self
    }

    #[must_use]
    pub fn latest_jj_revision(mut self, revision: impl Into<String>) -> Self {
        self.latest_jj_revision = Some(revision.into());
        self
    }

    #[must_use]
    pub fn evaluation_summary(mut self, summary: impl Into<String>) -> Self {
        self.evaluation_summary = Some(summary.into());
        self
    }

    #[must_use]
    pub fn spec_check_signature(&self) -> GoalSpecCheckSignature {
        let mut signature = GoalSpecCheckSignature::new(
            self.handoff.clone(),
            self.goal.clone(),
            self.spec_paths.clone(),
        )
        .with_audit_paths(self.audit_paths.clone());
        if let Some(revision) = &self.latest_jj_revision {
            signature = signature.with_latest_jj_revision(revision.clone());
        }
        if let Some(summary) = &self.evaluation_summary {
            signature = signature.with_evaluation_summary(summary.clone());
        }
        signature
    }

    #[must_use]
    pub fn spec_check_request(&self) -> GoalSpecCheckRequest {
        GoalSpecCheckRequest {
            signature: self.spec_check_signature(),
        }
    }

    #[must_use]
    pub fn stage_plan_signature(&self, check: &GoalSpecCheck) -> GoalStagePlanSignature {
        GoalStagePlanSignature::new(self.handoff.clone(), check.summary())
            .with_spec_paths(self.spec_paths.clone())
    }

    #[must_use]
    pub fn stage_plan_request(&self, check: &GoalSpecCheck) -> GoalStagePlanRequest {
        GoalStagePlanRequest {
            signature: self.stage_plan_signature(check),
        }
    }

    #[must_use]
    pub fn execution_request(&self, plan: &GoalStagePlan) -> GoalExecutionRequest {
        GoalExecutionRequest {
            signature: plan.execution_signature(self.handoff.clone()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalSpecCheckRequest {
    signature: GoalSpecCheckSignature,
}

impl GoalSpecCheckRequest {
    #[must_use]
    pub const fn signature(&self) -> &GoalSpecCheckSignature {
        &self.signature
    }

    #[must_use]
    pub fn instructions(&self) -> AgentInstructions {
        self.signature.instructions()
    }

    #[must_use]
    pub fn output_contract(&self) -> OutputContract {
        self.signature.output_contract()
    }

    #[must_use]
    pub fn agent_run_request(&self) -> AgentRunRequest {
        AgentRunRequest::new(self.instructions(), self.output_contract())
    }

    pub fn parse_json_bytes(&self, bytes: &[u8]) -> Result<GoalSpecCheck, AgenticParseError> {
        parse_goal_json(bytes)
    }

    pub fn parse_workspace_output(
        &self,
        workspace: &WorkspaceView<'_>,
    ) -> Result<GoalSpecCheck, AgenticParseError> {
        parse_workspace_json(workspace, CHECK_OUTPUT_PATH)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalStagePlanRequest {
    signature: GoalStagePlanSignature,
}

impl GoalStagePlanRequest {
    #[must_use]
    pub const fn signature(&self) -> &GoalStagePlanSignature {
        &self.signature
    }

    #[must_use]
    pub fn with_stage_budget(mut self, budget: impl Into<String>) -> Self {
        self.signature = self.signature.with_stage_budget(budget);
        self
    }

    #[must_use]
    pub fn instructions(&self) -> AgentInstructions {
        self.signature.instructions()
    }

    #[must_use]
    pub fn output_contract(&self) -> OutputContract {
        self.signature.output_contract()
    }

    #[must_use]
    pub fn agent_run_request(&self) -> AgentRunRequest {
        AgentRunRequest::new(self.instructions(), self.output_contract())
    }

    pub fn parse_json_bytes(&self, bytes: &[u8]) -> Result<GoalStagePlan, AgenticParseError> {
        parse_goal_json(bytes)
    }

    pub fn parse_workspace_output(
        &self,
        workspace: &WorkspaceView<'_>,
    ) -> Result<GoalStagePlan, AgenticParseError> {
        parse_workspace_json(workspace, STAGE_OUTPUT_PATH)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalExecutionRequest {
    signature: GoalExecutionSignature,
}

impl GoalExecutionRequest {
    #[must_use]
    pub const fn signature(&self) -> &GoalExecutionSignature {
        &self.signature
    }

    #[must_use]
    pub fn instructions(&self) -> AgentInstructions {
        self.signature.instructions()
    }

    #[must_use]
    pub fn output_contract(&self) -> OutputContract {
        self.signature.output_contract()
    }

    #[must_use]
    pub fn agent_run_request(&self) -> AgentRunRequest {
        AgentRunRequest::new(self.instructions(), self.output_contract())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalSpecCheckSignature {
    pub handoff: GoalHandoff,
    pub goal: String,
    pub spec_paths: Vec<String>,
    pub audit_paths: Vec<String>,
    pub latest_jj_revision: Option<String>,
    pub evaluation_summary: Option<String>,
}

impl GoalSpecCheckSignature {
    #[must_use]
    pub fn new(
        handoff: GoalHandoff,
        goal: impl Into<String>,
        spec_paths: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            handoff,
            goal: goal.into(),
            spec_paths: collect_strings(spec_paths),
            audit_paths: Vec::new(),
            latest_jj_revision: None,
            evaluation_summary: None,
        }
    }

    #[must_use]
    pub fn with_audit_paths(
        mut self,
        audit_paths: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.audit_paths = collect_strings(audit_paths);
        self
    }

    #[must_use]
    pub fn with_latest_jj_revision(mut self, revision: impl Into<String>) -> Self {
        self.latest_jj_revision = Some(revision.into());
        self
    }

    #[must_use]
    pub fn with_evaluation_summary(mut self, summary: impl Into<String>) -> Self {
        self.evaluation_summary = Some(summary.into());
        self
    }

    #[must_use]
    pub fn instructions(&self) -> AgentInstructions {
        AgentInstructions {
            system: Some(
                "Act as a Leaven pre-goal reviewer. Check the goal against the governing spec and reject proxy proofs."
                    .to_owned(),
            ),
            task: render_spec_check(self),
            context: Vec::new(),
        }
    }

    #[must_use]
    pub fn output_contract(&self) -> OutputContract {
        json_contract(
            CHECK_OUTPUT_PATH,
            "leaven_goal_spec_check_v1",
            GOAL_CHECK_SCHEMA,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalStagePlanSignature {
    pub handoff: GoalHandoff,
    pub assessment_summary: String,
    pub spec_paths: Vec<String>,
    pub stage_budget: Option<String>,
}

impl GoalStagePlanSignature {
    #[must_use]
    pub fn new(handoff: GoalHandoff, assessment_summary: impl Into<String>) -> Self {
        Self {
            handoff,
            assessment_summary: assessment_summary.into(),
            spec_paths: Vec::new(),
            stage_budget: None,
        }
    }

    #[must_use]
    pub fn with_spec_paths(
        mut self,
        spec_paths: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.spec_paths = collect_strings(spec_paths);
        self
    }

    #[must_use]
    pub fn with_stage_budget(mut self, budget: impl Into<String>) -> Self {
        self.stage_budget = Some(budget.into());
        self
    }

    #[must_use]
    pub fn instructions(&self) -> AgentInstructions {
        AgentInstructions {
            system: Some(
                "Act as a Leaven stage planner. Select the next best coherent stage and its proof path."
                    .to_owned(),
            ),
            task: render_stage_plan(self),
            context: Vec::new(),
        }
    }

    #[must_use]
    pub fn output_contract(&self) -> OutputContract {
        json_contract(
            STAGE_OUTPUT_PATH,
            "leaven_goal_stage_plan_v1",
            STAGE_PLAN_SCHEMA,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalExecutionSignature {
    pub handoff: GoalHandoff,
    pub stage_objective: String,
    pub why_this_stage: Option<String>,
    pub required_changes: Vec<String>,
    pub verification_commands: Vec<String>,
    pub jj_snapshot_labels: Vec<String>,
    pub stop_condition: Option<String>,
}

impl GoalExecutionSignature {
    #[must_use]
    pub fn new(handoff: GoalHandoff, stage_objective: impl Into<String>) -> Self {
        Self {
            handoff,
            stage_objective: stage_objective.into(),
            why_this_stage: None,
            required_changes: Vec::new(),
            verification_commands: Vec::new(),
            jj_snapshot_labels: Vec::new(),
            stop_condition: None,
        }
    }

    #[must_use]
    pub fn with_stage_reason(mut self, reason: impl Into<String>) -> Self {
        self.why_this_stage = Some(reason.into());
        self
    }

    #[must_use]
    pub fn with_required_changes(
        mut self,
        changes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.required_changes = collect_strings(changes);
        self
    }

    #[must_use]
    pub fn with_verification_commands(
        mut self,
        commands: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.verification_commands = collect_strings(commands);
        self
    }

    #[must_use]
    pub fn with_jj_snapshot_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.jj_snapshot_labels = collect_strings(labels);
        self
    }

    #[must_use]
    pub fn with_stop_condition(mut self, stop_condition: impl Into<String>) -> Self {
        self.stop_condition = Some(stop_condition.into());
        self
    }

    #[must_use]
    pub fn instructions(&self) -> AgentInstructions {
        AgentInstructions {
            system: Some(
                "Act as a persistent Codex goal executor inside Leaven. Code, snapshot, verify, and close honestly."
                    .to_owned(),
            ),
            task: render_execution(self),
            context: Vec::new(),
        }
    }

    #[must_use]
    pub fn output_contract(&self) -> OutputContract {
        OutputContract::WorkspaceDiff {
            roots: vec![WorkspacePath::root()],
        }
    }
}

fn render_spec_check(signature: &GoalSpecCheckSignature) -> String {
    let mut task = String::new();
    task.push_str("Check whether this goal is satisfied by the current workspace state.\n\n");
    task.push_str("Goal: ");
    task.push_str(&signature.goal);
    task.push_str("\n\n");
    append_handoff(&mut task, &signature.handoff);
    append_list(&mut task, "Governing spec paths", &signature.spec_paths);
    append_list(&mut task, "Audit paths", &signature.audit_paths);
    if let Some(revision) = &signature.latest_jj_revision {
        append_value(&mut task, "Latest jj revision", revision);
    }
    if let Some(summary) = &signature.evaluation_summary {
        append_value(&mut task, "Evaluation summary", summary);
    }
    task.push_str("\nReturn JSON with exactly these top-level fields: ");
    task.push_str(
        "\"status\", \"intent_preservation\", \"satisfied_requirements\", \"missing_requirements\", \"proof_gaps\", \"misleading_proxy_proofs\", \"next_stage_hint\".\n",
    );
    task.push_str(
        "Use status \"satisfied\" only when the proof denominator is actually met. Use \"not_satisfied\" when work remains. Use \"blocked\" when the next proof step cannot proceed.\n",
    );
    task
}

fn render_stage_plan(signature: &GoalStagePlanSignature) -> String {
    let mut task = String::new();
    task.push_str("Plan the next best stage for the persistent goal loop.\n\n");
    append_handoff(&mut task, &signature.handoff);
    append_value(
        &mut task,
        "Current goal/spec assessment",
        &signature.assessment_summary,
    );
    append_list(&mut task, "Governing spec paths", &signature.spec_paths);
    if let Some(budget) = &signature.stage_budget {
        append_value(&mut task, "Stage budget", budget);
    }
    task.push_str(
        "\nDo not plan a stage whose only proof is one of the misleading proxy proofs.\n",
    );
    task.push_str(
        "Return JSON with exactly these top-level fields: \"objective\", \"why_this_stage\", \"required_changes\", \"verification_commands\", \"jj_snapshot_labels\", \"stop_condition\".\n",
    );
    task
}

fn render_execution(signature: &GoalExecutionSignature) -> String {
    let mut task = String::new();
    task.push_str("Use persistent goal mode when the runtime exposes it.\n\n");
    append_value(&mut task, "Stage objective", &signature.stage_objective);
    if let Some(reason) = &signature.why_this_stage {
        append_value(&mut task, "Why this stage", reason);
    }
    append_handoff(&mut task, &signature.handoff);
    append_list(&mut task, "Required changes", &signature.required_changes);
    append_list(
        &mut task,
        "Required verification commands",
        &signature.verification_commands,
    );
    append_list(
        &mut task,
        "Required jj snapshot labels",
        &signature.jj_snapshot_labels,
    );
    if let Some(stop_condition) = &signature.stop_condition {
        append_value(&mut task, "Stop condition", stop_condition);
    }
    task.push_str(
        "\nCode until the stage objective is complete or genuinely blocked. Keep each coherent stage jj-tracked, and preserve intermediate state rather than overwriting it invisibly.\n",
    );
    task.push_str(
        "Only mark the goal complete after the proof denominator is satisfied by the requested verification path.\n",
    );
    task.push_str(
        "If blocked, stop with the blocker and the last jj snapshot instead of claiming success.\n",
    );
    task
}

fn append_handoff(task: &mut String, handoff: &GoalHandoff) {
    append_value(task, "Original intent", &handoff.original_intent);
    append_value(task, "Designed surface", &handoff.designed_surface);
    append_value(task, "Intent preservation", &handoff.intent_preservation);
    append_list(
        task,
        "Misleading proxy proofs",
        &handoff.misleading_proxy_proofs,
    );
    append_list(
        task,
        "Spec revisions before goal",
        &handoff.spec_revisions_before_goal,
    );
    append_list(task, "Acceptance path", &handoff.acceptance_path);
    append_value(task, "Proof denominator", &handoff.proof_denominator);
    append_list(task, "Explicit non-goals", &handoff.explicit_non_goals);
    append_value(task, "Decision", handoff.decision.as_wire());
}

fn append_value(task: &mut String, label: &str, value: &str) {
    task.push_str(label);
    task.push_str(": ");
    task.push_str(value);
    task.push('\n');
}

fn append_list(task: &mut String, label: &str, values: &[String]) {
    task.push_str(label);
    task.push_str(":\n");
    if values.is_empty() {
        task.push_str("- none\n");
    } else {
        for value in values {
            task.push_str("- ");
            task.push_str(value);
            task.push('\n');
        }
    }
}

fn json_contract(path: &str, name: &str, schema: &str) -> OutputContract {
    OutputContract::JsonFile {
        path: WorkspacePath::new(path).expect("goal output path is valid"),
        schema: Some(JsonSchemaRef {
            name: name.to_owned(),
            schema: schema.to_owned(),
        }),
    }
}

fn collect_strings(values: impl IntoIterator<Item = impl Into<String>>) -> Vec<String> {
    values.into_iter().map(Into::into).collect()
}

fn parse_workspace_json<T: DeserializeOwned>(
    workspace: &WorkspaceView<'_>,
    path: &str,
) -> Result<T, AgenticParseError> {
    let path = WorkspacePath::new(path).expect("goal output path is valid");
    let bytes = workspace.read_file(&path)?;
    parse_goal_json(&bytes)
}

fn parse_goal_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, AgenticParseError> {
    serde_json::from_slice(bytes)
        .map_err(|source| AgenticParseError::with_source("goal output JSON was invalid", source))
}
