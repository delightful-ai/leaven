use std::collections::BTreeMap;

use leaven_eval::Case;
use leaven_kernel::CaseId;
use leaven_public_seam::{
    ArtifactRecord, OptimizeObjective, OptimizeReflection, OptimizeRunRequestDocument,
    OptimizeSplit, OptimizerConfig,
};
use serde_json::Value;

use super::error::OptimizeRunHostError;
use super::problem::SeamPromptArtifact;

/// Wire artifact type the host executes today.
pub(super) const SUPPORTED_ARTIFACT_TYPE: &str = "prompt";

/// One lowered optimize-run case bound to a dense engine [`CaseId`] while
/// preserving its wire string id, input, target, and metadata.
///
/// The wire string id is what the worker stage payload carries (`case`), and
/// the target is served only through capability-gated scorer-stage callbacks;
/// the runner-stage payload never carries it.
#[derive(Clone, Debug)]
pub(super) struct LoweredCase {
    pub(super) wire_case: String,
    pub(super) input: Value,
    pub(super) target: Value,
    pub(super) metadata: Option<Value>,
}

/// Validated optimize-run request lowered into builder-ready inputs.
pub(super) struct LoweredRequest {
    pub(super) run_id: String,
    pub(super) seed: SeamPromptArtifact,
    pub(super) seed_schema: String,
    pub(super) train: Vec<Case<Value, Value>>,
    pub(super) validation: Vec<Case<Value, Value>>,
    pub(super) test: Vec<Case<Value, Value>>,
    /// Every lowered case keyed by dense engine id, for worker dispatch and
    /// scorer-stage target custody.
    pub(super) cases_by_id: BTreeMap<CaseId, LoweredCase>,
    pub(super) max_metric_calls: u64,
    pub(super) reflection_model: String,
    pub(super) capability_fingerprint: String,
}

pub(super) fn lower_request(
    document: &OptimizeRunRequestDocument,
) -> Result<LoweredRequest, OptimizeRunHostError> {
    let seed = lower_seed(document.seed())?;
    let max_metric_calls = lower_optimizer(document.optimizer())?;
    let reflection_model = lower_reflection(document.reflection())?;

    let mut train = Vec::new();
    let mut validation = Vec::new();
    let mut test = Vec::new();
    let mut cases_by_id = BTreeMap::new();

    for (index, case) in document.cases().iter().enumerate() {
        let case_id = CaseId::from_index(index);
        let split = case.split().unwrap_or(OptimizeSplit::Train);
        let lowered = LoweredCase {
            wire_case: case.case().to_owned(),
            input: case.input().clone(),
            target: case.target().clone(),
            metadata: case.metadata().cloned(),
        };
        let envelope = Case::new(case_id, lowered.input.clone(), Some(lowered.target.clone()));
        match split {
            OptimizeSplit::Train => train.push(envelope),
            OptimizeSplit::Validation => validation.push(envelope),
            OptimizeSplit::Test => test.push(envelope),
        }
        cases_by_id.insert(case_id, lowered);
    }

    Ok(LoweredRequest {
        run_id: document.run_id().to_owned(),
        seed,
        seed_schema: document.seed().artifact_schema().to_owned(),
        train,
        validation,
        test,
        cases_by_id,
        max_metric_calls,
        reflection_model,
        capability_fingerprint: document.capability_fingerprint().to_owned(),
    })
}

fn lower_seed(seed: &ArtifactRecord) -> Result<SeamPromptArtifact, OptimizeRunHostError> {
    if seed.artifact_type() != SUPPORTED_ARTIFACT_TYPE {
        return Err(OptimizeRunHostError::unsupported(format!(
            "artifact_type `{}` is not executable; the supported V1 artifact type is `{SUPPORTED_ARTIFACT_TYPE}`",
            seed.artifact_type()
        )));
    }
    let template = seed
        .artifact()
        .get("template")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            OptimizeRunHostError::lowering(
                "prompt seed artifact must carry a string `template` field",
            )
        })?;
    Ok(SeamPromptArtifact::new(template))
}

fn lower_optimizer(optimizer: &OptimizerConfig) -> Result<u64, OptimizeRunHostError> {
    match optimizer.objective() {
        OptimizeObjective::Instance => {}
        other => {
            return Err(OptimizeRunHostError::unsupported(format!(
                "optimizer objective `{}` is not executable; the supported V1 objective is `instance`",
                other.as_str()
            )));
        }
    }
    // GEPA's reference frontier is a per-case Pareto frontier and its minibatch
    // is fixed by the strategy profile; the wire's `population_size` and
    // `minibatch_size` map to no executable host knob in V1, so they are
    // refused rather than silently ignored.
    if optimizer.population_size().is_some() {
        return Err(OptimizeRunHostError::unsupported(
            "optimizer.population_size is not executable; V1 uses the fixed per-case Pareto frontier and omits population_size",
        ));
    }
    if optimizer.minibatch_size().is_some() {
        return Err(OptimizeRunHostError::unsupported(
            "optimizer.minibatch_size is not executable; V1 uses the fixed reference train minibatch and omits minibatch_size",
        ));
    }
    Ok(optimizer.max_metric_calls())
}

fn lower_reflection(reflection: &OptimizeReflection) -> Result<String, OptimizeRunHostError> {
    match reflection {
        OptimizeReflection::Lm { model } => Ok(model.clone()),
        OptimizeReflection::Agentic => Err(OptimizeRunHostError::unsupported(
            "reflection kind `agentic` is not executable; the supported V1 reflection kind is `lm`",
        )),
    }
}
