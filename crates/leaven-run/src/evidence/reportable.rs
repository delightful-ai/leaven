use leaven_evidence::{DataClass, OutputRecord};
use leaven_kernel::{CandidateId, CaseId};

/// Reportable score output minted from one scoring context.
///
/// The private scope prevents a scorer from satisfying the output contract with
/// a reusable placeholder. The evaluator unwraps it only when it belongs to the
/// candidate/case or candidate-group/case context currently being assessed.
#[derive(Clone, Debug)]
pub struct ReportableOutput {
    record: OutputRecord,
    scope: ReportableOutputScope,
    expected: Option<ReportableOutputDeclaration>,
}

impl ReportableOutput {
    pub(crate) fn new(
        record: OutputRecord,
        scope: ReportableOutputScope,
        expected: Option<ReportableOutputDeclaration>,
    ) -> Self {
        Self {
            record,
            scope,
            expected,
        }
    }

    pub(crate) fn into_record(
        self,
        expected_scope: &ReportableOutputScope,
    ) -> Result<OutputRecord, ReportableOutputError> {
        if self.scope != *expected_scope {
            return Err(ReportableOutputError::WrongScope);
        }
        if is_placeholder_output(&self.record) {
            return Err(ReportableOutputError::Placeholder);
        }
        let Some(expected) = self.expected else {
            return Err(ReportableOutputError::MissingAssessedOutput);
        };
        if expected.is_unbound_explicit_candidate_output() {
            return Err(ReportableOutputError::UnboundCandidateOutput);
        }
        if expected.is_unbound_explicit_candidate_artifact() {
            return Err(ReportableOutputError::UnboundCandidateArtifact);
        }
        if !is_assessed_candidate_or_artifact_output(&expected.record) {
            return Err(ReportableOutputError::MissingAssessedDataClass);
        }
        if !same_output_payload(&self.record, &expected.record) {
            return Err(ReportableOutputError::Unrelated);
        }
        Ok(expected.record)
    }
}

#[derive(Clone, Debug)]
pub struct ReportableOutputDeclaration {
    record: OutputRecord,
    origin: ReportableOutputOrigin,
}

impl ReportableOutputDeclaration {
    pub(crate) fn derived(record: OutputRecord) -> Self {
        Self {
            record,
            origin: ReportableOutputOrigin::DerivedFromRunnerOutput,
        }
    }

    pub(crate) fn explicit(record: OutputRecord) -> Self {
        Self {
            record,
            origin: ReportableOutputOrigin::ExplicitRecord,
        }
    }

    pub(crate) fn record(&self) -> &OutputRecord {
        &self.record
    }

    pub(crate) fn is_unbound_explicit_candidate_output(&self) -> bool {
        self.origin == ReportableOutputOrigin::ExplicitRecord
            && self
                .record
                .data_classes()
                .contains(&DataClass::candidate_output())
    }

    pub(crate) fn is_unbound_explicit_candidate_artifact(&self) -> bool {
        self.origin == ReportableOutputOrigin::ExplicitRecord
            && self
                .record
                .data_classes()
                .contains(&DataClass::candidate_artifact())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReportableOutputOrigin {
    DerivedFromRunnerOutput,
    ExplicitRecord,
}

fn is_placeholder_output(record: &OutputRecord) -> bool {
    matches!(record, OutputRecord::Inline { text, .. } if text.trim().is_empty())
}

fn is_assessed_candidate_or_artifact_output(record: &OutputRecord) -> bool {
    let classes = record.data_classes();
    classes.contains(&DataClass::candidate_output())
        || classes.contains(&DataClass::candidate_artifact())
}

fn same_output_payload(reported: &OutputRecord, expected: &OutputRecord) -> bool {
    match (reported, expected) {
        (
            OutputRecord::Inline {
                text: reported,
                truncated: reported_truncated,
                ..
            },
            OutputRecord::Inline {
                text: expected,
                truncated: expected_truncated,
                ..
            },
        ) => reported == expected && reported_truncated == expected_truncated,
        (
            OutputRecord::BlobRef {
                reference: reported,
                ..
            },
            OutputRecord::BlobRef {
                reference: expected,
                ..
            },
        ) => reported == expected,
        _ => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReportableOutputScope {
    candidates: Vec<CandidateId>,
    case: CaseId,
}

impl ReportableOutputScope {
    pub(crate) fn new(candidate: CandidateId, case: CaseId) -> Self {
        Self {
            candidates: vec![candidate],
            case,
        }
    }

    pub(crate) fn group(candidates: Vec<CandidateId>, case: CaseId) -> Self {
        Self { candidates, case }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReportableOutputError {
    /// The output was minted for a different candidate/case scoring context.
    #[error("reportable output came from another scoring context")]
    WrongScope,
    /// The output exists only as an empty inline placeholder.
    #[error("reportable output was an empty placeholder")]
    Placeholder,
    /// The runner did not declare the assessed output record.
    #[error("runner output did not declare reportable assessed output")]
    MissingAssessedOutput,
    /// The runner-declared output is not classified as assessed candidate/artifact output.
    #[error("runner output did not declare candidate or artifact assessed output")]
    MissingAssessedDataClass,
    /// The runner explicitly declared candidate-output data without a typed-output binding.
    #[error("runner output did not derive candidate output from typed output")]
    UnboundCandidateOutput,
    /// The runner explicitly declared candidate-artifact data without deriving it from artifact identity.
    #[error("runner output did not derive candidate artifact from artifact identity")]
    UnboundCandidateArtifact,
    /// The score reported output that was not the runner output being assessed.
    #[error("reportable output did not match assessed output")]
    Unrelated,
}

#[cfg(test)]
mod tests {
    use leaven_evidence::OutputRecord;
    use leaven_kernel::{CandidateId, CaseId};

    use super::{
        ReportableOutput, ReportableOutputDeclaration, ReportableOutputError, ReportableOutputScope,
    };

    #[test]
    fn grouped_reportable_output_scopes_do_not_match_single_candidate_scopes() {
        let case = CaseId::new(1);
        let left = CandidateId::new();
        let right = CandidateId::new();
        let group = ReportableOutputScope::group(vec![left, right], case);
        let single = ReportableOutputScope::new(left, case);
        let output = ReportableOutput {
            record: OutputRecord::candidate_inline("left/right comparison"),
            scope: group.clone(),
            expected: Some(ReportableOutputDeclaration::derived(
                OutputRecord::candidate_inline("left/right comparison"),
            )),
        };

        assert!(matches!(
            output.clone().into_record(&single),
            Err(ReportableOutputError::WrongScope)
        ));
        assert!(output.into_record(&group).is_ok());
    }
}
