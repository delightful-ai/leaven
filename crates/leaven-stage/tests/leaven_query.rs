use leaven_kernel::CandidateId;
use leaven_stage::{StageQuery, tool::parse_leaven_query_args};

#[test]
fn leaven_query_parses_candidate_and_lineage() {
    let candidate = CandidateId::new();
    let query = parse_leaven_query_args(&["candidate".to_owned(), candidate.to_string()]).unwrap();
    assert_eq!(query, StageQuery::Candidate { id: candidate });

    let query =
        parse_leaven_query_args(&["lineage".to_owned(), candidate.to_string(), "3".to_owned()])
            .unwrap();
    assert_eq!(
        query,
        StageQuery::Lineage {
            candidate,
            depth: 3
        }
    );
}

#[test]
fn leaven_query_parses_assessment_diff_and_default_help() {
    let assessment = leaven_kernel::AssessmentId::new();
    let left = leaven_kernel::CandidateId::new();
    let right = leaven_kernel::CandidateId::new();

    assert!(matches!(
        leaven_stage::tool::parse_leaven_query_args(&[]).unwrap(),
        StageQuery::Help
    ));
    assert_eq!(
        leaven_stage::tool::parse_leaven_query_args(&[
            "assessment".to_owned(),
            assessment.as_uuid().to_string(),
        ])
        .unwrap(),
        StageQuery::Assessment { id: assessment }
    );
    assert_eq!(
        leaven_stage::tool::parse_leaven_query_args(&[
            "diff".to_owned(),
            left.as_uuid().to_string(),
            right.as_uuid().to_string(),
        ])
        .unwrap(),
        StageQuery::Diff { left, right }
    );
}

#[test]
fn leaven_query_rejects_unknown_and_bad_ids() {
    assert!(parse_leaven_query_args(&["search".to_owned()]).is_err());
    assert!(parse_leaven_query_args(&["candidate".to_owned(), "not-a-uuid".to_owned()]).is_err());
}
