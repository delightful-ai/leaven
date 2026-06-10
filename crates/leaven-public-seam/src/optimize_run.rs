use serde_json::Value;

use crate::PublicSeamError;

/// Optimizer objective dispatched by a `leaven/optimize.run` request.
///
/// V1 wires `instance` (Pareto frontier over per-case scores) and `objective`
/// (collapsed aggregate objective) as executable objectives; `hybrid` and
/// `cartesian` parse as honest variants at the wire layer but are validate-only
/// at the service layer until their host strategy lands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptimizeObjective {
    /// Per-instance Pareto frontier objective.
    Instance,
    /// Collapsed aggregate objective.
    Objective,
    /// Hybrid instance/objective composition (wire-only in V1).
    Hybrid,
    /// Cartesian instance/objective composition (wire-only in V1).
    Cartesian,
}

impl OptimizeObjective {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "instance" => Ok(Self::Instance),
            "objective" => Ok(Self::Objective),
            "hybrid" => Ok(Self::Hybrid),
            "cartesian" => Ok(Self::Cartesian),
            other => Err(invalid_optimize_run(format!(
                "unknown optimizer objective `{other}`"
            ))),
        }
    }

    /// Wire spelling of the objective.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instance => "instance",
            Self::Objective => "objective",
            Self::Hybrid => "hybrid",
            Self::Cartesian => "cartesian",
        }
    }
}

/// Reflection path requested by a `leaven/optimize.run` request.
///
/// Provider details (concrete LM client, agentic runtime) stay
/// service-configured; the wire only carries the reflection kind and, for the
/// LM path, the model name the host should reflect with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptimizeReflection {
    /// LM-backed reflection with a named model.
    Lm {
        /// Model the host should reflect with.
        model: String,
    },
    /// Agentic reflection through a service-configured runtime.
    Agentic,
}

impl OptimizeReflection {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_optimize_run("optimize run reflection must be an object"))?;
        match required_str(object.get("kind"), "reflection.kind")? {
            "lm" => Ok(Self::Lm {
                model: required_nonempty_str(object.get("model"), "reflection.model")?.to_owned(),
            }),
            "agentic" => Ok(Self::Agentic),
            other => Err(invalid_optimize_run(format!(
                "unknown reflection kind `{other}`"
            ))),
        }
    }
}

/// Optimizer configuration carried by a `leaven/optimize.run` request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizerConfig {
    max_metric_calls: u64,
    population_size: Option<u64>,
    minibatch_size: Option<u64>,
    objective: OptimizeObjective,
}

impl OptimizerConfig {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_optimize_run("optimize run optimizer must be an object"))?;
        let max_metric_calls =
            required_u64(object.get("max_metric_calls"), "optimizer.max_metric_calls")?;
        if max_metric_calls < 1 {
            return Err(invalid_optimize_run(
                "optimizer.max_metric_calls must be at least 1",
            ));
        }
        let population_size =
            optional_u64(object.get("population_size"), "optimizer.population_size")?;
        let minibatch_size =
            optional_u64(object.get("minibatch_size"), "optimizer.minibatch_size")?;
        let objective = OptimizeObjective::parse(required_str(
            object.get("objective"),
            "optimizer.objective",
        )?)?;
        Ok(Self {
            max_metric_calls,
            population_size,
            minibatch_size,
            objective,
        })
    }

    /// Maximum number of metric calls the optimizer may spend.
    pub const fn max_metric_calls(&self) -> u64 {
        self.max_metric_calls
    }

    /// Optimizer population size, if configured.
    pub const fn population_size(&self) -> Option<u64> {
        self.population_size
    }

    /// Optimizer minibatch size, if configured.
    pub const fn minibatch_size(&self) -> Option<u64> {
        self.minibatch_size
    }

    /// Requested optimizer objective.
    pub const fn objective(&self) -> OptimizeObjective {
        self.objective
    }
}

/// Split a `leaven/optimize.run` case is assigned to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptimizeSplit {
    /// Training split.
    Train,
    /// Validation split.
    Validation,
    /// Held-out test split.
    Test,
}

impl OptimizeSplit {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "train" => Ok(Self::Train),
            "validation" => Ok(Self::Validation),
            "test" => Ok(Self::Test),
            other => Err(invalid_optimize_run(format!("unknown split `{other}`"))),
        }
    }

    /// Wire spelling of the split.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "validation",
            Self::Test => "test",
        }
    }
}

/// One case in a `leaven/optimize.run` request manifest.
///
/// Unlike runner stage payloads, optimize-run cases legitimately carry targets:
/// this document is consumed by the host, which owns target custody and reads
/// targets through capability-gated case access when scoring. The `target` field
/// is required by the wire schema and may be JSON null when a case has no target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizeCase {
    case: String,
    input: Value,
    target: Value,
    metadata: Option<Value>,
    split: Option<OptimizeSplit>,
}

impl OptimizeCase {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_optimize_run("optimize run case must be an object"))?;
        let case = required_str(object.get("case"), "cases.case")?.to_owned();
        let input = object
            .get("input")
            .ok_or_else(|| invalid_optimize_run("optimize run case must carry an input"))?
            .clone();
        let target = object
            .get("target")
            .ok_or_else(|| invalid_optimize_run("optimize run case must carry a target field"))?
            .clone();
        let metadata = object.get("metadata").cloned();
        let split = object
            .get("split")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| invalid_optimize_run("optimize run case split must be a string"))
            })
            .transpose()?
            .map(OptimizeSplit::parse)
            .transpose()?;
        Ok(Self {
            case,
            input,
            target,
            metadata,
            split,
        })
    }

    /// Case id.
    pub fn case(&self) -> &str {
        &self.case
    }

    /// Case input payload.
    pub const fn input(&self) -> &Value {
        &self.input
    }

    /// Case target payload, JSON null when the case has no target.
    pub const fn target(&self) -> &Value {
        &self.target
    }

    /// Whether this case carries a non-null target.
    pub fn has_target(&self) -> bool {
        !self.target.is_null()
    }

    /// Optional case metadata bag.
    pub const fn metadata(&self) -> Option<&Value> {
        self.metadata.as_ref()
    }

    /// Optional split assignment.
    pub const fn split(&self) -> Option<OptimizeSplit> {
        self.split
    }
}

/// A prompt-artifact wire record carried by `leaven/optimize.run`.
///
/// This mirrors the artifact triple a proposal `create` effect carries
/// (`artifact_type`, `artifact_schema`, `artifact`): the typed artifact name, the
/// schema fingerprint the artifact validates against, and the artifact JSON
/// itself (template-bearing for prompt artifacts). The seed and every frontier
/// entry's artifact use this same record shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRecord {
    artifact_type: String,
    artifact_schema: String,
    artifact: Value,
}

impl ArtifactRecord {
    fn from_schema_valid_value(value: &Value, field: &str) -> Result<Self, PublicSeamError> {
        let object = value.as_object().ok_or_else(|| {
            invalid_optimize_run(format!("optimize run {field} must be an object"))
        })?;
        let artifact_type =
            required_nonempty_str(object.get("artifact_type"), "artifact.artifact_type")?
                .to_owned();
        let artifact_schema =
            required_str(object.get("artifact_schema"), "artifact.artifact_schema")?.to_owned();
        let artifact = object
            .get("artifact")
            .ok_or_else(|| invalid_optimize_run("optimize run artifact must carry an artifact"))?
            .clone();
        Ok(Self {
            artifact_type,
            artifact_schema,
            artifact,
        })
    }

    /// Typed artifact name.
    pub fn artifact_type(&self) -> &str {
        &self.artifact_type
    }

    /// Schema fingerprint the artifact validates against.
    pub fn artifact_schema(&self) -> &str {
        &self.artifact_schema
    }

    /// Artifact JSON body.
    pub const fn artifact(&self) -> &Value {
        &self.artifact
    }
}

/// Schema-valid `leaven/optimize.run` request: seed, cases, optimizer, reflection.
///
/// The host owns the optimization loop. This document carries the seed prompt
/// artifact, a target-bearing case manifest, the optimizer/reflection
/// configuration, and the capability fingerprint that scopes the run. Targets
/// are allowed here precisely because the document goes to the host; runner
/// stage payloads still never carry targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizeRunRequestDocument {
    run_id: String,
    seed: ArtifactRecord,
    cases: Vec<OptimizeCase>,
    optimizer: OptimizerConfig,
    reflection: OptimizeReflection,
    capability_fingerprint: String,
}

impl OptimizeRunRequestDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_optimize_run("optimize run request must be an object"))?;
        require_message(object.get("message"), "optimize_run_request")?;
        let run_id = required_str(object.get("run_id"), "run_id")?.to_owned();
        let seed = ArtifactRecord::from_schema_valid_value(
            object
                .get("seed")
                .ok_or_else(|| invalid_optimize_run("optimize run request must carry a seed"))?,
            "seed",
        )?;
        let cases = object
            .get("cases")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_optimize_run("optimize run request must carry a cases array"))?
            .iter()
            .map(OptimizeCase::from_schema_valid_value)
            .collect::<Result<Vec<_>, _>>()?;
        if cases.is_empty() {
            return Err(invalid_optimize_run(
                "optimize run request must carry at least one case",
            ));
        }
        let optimizer =
            OptimizerConfig::from_schema_valid_value(object.get("optimizer").ok_or_else(
                || invalid_optimize_run("optimize run request must carry an optimizer config"),
            )?)?;
        let reflection =
            OptimizeReflection::from_schema_valid_value(object.get("reflection").ok_or_else(
                || invalid_optimize_run("optimize run request must carry a reflection config"),
            )?)?;
        let capability_fingerprint = required_str(
            object.get("capability_fingerprint"),
            "capability_fingerprint",
        )?
        .to_owned();
        Ok(Self {
            run_id,
            seed,
            cases,
            optimizer,
            reflection,
            capability_fingerprint,
        })
    }

    /// Run id this optimization targets.
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Seed prompt artifact record.
    pub const fn seed(&self) -> &ArtifactRecord {
        &self.seed
    }

    /// Non-empty case manifest.
    pub fn cases(&self) -> &[OptimizeCase] {
        &self.cases
    }

    /// Optimizer configuration.
    pub const fn optimizer(&self) -> &OptimizerConfig {
        &self.optimizer
    }

    /// Reflection configuration.
    pub const fn reflection(&self) -> &OptimizeReflection {
        &self.reflection
    }

    /// Capability fingerprint scoping the run.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }
}

/// One candidate entry in a `leaven/optimize.run` result frontier.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateEntry {
    candidate: Value,
    candidate_id: String,
    parent: Option<Value>,
    score: f64,
    artifact: ArtifactRecord,
}

impl CandidateEntry {
    fn from_schema_valid_value(value: &Value, field: &str) -> Result<Self, PublicSeamError> {
        let object = value.as_object().ok_or_else(|| {
            invalid_optimize_run(format!("optimize run {field} must be an object"))
        })?;
        let candidate = object
            .get("candidate")
            .ok_or_else(|| invalid_optimize_run("optimize run entry must carry a candidate"))?
            .clone();
        let candidate_id = candidate_ref_id(&candidate)?;
        let parent = match object.get("parent") {
            None => {
                return Err(invalid_optimize_run(
                    "optimize run entry must carry a parent field",
                ));
            }
            Some(Value::Null) => None,
            Some(parent) => {
                // Validate the parent ref resolves to an id, then keep it.
                candidate_ref_id(parent)?;
                Some(parent.clone())
            }
        };
        let score = required_finite_number(object.get("score"), "entry.score")?;
        let artifact = ArtifactRecord::from_schema_valid_value(
            object
                .get("artifact")
                .ok_or_else(|| invalid_optimize_run("optimize run entry must carry an artifact"))?,
            "entry.artifact",
        )?;
        Ok(Self {
            candidate,
            candidate_id,
            parent,
            score,
            artifact,
        })
    }

    /// Candidate ref (id string or object form).
    pub const fn candidate(&self) -> &Value {
        &self.candidate
    }

    /// Resolved candidate id.
    pub fn candidate_id(&self) -> &str {
        &self.candidate_id
    }

    /// Parent candidate ref, if any.
    pub const fn parent(&self) -> Option<&Value> {
        self.parent.as_ref()
    }

    /// Candidate score.
    pub const fn score(&self) -> f64 {
        self.score
    }

    /// Candidate artifact record.
    pub const fn artifact(&self) -> &ArtifactRecord {
        &self.artifact
    }
}

/// Durable run reference returned by a `leaven/optimize.run` result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizeRunReference {
    run: String,
    revision: String,
}

impl OptimizeRunReference {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_optimize_run("optimize run reference must be an object"))?;
        Ok(Self {
            run: required_str(object.get("run"), "run.run")?.to_owned(),
            revision: required_str(object.get("revision"), "run.revision")?.to_owned(),
        })
    }

    /// Durable run id.
    pub fn run(&self) -> &str {
        &self.run
    }

    /// Final graph revision the SDK can read back from.
    pub fn revision(&self) -> &str {
        &self.revision
    }
}

/// Schema-valid `leaven/optimize.run` result: the optimized projection.
///
/// The result carries the best candidate, the frontier, iteration and
/// metric-call counts, aggregate cost, the durable run/revision reference, and
/// the applied proposal-batch receipts. The semantic law is that `best` must
/// appear in `frontier` (matched by candidate id) so the projection cannot claim
/// a best candidate the frontier never admitted.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizeRunResultDocument {
    best: CandidateEntry,
    frontier: Vec<CandidateEntry>,
    iterations: u64,
    metric_calls_used: u64,
    cost: Value,
    run: OptimizeRunReference,
    applied_proposals: Vec<String>,
}

impl OptimizeRunResultDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_optimize_run("optimize run result must be an object"))?;
        require_message(object.get("message"), "optimize_run_result")?;
        let best = CandidateEntry::from_schema_valid_value(
            object
                .get("best")
                .ok_or_else(|| invalid_optimize_run("optimize run result must carry best"))?,
            "best",
        )?;
        let frontier = object
            .get("frontier")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_optimize_run("optimize run result must carry a frontier array"))?
            .iter()
            .map(|value| CandidateEntry::from_schema_valid_value(value, "frontier"))
            .collect::<Result<Vec<_>, _>>()?;
        if frontier.is_empty() {
            return Err(invalid_optimize_run(
                "optimize run result frontier must not be empty",
            ));
        }
        if !frontier
            .iter()
            .any(|entry| entry.candidate_id() == best.candidate_id())
        {
            return Err(invalid_optimize_run(
                "optimize run result best candidate must appear in the frontier",
            ));
        }
        let iterations = required_u64(object.get("iterations"), "iterations")?;
        let metric_calls_used = required_u64(object.get("metric_calls_used"), "metric_calls_used")?;
        let cost = object
            .get("cost")
            .ok_or_else(|| invalid_optimize_run("optimize run result must carry a cost"))?
            .clone();
        let run =
            OptimizeRunReference::from_schema_valid_value(object.get("run").ok_or_else(|| {
                invalid_optimize_run("optimize run result must carry a run reference")
            })?)?;
        let applied_proposals = applied_proposals(object.get("applied_proposals"))?;
        Ok(Self {
            best,
            frontier,
            iterations,
            metric_calls_used,
            cost,
            run,
            applied_proposals,
        })
    }

    /// Best candidate produced by the optimization.
    pub const fn best(&self) -> &CandidateEntry {
        &self.best
    }

    /// Non-empty frontier of admitted candidates.
    pub fn frontier(&self) -> &[CandidateEntry] {
        &self.frontier
    }

    /// Number of optimizer iterations performed.
    pub const fn iterations(&self) -> u64 {
        self.iterations
    }

    /// Number of metric calls spent.
    pub const fn metric_calls_used(&self) -> u64 {
        self.metric_calls_used
    }

    /// Aggregate cost JSON returned by the optimization.
    pub const fn cost(&self) -> &Value {
        &self.cost
    }

    /// Durable run/revision reference for SDK readback.
    pub const fn run(&self) -> &OptimizeRunReference {
        &self.run
    }

    /// Applied proposal-batch receipt ids.
    pub fn applied_proposals(&self) -> &[String] {
        &self.applied_proposals
    }
}

fn require_message(value: Option<&Value>, expected: &str) -> Result<(), PublicSeamError> {
    match value.and_then(Value::as_str) {
        Some(message) if message == expected => Ok(()),
        _ => Err(invalid_optimize_run(format!(
            "optimize run document must declare message `{expected}`"
        ))),
    }
}

fn required_str<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value.and_then(Value::as_str).ok_or_else(|| {
        invalid_optimize_run(format!("optimize run field `{field}` must be a string"))
    })
}

fn required_nonempty_str<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    let text = required_str(value, field)?;
    if text.is_empty() {
        return Err(invalid_optimize_run(format!(
            "optimize run field `{field}` must not be empty"
        )));
    }
    Ok(text)
}

fn required_u64(value: Option<&Value>, field: &str) -> Result<u64, PublicSeamError> {
    value
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_optimize_run(format!("optimize run field `{field}` must be a u64")))
}

fn optional_u64(value: Option<&Value>, field: &str) -> Result<Option<u64>, PublicSeamError> {
    value
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                invalid_optimize_run(format!("optimize run field `{field}` must be a u64"))
            })
        })
        .transpose()
}

fn required_finite_number(value: Option<&Value>, field: &str) -> Result<f64, PublicSeamError> {
    let number = value.and_then(Value::as_f64).ok_or_else(|| {
        invalid_optimize_run(format!("optimize run field `{field}` must be a number"))
    })?;
    if !number.is_finite() {
        return Err(invalid_optimize_run(format!(
            "optimize run field `{field}` must be finite"
        )));
    }
    Ok(number)
}

fn candidate_ref_id(value: &Value) -> Result<String, PublicSeamError> {
    match value {
        Value::String(id) if !id.trim().is_empty() => Ok(id.to_owned()),
        Value::Object(object) => {
            required_str(object.get("id"), "candidate.id").map(ToOwned::to_owned)
        }
        _ => Err(invalid_optimize_run(
            "optimize run candidate ref must carry an id",
        )),
    }
}

fn applied_proposals(value: Option<&Value>) -> Result<Vec<String>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let ids = value
        .as_array()
        .ok_or_else(|| invalid_optimize_run("applied_proposals must be an array"))?;
    ids.iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_optimize_run("applied_proposals entry must be a string"))
        })
        .collect()
}

fn invalid_optimize_run(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidOptimizeRun {
        message: message.into(),
    }
}
