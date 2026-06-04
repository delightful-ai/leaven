use serde::Deserialize;

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum CapabilitySubject {
    StageCall {
        run: String,
        stage_call_id: String,
        role: String,
    },
    EvaluationStageCall {
        run: String,
        stage_call_id: String,
        evaluation_request_id: String,
        #[serde(default)]
        evaluator: Option<String>,
    },
    Operator {
        principal: String,
    },
}
