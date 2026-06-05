use serde_json::Value;

mod assessment;
mod dialect_usage;
mod expression;
mod model;
mod parse;

#[allow(unused_imports)]
pub use expression::PlanExtensionPayload;
#[allow(unused_imports)]
pub use expression::PlanLiteralValue;
pub use expression::{
    PlanArtifactProjectionSelector, PlanCostScope, PlanExpression, PlanExpressionKind,
    PlanGraphEventFilter, PlanGraphEventFilterPayload, PlanGraphQuerySource,
};
pub use model::{
    PlanApplyProposalBatchWrite, PlanCallKind, PlanCommitKind, PlanEmitRunEventWrite,
    PlanEvaluationSetExpr, PlanEvaluationShape, PlanEventPayload, PlanId, PlanMode, PlanOperation,
    PlanOperationKind, PlanProposalCausalInputs, PlanQueryKind, PlanRequestEvaluationWrite,
    PlanReturnBinding, PlanSchemaVersion, PlanSubmitAssessmentsWrite, PlanSubmitProposalBatchWrite,
    PlanWriteKind,
};

use assessment::AssessmentScoreOutputUsage;
pub use assessment::{
    PlanAssessmentPreferenceValue, PlanAssessmentRankingValue, PlanAssessmentTargetValue,
    PlanScoreOutputValue,
};
use dialect_usage::DialectUsage;
use model::PlanOperationDetail;
use parse::{invalid_plan, nested_kind, required_object_string, string_array};

use crate::PublicSeamError;

/// Schema-valid public-seam Plan IR document classified by core operation family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanDocument {
    plan_id: PlanId,
    schema_version: PlanSchemaVersion,
    operation_kinds: Vec<PlanOperationKind>,
    operations: Vec<PlanOperation>,
    return_names: Vec<String>,
    return_bindings: Vec<PlanReturnBinding>,
    consistency_kind: String,
    at_revision: Option<String>,
    since_revision: Option<String>,
    until_revision: Option<String>,
    events_since_revision_queries: usize,
    pinned_pointer_count: usize,
    pinned_jsonpath_count: usize,
    strict_template_count: usize,
    assessment_score_outputs: AssessmentScoreOutputUsage,
    mode: PlanMode,
    commit: PlanCommitKind,
}

impl PlanDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_plan("plan must be an object"))?;
        let ops = object
            .get("ops")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_plan("plan ops must be an array"))?;
        let plan_id = PlanId::parse(required_object_string(object, "plan_id")?)?;
        let schema_version =
            PlanSchemaVersion::parse(required_object_string(object, "schema_version")?)?;
        let mut operation_kinds = Vec::with_capacity(ops.len());
        let mut operations = Vec::with_capacity(ops.len());
        let consistency = object
            .get("consistency")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_plan("plan `consistency` must carry a kind"))?;
        let consistency_kind = nested_kind(object.get("consistency"), "consistency")?.to_owned();
        let at_revision = consistency
            .get("revision")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let since_revision = consistency
            .get("since")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let until_revision = consistency
            .get("until")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let mut events_since_revision_queries = 0;
        let mut dialect_usage = DialectUsage::default();
        let mut assessment_score_outputs = AssessmentScoreOutputUsage::default();
        for op in ops {
            dialect_usage.inspect_value(op)?;
            let op_object = op
                .as_object()
                .ok_or_else(|| invalid_plan("plan op must be an object"))?;
            let operation = PlanOperation::from_schema_valid_object(op_object)?;
            let operation_kind = operation.kind;
            match &operation.detail {
                PlanOperationDetail::Let { expression } => {
                    expression.validate_event_sources(
                        &consistency_kind,
                        since_revision.as_deref(),
                        until_revision.as_deref(),
                    )?;
                    events_since_revision_queries += expression
                        .event_query_count(since_revision.as_deref(), until_revision.as_deref());
                }
                PlanOperationDetail::Call { .. } => {}
                PlanOperationDetail::Write { write } => {
                    assessment_score_outputs.merge(&write.submit_assessments);
                }
            }
            operation_kinds.push(operation_kind);
            operations.push(operation);
        }
        let return_names = string_array(object.get("return"), "return")?;

        Ok(Self {
            plan_id,
            schema_version,
            operation_kinds,
            operations,
            return_bindings: return_names
                .iter()
                .map(|name| PlanReturnBinding::new(name.clone()))
                .collect(),
            return_names,
            consistency_kind,
            at_revision,
            since_revision,
            until_revision,
            events_since_revision_queries,
            pinned_pointer_count: dialect_usage.pointers,
            pinned_jsonpath_count: dialect_usage.jsonpaths,
            strict_template_count: dialect_usage.templates,
            assessment_score_outputs,
            mode: PlanMode::parse(nested_kind(object.get("mode"), "mode")?)?,
            commit: PlanCommitKind::parse(nested_kind(object.get("commit"), "commit")?)?,
        })
    }
}
