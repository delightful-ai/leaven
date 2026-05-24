use serde_json::Value;

use crate::evidence::EvidenceEnvelopeDocument;
use crate::{PinnedDialectEvaluator, PublicSeamError};

/// Schema-valid public-seam Plan IR document classified by core operation family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanDocument {
    operation_kinds: Vec<PlanOperationKind>,
    return_names: Vec<String>,
    consistency_kind: String,
    at_revision: Option<String>,
    since_revision: Option<String>,
    until_revision: Option<String>,
    events_since_revision_queries: usize,
    pinned_pointer_count: usize,
    pinned_jsonpath_count: usize,
    strict_template_count: usize,
    assessment_score_outputs: AssessmentScoreOutputUsage,
    mode_kind: String,
    commit_kind: String,
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
        let mut operation_kinds = Vec::with_capacity(ops.len());
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
            let kind = required_string(op, "kind")?;
            let operation_kind = match kind {
                "let" => {
                    if let Some(expr) = op.as_object().and_then(|object| object.get("expr")) {
                        validate_events_revision_sources(
                            expr,
                            &consistency_kind,
                            since_revision.as_deref(),
                            until_revision.as_deref(),
                        )?;
                        events_since_revision_queries += count_events_since_revision_queries(
                            expr,
                            since_revision.as_deref(),
                            until_revision.as_deref(),
                        );
                    }
                    PlanOperationKind::Let
                }
                "call" => {
                    ensure_nested_kind(op, "call", "call")?;
                    PlanOperationKind::Call
                }
                "write" => {
                    let write = op
                        .get("write")
                        .ok_or_else(|| invalid_plan("write op is missing `write`"))?;
                    if nested_kind(Some(write), "write")? == "submit_assessments" {
                        assessment_score_outputs.inspect_submit_assessments(write)?;
                    }
                    PlanOperationKind::Write
                }
                "extension" => {
                    return Err(invalid_plan(
                        "top-level extension plan op is not part of the locked Let/Call/Write family",
                    ));
                }
                other => {
                    return Err(invalid_plan(format!(
                        "unknown plan operation kind `{other}`"
                    )));
                }
            };
            operation_kinds.push(operation_kind);
        }

        Ok(Self {
            operation_kinds,
            return_names: string_array(object.get("return"), "return")?,
            consistency_kind,
            at_revision,
            since_revision,
            until_revision,
            events_since_revision_queries,
            pinned_pointer_count: dialect_usage.pointers,
            pinned_jsonpath_count: dialect_usage.jsonpaths,
            strict_template_count: dialect_usage.templates,
            assessment_score_outputs,
            mode_kind: nested_kind(object.get("mode"), "mode")?.to_owned(),
            commit_kind: nested_kind(object.get("commit"), "commit")?.to_owned(),
        })
    }

    /// Core operation family in document order.
    pub fn operation_kinds(&self) -> &[PlanOperationKind] {
        &self.operation_kinds
    }

    /// Return binding names in document order.
    pub fn return_names(&self) -> &[String] {
        &self.return_names
    }

    /// Consistency mode discriminator.
    pub fn consistency_kind(&self) -> &str {
        &self.consistency_kind
    }

    /// Pinned graph revision for `at_revision` consistency.
    pub fn at_revision(&self) -> Option<&str> {
        self.at_revision.as_deref()
    }

    /// Base graph revision for `since_revision` consistency.
    pub fn since_revision(&self) -> Option<&str> {
        self.since_revision.as_deref()
    }

    /// Upper graph revision for `since_revision` consistency when bounded.
    pub fn until_revision(&self) -> Option<&str> {
        self.until_revision.as_deref()
    }

    /// Number of graph event queries bound to the plan's `since_revision` base.
    pub fn events_since_revision_queries(&self) -> usize {
        self.events_since_revision_queries
    }

    /// Number of RFC 6901 JSON Pointer values semantically validated in the document.
    pub fn pinned_pointer_count(&self) -> usize {
        self.pinned_pointer_count
    }

    /// Number of Leaven-subset `JSONPath` values semantically validated in the document.
    pub fn pinned_jsonpath_count(&self) -> usize {
        self.pinned_jsonpath_count
    }

    /// Number of strict Mustache templates semantically validated in the document.
    pub fn strict_template_count(&self) -> usize {
        self.strict_template_count
    }

    /// Number of assessment `Score.output` values semantically validated.
    pub fn assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.total()
    }

    /// Number of assessment evidence envelopes semantically validated.
    pub fn assessment_evidence_count(&self) -> usize {
        self.assessment_score_outputs.evidence_envelopes
    }

    /// Number of independent assessment `Score.output` values semantically validated.
    pub fn independent_assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.independent
    }

    /// Number of pairwise assessment `Score.output` values semantically validated.
    pub fn pairwise_assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.pairwise
    }

    /// Number of listwise assessment `Score.output` values semantically validated.
    pub fn listwise_assessment_score_output_count(&self) -> usize {
        self.assessment_score_outputs.listwise
    }

    /// Whether this plan is a finite event diff through `consistency.since_revision`.
    pub fn is_since_revision_event_diff(&self) -> bool {
        self.consistency_kind == "since_revision"
            && self.since_revision.is_some()
            && self.events_since_revision_queries > 0
    }

    /// Evaluation mode discriminator.
    pub fn mode_kind(&self) -> &str {
        &self.mode_kind
    }

    /// Commit policy discriminator.
    pub fn commit_kind(&self) -> &str {
        &self.commit_kind
    }
}

/// Locked Plan IR core operation family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanOperationKind {
    /// Pure value/query binding.
    Let,
    /// Effectful capability call.
    Call,
    /// Staged graph mutation intent.
    Write,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AssessmentScoreOutputUsage {
    independent: usize,
    pairwise: usize,
    listwise: usize,
    evidence_envelopes: usize,
}

impl AssessmentScoreOutputUsage {
    fn inspect_submit_assessments(&mut self, write: &Value) -> Result<(), PublicSeamError> {
        let assessments = write
            .as_object()
            .and_then(|object| object.get("assessments"))
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_plan("submit_assessments write must carry assessments"))?;
        for assessment in assessments {
            let object = assessment
                .as_object()
                .ok_or_else(|| invalid_plan("submit_assessments entries must be objects"))?;
            let kind = object
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("submit_assessments entries must carry kind"))?;
            let output = object
                .get("score")
                .and_then(Value::as_object)
                .and_then(|score| score.get("output"))
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    invalid_plan("submit_assessments score must carry a reportable output")
                })?;
            validate_assessment_evidence(object)?;
            validate_assessment_candidates(kind, object)?;
            validate_score_output(kind, object, output)?;
            match kind {
                "independent" => self.independent += 1,
                "pairwise" => self.pairwise += 1,
                "listwise" => self.listwise += 1,
                other => return Err(invalid_plan(format!("unknown assessment kind `{other}`"))),
            }
            self.evidence_envelopes += 1;
        }
        Ok(())
    }

    const fn total(&self) -> usize {
        self.independent + self.pairwise + self.listwise
    }
}

fn validate_assessment_evidence(
    assessment: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let evidence = assessment
        .get("evidence")
        .ok_or_else(|| invalid_plan("submit_assessments assessment must carry evidence"))?;
    EvidenceEnvelopeDocument::from_schema_valid_value(evidence).map_err(|source| {
        invalid_plan(format!(
            "submit_assessments evidence must satisfy EvidenceEnvelope semantics: {source}"
        ))
    })?;
    Ok(())
}

fn validate_score_output(
    assessment_kind: &str,
    assessment: &serde_json::Map<String, Value>,
    output: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let data_classes = output
        .get("data_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("submit_assessments Score.output must carry data_classes"))?;
    let carries_assessed_output = data_classes.iter().any(|class| {
        matches!(
            class.as_str(),
            Some("candidate.output" | "candidate.artifact")
        )
    });
    if !carries_assessed_output {
        return Err(invalid_plan(
            "submit_assessments Score.output must carry candidate.output or candidate.artifact data class",
        ));
    }
    validate_score_output_candidate_binding(assessment_kind, assessment, output)?;
    let summary = output
        .get("summary")
        .and_then(Value::as_str)
        .filter(|summary| !summary.trim().is_empty());
    if let Some(summary) = summary {
        validate_score_output_evidence_projection(assessment, summary)?;
        return Ok(());
    }
    match output.get("value") {
        Some(Value::Null) => {
            return Err(invalid_plan(
                "submit_assessments Score.output value must not be null",
            ));
        }
        Some(Value::String(text)) if text.trim().is_empty() => {}
        Some(Value::String(text)) => {
            validate_score_output_evidence_projection(assessment, text)?;
            return Ok(());
        }
        Some(_) => {
            return Err(invalid_plan(
                "submit_assessments Score.output must carry a non-empty summary for structured output projection",
            ));
        }
        None => {}
    }
    if output.get("blob_ref").is_some()
        || output
            .get("trace_refs")
            .and_then(Value::as_array)
            .is_some_and(|trace_refs| !trace_refs.is_empty())
    {
        return Err(invalid_plan(
            "submit_assessments Score.output blob or trace output must carry a public evidence summary projection",
        ));
    }
    Err(invalid_plan(
        "submit_assessments Score.output must carry reportable output content",
    ))
}

fn validate_assessment_candidates(
    kind: &str,
    assessment: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match kind {
        "independent" => {
            candidate_string(assessment, "candidate")?;
        }
        "pairwise" | "listwise" => {
            candidate_array(assessment, "candidates")?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_score_output_candidate_binding(
    assessment_kind: &str,
    assessment: &serde_json::Map<String, Value>,
    output: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let Some(value) = output.get("value") else {
        if has_score_output_external_projection(output) {
            return Ok(());
        }
        return Err(invalid_plan(
            "submit_assessments Score.output must carry candidate-bound value or blob/trace output projection",
        ));
    };
    match assessment_kind {
        "independent" => {
            let candidate = candidate_string(assessment, "candidate")?;
            validate_candidate_output_entry(value, candidate)
        }
        "pairwise" | "listwise" => {
            let candidates = candidate_array(assessment, "candidates")?;
            let entries = value.as_array().ok_or_else(|| {
                invalid_plan(
                    "submit_assessments pairwise/listwise Score.output value must be candidate entries",
                )
            })?;
            if entries.len() != candidates.len() {
                return Err(invalid_plan(
                    "submit_assessments Score.output candidate entries must match assessed candidates",
                ));
            }
            for (entry, candidate) in entries.iter().zip(candidates) {
                validate_candidate_output_entry(entry, candidate)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn has_score_output_external_projection(output: &serde_json::Map<String, Value>) -> bool {
    output.get("blob_ref").is_some()
        || output
            .get("trace_refs")
            .and_then(Value::as_array)
            .is_some_and(|trace_refs| !trace_refs.is_empty())
}

fn validate_candidate_output_entry(value: &Value, candidate: &str) -> Result<(), PublicSeamError> {
    let entry = value.as_object().ok_or_else(|| {
        invalid_plan("submit_assessments Score.output value must be a candidate-bound object")
    })?;
    if entry.get("candidate").and_then(Value::as_str) != Some(candidate) {
        return Err(invalid_plan(
            "submit_assessments Score.output candidate binding must match assessed candidate",
        ));
    }
    let carries_output = entry
        .get("output")
        .or_else(|| entry.get("artifact"))
        .is_some_and(has_reportable_content);
    if carries_output {
        Ok(())
    } else {
        Err(invalid_plan(
            "submit_assessments Score.output candidate binding must carry output or artifact content",
        ))
    }
}

fn has_reportable_content(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(object) => !object.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn candidate_string<'a>(
    assessment: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, PublicSeamError> {
    assessment
        .get(field)
        .and_then(Value::as_str)
        .filter(|candidate| !candidate.trim().is_empty())
        .ok_or_else(|| {
            invalid_plan(format!(
                "submit_assessments assessment must carry non-empty `{field}`"
            ))
        })
}

fn candidate_array<'a>(
    assessment: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<Vec<&'a str>, PublicSeamError> {
    let values = assessment
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_plan(format!(
                "submit_assessments assessment must carry `{field}`"
            ))
        })?;
    if values.is_empty() {
        return Err(invalid_plan(format!(
            "submit_assessments assessment `{field}` must not be empty"
        )));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|candidate| !candidate.trim().is_empty())
                .ok_or_else(|| {
                    invalid_plan(format!(
                        "submit_assessments assessment `{field}` entries must be non-empty strings"
                    ))
                })
        })
        .collect()
}

fn validate_score_output_evidence_projection(
    assessment: &serde_json::Map<String, Value>,
    expected_summary: &str,
) -> Result<(), PublicSeamError> {
    let evidence_summary = assessment
        .get("evidence")
        .and_then(Value::as_object)
        .and_then(|evidence| evidence.get("public"))
        .and_then(Value::as_object)
        .and_then(|public| public.get("summary"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_plan(
                "submit_assessments Score.output must be projected by evidence.public.summary",
            )
        })?;
    if evidence_summary == expected_summary {
        Ok(())
    } else {
        Err(invalid_plan(
            "submit_assessments Score.output must match evidence.public.summary",
        ))
    }
}

#[derive(Default)]
struct DialectUsage {
    pointers: usize,
    jsonpaths: usize,
    templates: usize,
    evaluator: PinnedDialectEvaluator,
}

impl DialectUsage {
    fn inspect_value(&mut self, value: &Value) -> Result<(), PublicSeamError> {
        match value {
            Value::Object(object) => self.inspect_object(object),
            Value::Array(values) => {
                for value in values {
                    self.inspect_value(value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn inspect_object(
        &mut self,
        object: &serde_json::Map<String, Value>,
    ) -> Result<(), PublicSeamError> {
        if let Some(pointer) = object.get("field").and_then(Value::as_str) {
            self.validate_pointer(pointer)?;
        }
        if object.get("kind").and_then(Value::as_str) == Some("stratified") {
            if let Some(pointer) = object.get("by").and_then(Value::as_str) {
                self.validate_pointer(pointer)?;
            }
        }
        if let Some(fields) = object.get("fields").and_then(Value::as_array) {
            for pointer in fields.iter().filter_map(Value::as_str) {
                self.validate_pointer(pointer)?;
            }
        }
        if object.get("kind").and_then(Value::as_str) == Some("extract") {
            if let Some(path) = object.get("path").and_then(Value::as_str) {
                self.validate_jsonpath(path)?;
            }
        }
        if object.get("kind").and_then(Value::as_str) == Some("template") {
            let dialect = object
                .get("dialect")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("template expression must carry a dialect"))?;
            let template = object
                .get("template")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_plan("template expression must carry a template"))?;
            self.evaluator
                .render_template(dialect, template, &serde_json::json!({}))?;
            self.templates += 1;
        }
        for (key, value) in object {
            if object.get("kind").and_then(Value::as_str) == Some("events") && key == "filter" {
                continue;
            }
            if object.get("kind").and_then(Value::as_str) == Some("schema_valid") && key == "value"
            {
                self.inspect_value(value)?;
                continue;
            }
            if is_arbitrary_json_slot(key) {
                continue;
            }
            self.inspect_value(value)?;
        }
        Ok(())
    }

    fn validate_pointer(&mut self, pointer: &str) -> Result<(), PublicSeamError> {
        match self
            .evaluator
            .resolve_json_pointer(&serde_json::json!({}), pointer)
        {
            Ok(_) => {}
            Err(PublicSeamError::InvalidDialect { message })
                if message.contains("was not present")
                    || message.contains("out of bounds")
                    || message.contains("cannot descend") => {}
            Err(error) => return Err(error),
        }
        self.pointers += 1;
        Ok(())
    }

    fn validate_jsonpath(&mut self, path: &str) -> Result<(), PublicSeamError> {
        self.evaluator
            .extract_json_path(&serde_json::json!({}), path)?;
        self.jsonpaths += 1;
        Ok(())
    }
}

fn is_arbitrary_json_slot(key: &str) -> bool {
    matches!(
        key,
        "value"
            | "values"
            | "payload"
            | "scope"
            | "selector"
            | "provider_hints"
            | "schema"
            | "input_schema"
            | "metadata"
            | "rubric"
            | "causal"
            | "target"
            | "preference"
            | "ranking"
    )
}

fn ensure_nested_kind(value: &Value, field: &str, owner: &str) -> Result<(), PublicSeamError> {
    let _ = value
        .get(field)
        .ok_or_else(|| invalid_plan(format!("{owner} op is missing `{field}`")))?
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("{owner} op `{field}` must carry a typed kind")))?;
    Ok(())
}

fn nested_kind<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan `{field}` must carry a kind")))
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("plan op must carry string `{field}`")))
}

fn string_array(value: Option<&Value>, field: &str) -> Result<Vec<String>, PublicSeamError> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan(format!("plan `{field}` must be an array")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_plan(format!("plan `{field}` entries must be strings")))
        })
        .collect()
}

fn count_events_since_revision_queries(
    value: &Value,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> usize {
    let Some(object) = value.as_object() else {
        return 0;
    };
    match object.get("kind").and_then(Value::as_str) {
        Some("graph_query") => usize::from(graph_query_matches_since_revision(
            object,
            since_revision,
            until_revision,
        )),
        Some("project" | "filter") => object
            .get("input")
            .map(|input| count_events_since_revision_queries(input, since_revision, until_revision))
            .unwrap_or(0),
        _ => 0,
    }
}

fn validate_events_revision_sources(
    value: &Value,
    consistency_kind: &str,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> Result<(), PublicSeamError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    match object.get("kind").and_then(Value::as_str) {
        Some("graph_query") => validate_graph_query_revision_source(
            object,
            consistency_kind,
            since_revision,
            until_revision,
        ),
        Some("project" | "filter") => object
            .get("input")
            .map(|input| {
                validate_events_revision_sources(
                    input,
                    consistency_kind,
                    since_revision,
                    until_revision,
                )
            })
            .unwrap_or(Ok(())),
        _ => Ok(()),
    }
}

fn validate_graph_query_revision_source(
    object: &serde_json::Map<String, Value>,
    consistency_kind: &str,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> Result<(), PublicSeamError> {
    let Some(source) = object.get("source").and_then(Value::as_object) else {
        return Ok(());
    };
    if source.get("kind").and_then(Value::as_str) != Some("events") {
        return Ok(());
    }
    if consistency_kind != "since_revision" {
        return Ok(());
    }
    let Some(since_revision) = since_revision else {
        return Err(invalid_plan(
            "since_revision event queries must carry a plan base revision",
        ));
    };
    if source.get("since_revision").and_then(Value::as_str) != Some(since_revision) {
        return Err(invalid_plan(
            "events source since_revision must match plan consistency base",
        ));
    }
    if let Some(until_revision) = until_revision {
        if source.get("until_revision").and_then(Value::as_str) != Some(until_revision) {
            return Err(invalid_plan(
                "events source until_revision must match plan consistency bound",
            ));
        }
    }
    Ok(())
}

fn graph_query_matches_since_revision(
    object: &serde_json::Map<String, Value>,
    since_revision: Option<&str>,
    until_revision: Option<&str>,
) -> bool {
    let Some(source) = object.get("source").and_then(Value::as_object) else {
        return false;
    };
    if source.get("kind").and_then(Value::as_str) != Some("events") {
        return false;
    }
    let Some(since_revision) = since_revision else {
        return false;
    };
    if source.get("since_revision").and_then(Value::as_str) != Some(since_revision) {
        return false;
    }
    match until_revision {
        Some(until_revision) => {
            source.get("until_revision").and_then(Value::as_str) == Some(until_revision)
        }
        None => true,
    }
}

fn invalid_plan(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
