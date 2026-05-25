use leaven_core::InfoRef;
use leaven_evidence::Attachment;
use leaven_kernel::{AgentId, CandidateId, CaseId, CaseRunId, TraceRef};

use super::identity::deterministic_example_run_id;

/// One evaluated case, projected for GEPA reflection.
///
/// The case owns runner-visible input/expected material and one or more runs
/// over that input. LM-parity datasets emit exactly one run per case; agentic
/// datasets can attach transcripts, checks, and multiple attempts without
/// flattening them into prose.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReflectiveCase {
    /// Case the artifact ran on, when the projection knows it.
    pub case_id: Option<CaseId>,
    /// Runner-visible case input.
    pub input: ReflectiveValue,
    /// Optional expected output, reference, rubric, or oracle.
    pub expected: Option<ReflectiveValue>,
    /// Runs observed for this case.
    pub runs: Vec<ReflectiveRun>,
    /// Provenance refs for this case-level projection.
    pub source_refs: Vec<InfoRef>,
}

impl ReflectiveCase {
    /// Flat constructor for the common single-run reflective row.
    #[must_use]
    pub fn from_example(
        input: ReflectiveValue,
        expected: Option<ReflectiveValue>,
        produced: Option<ReflectiveValue>,
        score: Option<f64>,
        feedback: impl Into<String>,
    ) -> Self {
        let feedback = feedback.into();
        let run_id = deterministic_example_run_id(
            &input,
            expected.as_ref(),
            produced.as_ref(),
            score,
            &feedback,
        );
        Self {
            case_id: None,
            input,
            expected,
            runs: vec![ReflectiveRun {
                run_id,
                agent_id: None,
                attempt_index: Some(0),
                produced,
                score,
                max_score: None,
                passed: None,
                feedback,
                checks: None,
                side_info: Vec::new(),
                attachments: Vec::new(),
                source_refs: Vec::new(),
            }],
            source_refs: Vec::new(),
        }
    }
}

/// One observed artifact run for a reflective case.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReflectiveRun {
    pub run_id: CaseRunId,
    pub agent_id: Option<AgentId>,
    pub attempt_index: Option<usize>,
    pub produced: Option<ReflectiveValue>,
    pub score: Option<f64>,
    pub max_score: Option<f64>,
    pub passed: Option<bool>,
    pub feedback: String,
    pub checks: Option<Checks>,
    /// LM-paradigm flat field rendering. Empty for agent-paradigm cases.
    pub side_info: Vec<(String, ReflectiveSideInfoValue)>,
    /// Agent-paradigm typed evidence. Empty for LM-only cases.
    pub attachments: Vec<Attachment>,
    pub source_refs: Vec<InfoRef>,
}

/// Reflective value material that can be rendered or materialized.
///
/// `serde_json::Value` carries `f64`, so this enum stays `PartialEq` only.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ReflectiveValue {
    Text(String),
    Json(serde_json::Value),
    File(TraceRef),
    Mapping(Vec<(String, Self)>),
}

impl Default for ReflectiveValue {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

/// Structured pass/fail checks for a reflective run.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Checks {
    pub passes: Vec<Check>,
    pub fails: Vec<Check>,
}

/// One structured pass/fail check.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub id: String,
    pub requirement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Upstream GEPA side-info value rendered into markdown for reflection.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ReflectiveSideInfoValue {
    /// Scalar text rendered as-is after trimming surrounding whitespace.
    Text(String),
    /// Ordered mapping rendered as nested markdown headings.
    Mapping(Vec<(String, Self)>),
    /// Ordered list rendered as `Item N` nested markdown headings.
    List(Vec<Self>),
}

impl ReflectiveSideInfoValue {
    /// Build an ordered mapping value.
    pub fn mapping(fields: impl IntoIterator<Item = (String, Self)>) -> Self {
        Self::Mapping(fields.into_iter().collect())
    }

    /// Build an ordered list value.
    pub fn list(items: impl IntoIterator<Item = Self>) -> Self {
        Self::List(items.into_iter().collect())
    }
}

impl From<String> for ReflectiveSideInfoValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ReflectiveSideInfoValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

/// Shared GEPA reflection request.
///
/// The optimizer loop builds this exactly once per reflection step, via the
/// configured [`crate::ReflectiveDatasetBuilder`], and passes the same value to
/// whichever reflector is configured. There is no place for a backend to
/// project the reflective data differently.
///
/// The default `String` part keeps agent-stage JSON requests small. Typed
/// reflectors can use their surface's native `PartId`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReflectRequest<Part = String> {
    /// Candidate the reflection improves.
    pub parent: CandidateId,
    /// Surface part the reflection edits.
    pub part: Part,
    /// Human-readable label for the selected part.
    pub part_label: String,
    /// Reflective cases the reflector presents to the model.
    pub examples: Vec<ReflectiveCase>,
    /// Provenance refs lowered into the resulting proposal's `informed_by`.
    pub source_refs: Vec<InfoRef>,
    /// Stable GEPA proposal-attempt ordinal, when the request comes from the
    /// reference loop.
    pub attempt_index: Option<usize>,
}

impl ReflectRequest<String> {
    /// Build a request with a `String` part equal to its label.
    #[must_use]
    pub fn new(parent: CandidateId, part_label: impl Into<String>) -> Self {
        let part_label = part_label.into();
        Self {
            parent,
            part: part_label.clone(),
            part_label,
            examples: Vec::new(),
            source_refs: Vec::new(),
            attempt_index: None,
        }
    }
}

impl<Part> ReflectRequest<Part> {
    /// Build a request for an explicit surface-native part.
    #[must_use]
    pub fn for_part(parent: CandidateId, part: Part, part_label: impl Into<String>) -> Self {
        Self {
            parent,
            part,
            part_label: part_label.into(),
            examples: Vec::new(),
            source_refs: Vec::new(),
            attempt_index: None,
        }
    }

    /// Attach the reflective cases for this request.
    #[must_use]
    pub fn with_examples(mut self, examples: impl IntoIterator<Item = ReflectiveCase>) -> Self {
        self.examples.extend(examples);
        self
    }

    /// Attach provenance refs lowered into the resulting proposal.
    #[must_use]
    pub fn with_source_refs(mut self, refs: impl IntoIterator<Item = InfoRef>) -> Self {
        self.source_refs.extend(refs);
        self
    }

    /// Attach the GEPA proposal-attempt ordinal for cache-stable reflection
    /// diversity.
    #[must_use]
    pub const fn with_attempt_index(mut self, attempt_index: usize) -> Self {
        self.attempt_index = Some(attempt_index);
        self
    }

    /// All provenance refs for the resulting proposal's `informed_by`.
    ///
    /// This is the union of the request-level `source_refs` and every
    /// example's own `source_refs`.
    #[must_use]
    pub fn informed_by(&self) -> Vec<InfoRef> {
        self.source_refs
            .iter()
            .cloned()
            .chain(self.examples.iter().flat_map(|case| {
                case.source_refs.iter().cloned().chain(
                    case.runs
                        .iter()
                        .flat_map(|run| run.source_refs.iter().cloned()),
                )
            }))
            .collect()
    }
}
