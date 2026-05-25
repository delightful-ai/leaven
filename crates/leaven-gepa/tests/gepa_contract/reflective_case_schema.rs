use leaven_core::{ExternalRef, InfoRef};
use leaven_gepa::{
    Attachment, AttachmentKind, Check, Checks, ReflectRequest, ReflectiveCase, ReflectiveRun,
    ReflectiveValue,
};
use leaven_kernel::{AgentId, CandidateId, CaseId, CaseRunId, TraceRef};

#[test]
fn reflective_case_serializes_runs_checks_and_attachments() {
    let trace = TraceRef {
        store: "trace-store".to_owned(),
        key: "session/main".to_owned(),
    };
    let run = ReflectiveRun {
        run_id: CaseRunId::new(),
        agent_id: Some(AgentId::from("worker")),
        attempt_index: Some(1),
        produced: Some(ReflectiveValue::Text("answer".to_owned())),
        score: Some(0.42),
        max_score: Some(1.0),
        passed: Some(false),
        feedback: "missed grounding".to_owned(),
        checks: Some(Checks {
            passes: vec![Check {
                id: "format".to_owned(),
                requirement: "valid format".to_owned(),
                reason: None,
            }],
            fails: vec![Check {
                id: "grounding".to_owned(),
                requirement: "cite traces".to_owned(),
                reason: Some("no trace citation".to_owned()),
            }],
        }),
        side_info: Vec::new(),
        attachments: vec![Attachment {
            name: "session/main".to_owned(),
            kind: AttachmentKind::Transcript(trace),
            media_type: Some("text/markdown".to_owned()),
        }],
        source_refs: vec![InfoRef::External(ExternalRef {
            kind: "trace".to_owned(),
            id: "session/main".to_owned(),
        })],
    };
    let case = ReflectiveCase {
        case_id: Some(CaseId::new(7)),
        input: ReflectiveValue::Json(serde_json::json!({"task": "solve"})),
        expected: Some(ReflectiveValue::Text("expected".to_owned())),
        runs: vec![run],
        source_refs: vec![InfoRef::External(ExternalRef {
            kind: "case".to_owned(),
            id: "7".to_owned(),
        })],
    };

    let encoded = serde_json::to_string(&case).unwrap();
    assert!(encoded.contains("\"case_id\":7"));
    assert!(encoded.contains("\"kind\":\"json\""));
    assert!(encoded.contains("\"attachments\""));
    assert!(encoded.contains("\"fails\""));

    let decoded: ReflectiveCase = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, case);
}

#[test]
fn flat_example_constructor_builds_one_run_case() {
    let case = ReflectiveCase::from_example(
        ReflectiveValue::Text("input".to_owned()),
        None,
        Some(ReflectiveValue::Text("output".to_owned())),
        Some(0.5),
        "feedback",
    );

    assert_eq!(case.case_id, None);
    assert_eq!(case.input, ReflectiveValue::Text("input".to_owned()));
    assert_eq!(case.expected, None);
    assert_eq!(case.runs.len(), 1);
    assert_eq!(case.runs[0].attempt_index, Some(0));
    assert_eq!(
        case.runs[0].produced,
        Some(ReflectiveValue::Text("output".to_owned()))
    );
    assert_eq!(case.runs[0].score, Some(0.5));
    assert_eq!(case.runs[0].feedback, "feedback");
    let same = ReflectiveCase::from_example(
        ReflectiveValue::Text("input".to_owned()),
        None,
        Some(ReflectiveValue::Text("output".to_owned())),
        Some(0.5),
        "feedback",
    );
    let different_score = ReflectiveCase::from_example(
        ReflectiveValue::Text("input".to_owned()),
        None,
        Some(ReflectiveValue::Text("output".to_owned())),
        None,
        "feedback",
    );
    assert_eq!(case.runs[0].run_id, same.runs[0].run_id);
    assert_ne!(case.runs[0].run_id, different_score.runs[0].run_id);
}

#[test]
fn reflect_request_informed_by_includes_case_and_run_refs() {
    let parent = CandidateId::new();
    let case_ref = InfoRef::External(ExternalRef {
        kind: "case".to_owned(),
        id: "1".to_owned(),
    });
    let run_ref = InfoRef::External(ExternalRef {
        kind: "run".to_owned(),
        id: "1/0".to_owned(),
    });
    let mut case = ReflectiveCase::from_example(
        ReflectiveValue::Text("input".to_owned()),
        None,
        None,
        None,
        "",
    );
    case.source_refs.push(case_ref.clone());
    case.runs[0].source_refs.push(run_ref.clone());

    let request = ReflectRequest::for_part(parent, "system", "system").with_examples([case]);

    assert!(request.informed_by().contains(&case_ref));
    assert!(request.informed_by().contains(&run_ref));
}
