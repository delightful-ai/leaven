use std::time::Duration;

use leaven_evidence::{
    AgentAnalystCallError, AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput,
    AgentAnalystCallStatus, AgentAnalystFanoutError, AgentAnalystFanoutEvidence, AgentAnalystRole,
    AgentTrajectoryAnalysisKind, AgentTrajectoryAnalysisRecord, AgentTrajectoryCorpusError,
    AgentTrajectoryCorpusEvidence, AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput,
    AgentTrajectoryOutcome, CommandEvidence, CommandRecord, OutputRecord,
};
use leaven_kernel::{AgentSessionId, BlobRef, CaseId, FingerprintBuilder};

#[test]
fn command_record_preserves_status_duration_and_inline_output() {
    let record = CommandRecord::new(
        "python harness.py",
        Some(0),
        OutputRecord::inline("ok"),
        OutputRecord::truncated("warning..."),
        Duration::from_millis(42),
    );

    assert_eq!(record.command(), "python harness.py");
    assert_eq!(record.exit_status(), Some(0));
    assert_eq!(record.duration(), Duration::from_millis(42));
    assert_eq!(record.stdout(), &OutputRecord::inline("ok"));
    assert_eq!(record.stderr(), &OutputRecord::truncated("warning..."));
}

#[test]
fn command_output_can_be_blob_backed() {
    let reference = BlobRef {
        store: "blob-store".to_owned(),
        key: "stdout/0".to_owned(),
    };

    assert_eq!(
        OutputRecord::blob(reference.clone()),
        OutputRecord::BlobRef(reference)
    );
}

#[test]
fn agent_trajectory_groups_transcript_and_commands() {
    let command = CommandRecord::new(
        "pytest",
        Some(1),
        OutputRecord::truncated("one failed"),
        OutputRecord::inline(""),
        Duration::from_secs(2),
    );
    let commands = CommandEvidence::new(vec![command.clone()]);
    let mut fingerprint_builder = FingerprintBuilder::new();
    fingerprint_builder
        .update("model=qwen3.5")
        .update("temperature=0");
    let model_config_fingerprint = fingerprint_builder.finish();
    let trajectory = AgentTrajectoryEvidence::new(AgentTrajectoryEvidenceInput {
        session_id: AgentSessionId::new(),
        case_id: Some(CaseId::from_index(7)),
        task_id: "13-1".to_owned(),
        outcome: AgentTrajectoryOutcome::Failure {
            reason: "spreadsheet answer mismatch".to_owned(),
        },
        model_id: "Qwen3.5-122B-A10B".to_owned(),
        model_config_fingerprint,
        transcript: OutputRecord::inline("agent transcript"),
        commands,
    })
    .with_analysis_records(vec![AgentTrajectoryAnalysisRecord::new(
        AgentTrajectoryAnalysisKind::Error,
        "error_analysis_13-1.md",
        OutputRecord::inline("{\"items\":[{\"lesson\":\"check formula range\"}]}"),
    )]);

    assert_eq!(trajectory.case_id(), Some(CaseId::from_index(7)));
    assert_eq!(trajectory.task_id(), "13-1");
    assert_eq!(
        trajectory.outcome(),
        &AgentTrajectoryOutcome::Failure {
            reason: "spreadsheet answer mismatch".to_owned(),
        }
    );
    assert_eq!(trajectory.model_id(), "Qwen3.5-122B-A10B");
    assert_eq!(
        trajectory.model_config_fingerprint(),
        model_config_fingerprint
    );
    assert_eq!(
        trajectory.transcript(),
        &OutputRecord::inline("agent transcript")
    );
    assert_eq!(trajectory.commands().records(), &[command]);
    assert_eq!(trajectory.analysis_records().len(), 1);
    assert_eq!(
        trajectory.analysis_records()[0].kind(),
        AgentTrajectoryAnalysisKind::Error
    );
    assert_eq!(
        trajectory.analysis_records()[0].source_file(),
        "error_analysis_13-1.md"
    );
    assert_eq!(
        trajectory.analysis_records()[0].payload(),
        &OutputRecord::inline("{\"items\":[{\"lesson\":\"check formula range\"}]}")
    );
}

#[test]
fn agent_trajectory_corpus_tracks_completed_and_pending_task_manifest() {
    let mut corpus =
        AgentTrajectoryCorpusEvidence::new(["13-1".to_owned(), "59902".to_owned()]).unwrap();
    assert_eq!(corpus.expected_task_ids(), ["13-1", "59902"]);
    assert_eq!(corpus.completed_task_ids(), Vec::<&str>::new());
    assert_eq!(corpus.pending_task_ids(), vec!["13-1", "59902"]);

    let trajectory = spreadsheet_trajectory("13-1", CaseId::from_index(0));
    corpus.push(trajectory.clone()).unwrap();

    assert_eq!(corpus.trajectories().len(), 1);
    assert_eq!(corpus.completed_task_ids(), vec!["13-1"]);
    assert_eq!(corpus.pending_task_ids(), vec!["59902"]);
    assert_eq!(corpus.by_task("13-1"), vec![&trajectory]);
    assert!(corpus.by_task("59902").is_empty());
    assert_eq!(
        corpus.by_task("13-1")[0].analysis_records()[0].kind(),
        AgentTrajectoryAnalysisKind::Error
    );

    let duplicate_seed = spreadsheet_trajectory("13-1", CaseId::from_index(0));
    corpus.push(duplicate_seed).unwrap();
    assert_eq!(corpus.by_task("13-1").len(), 2);
    assert_eq!(corpus.completed_task_ids(), vec!["13-1"]);

    let unknown = spreadsheet_trajectory("not-in-manifest", CaseId::from_index(99));
    assert_eq!(
        corpus.push(unknown).unwrap_err(),
        AgentTrajectoryCorpusError::UnknownTask {
            task_id: "not-in-manifest".to_owned(),
        }
    );
}

#[test]
fn agent_trajectory_corpus_refuses_duplicate_manifest_task_ids() {
    assert_eq!(
        AgentTrajectoryCorpusEvidence::new([
            "13-1".to_owned(),
            "59902".to_owned(),
            "13-1".to_owned(),
        ])
        .unwrap_err(),
        AgentTrajectoryCorpusError::DuplicateTask {
            task_id: "13-1".to_owned(),
        }
    );
}

#[test]
fn analyst_fanout_tracks_durable_call_state_and_pending_manifest() {
    let mut fanout =
        AgentAnalystFanoutEvidence::new(["error-13-1".to_owned(), "success-59902".to_owned()])
            .unwrap();
    let call = AgentAnalystCallEvidence::new(AgentAnalystCallEvidenceInput {
        call_id: "error-13-1".to_owned(),
        role: AgentAnalystRole::Error,
        source_task_ids: vec!["13-1".to_owned()],
        prompt: OutputRecord::blob(BlobRef {
            store: "trace2skill-stage2".to_owned(),
            key: "prompts/error-13-1.md".to_owned(),
        }),
        response: Some(OutputRecord::blob(BlobRef {
            store: "trace2skill-stage2".to_owned(),
            key: "responses/error-13-1.md".to_owned(),
        })),
        status: AgentAnalystCallStatus::ParseFailed {
            reason: "patch JSON did not validate".to_owned(),
            artifact: Some(OutputRecord::blob(BlobRef {
                store: "trace2skill-stage2".to_owned(),
                key: "parse_failures/error-13-1.json".to_owned(),
            })),
        },
        retry_count: 2,
        support_count: 3,
    })
    .unwrap();

    fanout.push(call).unwrap();

    assert_eq!(
        fanout.completed_call_ids(),
        vec!["error-13-1"],
        "parse failures are durable completed calls, not missing work"
    );
    assert_eq!(fanout.pending_call_ids(), vec!["success-59902"]);
    let stored = fanout.by_call("error-13-1").unwrap();
    assert_eq!(stored.role(), AgentAnalystRole::Error);
    assert_eq!(stored.source_task_ids(), ["13-1"]);
    assert_eq!(stored.retry_count(), 2);
    assert_eq!(stored.support_count(), 3);
    assert!(matches!(
        stored.status(),
        AgentAnalystCallStatus::ParseFailed { .. }
    ));
}

#[test]
fn analyst_fanout_refuses_ambiguous_or_unknown_call_identity() {
    assert_eq!(
        AgentAnalystFanoutEvidence::new(["error-13-1".to_owned(), "error-13-1".to_owned(),])
            .unwrap_err(),
        AgentAnalystFanoutError::DuplicateCallManifest {
            call_id: "error-13-1".to_owned(),
        }
    );

    let empty_support = AgentAnalystCallEvidence::new(AgentAnalystCallEvidenceInput {
        call_id: "error-13-1".to_owned(),
        role: AgentAnalystRole::Error,
        source_task_ids: vec!["13-1".to_owned()],
        prompt: OutputRecord::inline("prompt"),
        response: None,
        status: AgentAnalystCallStatus::Pending,
        retry_count: 0,
        support_count: 0,
    })
    .unwrap_err();
    assert_eq!(empty_support, AgentAnalystCallError::EmptySupport);

    let mut fanout = AgentAnalystFanoutEvidence::new(["error-13-1".to_owned()]).unwrap();
    let unknown = fanout
        .push(
            AgentAnalystCallEvidence::new(AgentAnalystCallEvidenceInput {
                call_id: "error-59902".to_owned(),
                role: AgentAnalystRole::Error,
                source_task_ids: vec!["59902".to_owned()],
                prompt: OutputRecord::inline("prompt"),
                response: Some(OutputRecord::inline("response")),
                status: AgentAnalystCallStatus::Succeeded,
                retry_count: 0,
                support_count: 1,
            })
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        unknown,
        AgentAnalystFanoutError::UnknownCall {
            call_id: "error-59902".to_owned(),
        }
    );
}

fn spreadsheet_trajectory(task_id: &str, case_id: CaseId) -> AgentTrajectoryEvidence {
    let mut fingerprint_builder = FingerprintBuilder::new();
    fingerprint_builder.update("model=qwen3.5");
    let model_config_fingerprint = fingerprint_builder.finish();
    AgentTrajectoryEvidence::new(AgentTrajectoryEvidenceInput {
        session_id: AgentSessionId::new(),
        case_id: Some(case_id),
        task_id: task_id.to_owned(),
        outcome: AgentTrajectoryOutcome::Failure {
            reason: "spreadsheet answer mismatch".to_owned(),
        },
        model_id: "Qwen3.5-122B-A10B".to_owned(),
        model_config_fingerprint,
        transcript: OutputRecord::blob(BlobRef {
            store: "trace-blobs".to_owned(),
            key: format!("{task_id}.log"),
        }),
        commands: CommandEvidence::new(Vec::new()),
    })
    .with_analysis_records(vec![AgentTrajectoryAnalysisRecord::new(
        AgentTrajectoryAnalysisKind::Error,
        format!("error_analysis_{task_id}.md"),
        OutputRecord::blob(BlobRef {
            store: "analysis-blobs".to_owned(),
            key: format!("{task_id}.json"),
        }),
    )])
}
