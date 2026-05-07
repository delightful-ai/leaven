use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_kernel::CaseId;

#[test]
fn sparse_casewise_evidence_uses_absence_for_missing_cases() {
    let evidence = CasewiseEvidence::new(vec![CaseOutcome::new(
        CaseId::new(2),
        ScalarEvidence::new(0.7).unwrap(),
    )]);

    assert!(evidence.get(CaseId::new(1)).is_none());
    assert_eq!(
        *evidence.get(CaseId::new(2)).unwrap(),
        ScalarEvidence::new(0.7).unwrap()
    );
}

#[test]
fn duplicate_case_ids_canonicalize_with_last_outcome_winning() {
    let evidence = CasewiseEvidence::new(vec![
        CaseOutcome::new(CaseId::new(1), ScalarEvidence::new(0.1).unwrap()),
        CaseOutcome::new(CaseId::new(1), ScalarEvidence::new(0.9).unwrap()),
    ]);

    assert_eq!(evidence.outcomes().len(), 1);
    assert_eq!(
        *evidence.get(CaseId::new(1)).unwrap(),
        ScalarEvidence::new(0.9).unwrap()
    );
}

#[test]
fn outcomes_are_sorted_by_case_id() {
    let evidence = CasewiseEvidence::new(vec![
        CaseOutcome::new(CaseId::new(3), ScalarEvidence::new(0.3).unwrap()),
        CaseOutcome::new(CaseId::new(1), ScalarEvidence::new(0.1).unwrap()),
        CaseOutcome::new(CaseId::new(2), ScalarEvidence::new(0.2).unwrap()),
    ]);

    let cases: Vec<_> = evidence.outcomes().iter().map(CaseOutcome::case).collect();
    assert_eq!(cases, vec![CaseId::new(1), CaseId::new(2), CaseId::new(3)]);
}
