use leaven_kernel::CandidateId;
use leaven_stage::{
    StageQuery, StageQueryKind,
    tool::{leaven_query_help, parse_leaven_query_args},
};

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
    assert_eq!(
        leaven_stage::tool::parse_leaven_query_args(&["list".to_owned(), "candidates".to_owned()])
            .unwrap(),
        StageQuery::ListCandidates
    );
    assert_eq!(
        leaven_stage::tool::parse_leaven_query_args(&["evidence".to_owned()]).unwrap(),
        StageQuery::Evidence
    );
}

#[test]
fn leaven_query_rejects_unknown_flags_and_bad_ids() {
    assert!(parse_leaven_query_args(&["search".to_owned()]).is_err());
    assert!(parse_leaven_query_args(&["candidate".to_owned(), "not-a-uuid".to_owned()]).is_err());
    assert!(parse_leaven_query_args(&["candidate".to_owned(), "--artifact".to_owned()]).is_err());
    assert!(parse_leaven_query_args(&["candidate".to_owned(), "../secret".to_owned()]).is_err());
}

#[test]
fn leaven_query_help_lists_all_v0_4_variants() {
    let help = leaven_query_help();
    for kind in StageQueryKind::all_v0_4() {
        assert!(
            help.contains(kind.label()),
            "help omitted {}:\n{help}",
            kind.label()
        );
    }
}
