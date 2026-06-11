use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use leaven_eval::Case;
use leaven_kernel::CaseId;
use leaven_public_seam::{
    ArtifactPayload, ArtifactRecord, OptimizeObjective, OptimizeReflection,
    OptimizeRunRequestDocument, OptimizeSplit, OptimizerConfig,
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
    /// Candidate-pool cap: caps the seed plus loop-authored children. `None`
    /// leaves the GEPA loop unbounded (reference behavior).
    pub(super) max_candidates: Option<NonZeroUsize>,
    /// Train screening minibatch override. `None` keeps the profile-fixed
    /// reference minibatch.
    pub(super) train_minibatch_size: Option<NonZeroUsize>,
    /// USD cost ceiling in micro-dollars. `None` leaves the run capped only by
    /// `max_metric_calls` (no usd ceiling).
    pub(super) max_cost_usd_micro: Option<u64>,
    pub(super) reflection_model: String,
    pub(super) capability_fingerprint: String,
}

pub(super) fn lower_request(
    document: &OptimizeRunRequestDocument,
) -> Result<LoweredRequest, OptimizeRunHostError> {
    let seed = lower_seed(document.seed())?;
    let optimizer = lower_optimizer(document.optimizer())?;
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
        max_metric_calls: optimizer.max_metric_calls,
        max_candidates: optimizer.max_candidates,
        train_minibatch_size: optimizer.train_minibatch_size,
        max_cost_usd_micro: optimizer.max_cost_usd_micro,
        reflection_model,
        capability_fingerprint: document.capability_fingerprint().to_owned(),
    })
}

/// Optimizer knobs lowered from the locked request into executable host config.
struct LoweredOptimizer {
    max_metric_calls: u64,
    max_candidates: Option<NonZeroUsize>,
    train_minibatch_size: Option<NonZeroUsize>,
    max_cost_usd_micro: Option<u64>,
}

fn lower_seed(seed: &ArtifactRecord) -> Result<SeamPromptArtifact, OptimizeRunHostError> {
    // V1 executes only the prompt artifact type. The agent_kit projection parses
    // at the wire layer (the Git-backed AgentKit host slice lands later), so the
    // host refuses it by typed payload kind naming the supported type.
    match seed.payload() {
        ArtifactPayload::Prompt { template } => Ok(SeamPromptArtifact::new(template)),
        ArtifactPayload::AgentKit { .. } => Err(OptimizeRunHostError::unsupported(format!(
            "artifact_type `{}` is not executable; the supported V1 artifact type is `{SUPPORTED_ARTIFACT_TYPE}`",
            seed.artifact_type()
        ))),
    }
}

fn lower_optimizer(optimizer: &OptimizerConfig) -> Result<LoweredOptimizer, OptimizeRunHostError> {
    match optimizer.objective() {
        OptimizeObjective::Instance => {}
        other => {
            return Err(OptimizeRunHostError::unsupported(format!(
                "optimizer objective `{}` is not executable; the supported V1 objective is `instance`",
                other.as_str()
            )));
        }
    }
    Ok(LoweredOptimizer {
        max_metric_calls: optimizer.max_metric_calls(),
        max_candidates: lower_population_size(optimizer.population_size())?,
        train_minibatch_size: lower_minibatch_size(optimizer.minibatch_size()),
        max_cost_usd_micro: optimizer.max_cost_usd_micro(),
    })
}

/// Lowers `population_size` into the GEPA candidate-pool cap.
///
/// The wire schema already guarantees `population_size >= 1`. The service adds a
/// tighter law: the cap counts the seed plus loop-authored children, so a cap of
/// 1 admits only the seed and can never author a child. That is a no-op
/// optimization request, so it is refused naming the `>= 2` bound. Absent
/// `population_size` leaves the loop unbounded (reference behavior).
fn lower_population_size(
    population_size: Option<u64>,
) -> Result<Option<NonZeroUsize>, OptimizeRunHostError> {
    let Some(size) = population_size else {
        return Ok(None);
    };
    if size < 2 {
        return Err(OptimizeRunHostError::unsupported(
            "optimizer.population_size must be at least 2; a cap of 1 admits only the seed and can never author a child",
        ));
    }
    Ok(NonZeroUsize::new(
        usize::try_from(size).unwrap_or(usize::MAX),
    ))
}

/// Lowers `minibatch_size` into the GEPA train screening minibatch override.
///
/// The wire schema already guarantees `minibatch_size >= 1`, so any present
/// value is a valid override. Absent `minibatch_size` keeps the profile-fixed
/// reference minibatch.
fn lower_minibatch_size(minibatch_size: Option<u64>) -> Option<NonZeroUsize> {
    minibatch_size.and_then(|size| NonZeroUsize::new(usize::try_from(size).unwrap_or(usize::MAX)))
}

fn lower_reflection(reflection: &OptimizeReflection) -> Result<String, OptimizeRunHostError> {
    match reflection {
        OptimizeReflection::Lm { model } => Ok(model.clone()),
        OptimizeReflection::Agentic => Err(OptimizeRunHostError::unsupported(
            "reflection kind `agentic` is not executable; the supported V1 reflection kind is `lm`",
        )),
    }
}
