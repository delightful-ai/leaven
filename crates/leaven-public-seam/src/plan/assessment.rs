use serde_json::Value;

use crate::PublicSeamError;
use crate::evidence::EvidenceEnvelopeDocument;

use super::parse::invalid_plan;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssessmentScoreOutputUsage {
    pub(super) independent: usize,
    pub(super) pairwise: usize,
    pub(super) listwise: usize,
    pub(super) evidence_envelopes: usize,
    target_values: Vec<PlanAssessmentTargetValue>,
    preference_values: Vec<PlanAssessmentPreferenceValue>,
    ranking_values: Vec<PlanAssessmentRankingValue>,
    output_values: Vec<PlanScoreOutputValue>,
}

impl AssessmentScoreOutputUsage {
    pub(super) fn merge(&mut self, other: &Self) {
        self.independent += other.independent;
        self.pairwise += other.pairwise;
        self.listwise += other.listwise;
        self.evidence_envelopes += other.evidence_envelopes;
        self.target_values
            .extend(other.target_values.iter().cloned());
        self.preference_values
            .extend(other.preference_values.iter().cloned());
        self.ranking_values
            .extend(other.ranking_values.iter().cloned());
        self.output_values
            .extend(other.output_values.iter().cloned());
    }

    pub(super) fn inspect_submit_assessments(
        &mut self,
        write: &Value,
    ) -> Result<(), PublicSeamError> {
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
            if let Some(target) = object.get("target") {
                self.target_values
                    .push(PlanAssessmentTargetValue::from_schema_valid_value(target));
            }
            if let Some(preference) = object.get("preference") {
                self.preference_values.push(
                    PlanAssessmentPreferenceValue::from_schema_valid_value(preference),
                );
            }
            if let Some(ranking) = object.get("ranking") {
                self.ranking_values
                    .push(PlanAssessmentRankingValue::from_schema_valid_value(ranking));
            }
            if let Some(value) = output.get("value") {
                self.output_values
                    .push(PlanScoreOutputValue::from_schema_valid_value(value));
            }
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

    pub(super) const fn total(&self) -> usize {
        self.independent + self.pairwise + self.listwise
    }

    pub(super) fn output_values(&self) -> &[PlanScoreOutputValue] {
        &self.output_values
    }

    pub(super) fn target_values(&self) -> &[PlanAssessmentTargetValue] {
        &self.target_values
    }

    pub(super) fn preference_values(&self) -> &[PlanAssessmentPreferenceValue] {
        &self.preference_values
    }

    pub(super) fn ranking_values(&self) -> &[PlanAssessmentRankingValue] {
        &self.ranking_values
    }
}

/// Schema-valid JSON value carried by an assessment `Score.output.value`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanScoreOutputValue(Value);

impl PlanScoreOutputValue {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON value carried on the wire by the score output.
    pub const fn as_json(&self) -> &Value {
        &self.0
    }
}

/// Schema-valid JSON value carried by an assessment target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanAssessmentTargetValue(Value);

impl PlanAssessmentTargetValue {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON value carried on the wire by the assessment target.
    pub const fn as_json(&self) -> &Value {
        &self.0
    }
}

/// Schema-valid JSON value carried by a pairwise assessment preference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanAssessmentPreferenceValue(Value);

impl PlanAssessmentPreferenceValue {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON value carried on the wire by the assessment preference.
    pub const fn as_json(&self) -> &Value {
        &self.0
    }
}

/// Schema-valid JSON value carried by a listwise assessment ranking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanAssessmentRankingValue(Value);

impl PlanAssessmentRankingValue {
    fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON value carried on the wire by the assessment ranking.
    pub const fn as_json(&self) -> &Value {
        &self.0
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
