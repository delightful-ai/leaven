use std::fs;
use std::path::{Path, PathBuf};

use leaven_eval::SplitRole;
use leaven_evidence::{
    AgentAnalystRole, AgentTrajectoryAnalysisKind, AgentTrajectoryCorpusEvidence,
    AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput, AgentTrajectoryOutcome, CommandEvidence,
    OutputRecord,
};
use leaven_kernel::{AgentSessionId, CaseId, Fingerprint};
use trace2skill_spreadsheetbench::{
    Trace2SkillRunArtifactInput, Trace2SkillStage2AnalystFanoutInput,
    build_stage2_analyst_fanout_from_training_corpus, build_training_corpus_from_run_artifacts,
    load_verified_400_manifest,
};

#[test]
fn builds_training_corpus_from_upstream_results_and_logs_without_model_work() {
    let root = unique_temp_dir("trace2skill-artifacts");
    let fixture = write_upstream_run_fixture(&root);

    let manifest_path = fixture_manifest_path(&root);
    let manifest = load_verified_400_manifest(&manifest_path).unwrap();
    let corpus = build_training_corpus_from_run_artifacts(
        &manifest,
        Trace2SkillRunArtifactInput {
            results_file: &fixture.results_file,
            log_dir: Some(&fixture.log_dir),
            log_format: "markdown",
            analysis_dir: Some(&fixture.analysis_dir),
        },
    )
    .unwrap();

    let train = manifest
        .split_manifest
        .cases_for_role(&SplitRole::Train)
        .unwrap();
    assert_eq!(corpus.expected_task_ids().len(), 200);
    assert_eq!(corpus.expected_task_ids()[0], "13-1");
    assert_eq!(corpus.expected_task_ids()[199], "52575");
    assert_eq!(corpus.completed_task_ids(), vec!["13-1"]);
    assert_eq!(corpus.pending_task_ids().len(), 199);

    let trajectory = corpus.by_task("13-1")[0];
    assert_eq!(trajectory.case_id(), Some(CaseId::from_index(0)));
    assert_eq!(
        trajectory.outcome(),
        &AgentTrajectoryOutcome::Failure {
            reason: "answer mismatch".to_owned(),
        }
    );
    assert_eq!(trajectory.model_id(), "Qwen3.5-122B-A10B");
    assert!(
        matches!(trajectory.transcript(), OutputRecord::BlobRef { reference, .. } if reference.key.ends_with("cli_skill_preloaded_agent_13-1.md"))
    );
    assert_eq!(trajectory.analysis_records().len(), 1);
    assert_eq!(
        trajectory.analysis_records()[0].kind(),
        AgentTrajectoryAnalysisKind::Error
    );
    assert!(
        matches!(trajectory.analysis_records()[0].payload(), OutputRecord::BlobRef { reference, .. } if reference.key.ends_with("error_analysis_13-1.md"))
    );
    assert_eq!(train[0], CaseId::from_index(0));

    let prompt_dir = root.join("upstream-prompts");
    write_prompt_templates(&prompt_dir);
    let fanout =
        build_stage2_analyst_fanout_from_training_corpus(Trace2SkillStage2AnalystFanoutInput {
            corpus: &corpus,
            upstream_prompt_dir: &prompt_dir,
        })
        .unwrap();
    assert_eq!(fanout.expected_call_ids(), ["error-13-1-1"]);
    let call = fanout.by_call("error-13-1-1").unwrap();
    assert_eq!(call.role(), AgentAnalystRole::Error);
    assert_eq!(call.source_task_ids(), ["13-1"]);
    assert_eq!(call.support_count(), 1);
    assert_eq!(call.retry_count(), 0);
    assert_eq!(fanout.pending_call_ids(), vec!["error-13-1-1"]);
}

#[test]
fn stage2_analyst_call_ids_disambiguate_duplicate_task_trajectories() {
    let root = unique_temp_dir("trace2skill-duplicate-prompts");
    let mut corpus = AgentTrajectoryCorpusEvidence::new(["13-1".to_owned()]).unwrap();
    corpus.push(trajectory("13-1")).unwrap();
    corpus.push(trajectory("13-1")).unwrap();

    let prompt_dir = root.join("upstream-prompts");
    write_prompt_templates(&prompt_dir);
    let fanout =
        build_stage2_analyst_fanout_from_training_corpus(Trace2SkillStage2AnalystFanoutInput {
            corpus: &corpus,
            upstream_prompt_dir: &prompt_dir,
        })
        .unwrap();

    assert_eq!(fanout.expected_call_ids(), ["error-13-1-1", "error-13-1-2"]);
}

#[test]
fn stage2_corpus_fanout_embeds_upstream_prompt_sources() {
    let root = unique_temp_dir("trace2skill-corpus-prompts");
    let prompt_dir = root.join("upstream-prompts");
    write_prompt_templates(&prompt_dir);
    let mut corpus =
        AgentTrajectoryCorpusEvidence::new(["13-1".to_owned(), "14-1".to_owned()]).unwrap();
    corpus.push(trajectory("13-1")).unwrap();
    corpus
        .push(trajectory_with_outcome(
            "14-1",
            AgentTrajectoryOutcome::Success,
        ))
        .unwrap();

    let fanout =
        build_stage2_analyst_fanout_from_training_corpus(Trace2SkillStage2AnalystFanoutInput {
            corpus: &corpus,
            upstream_prompt_dir: &prompt_dir,
        })
        .unwrap();

    let error_call = fanout.by_call("error-13-1-1").unwrap();
    let error_prompt = inline_text(error_call.prompt());
    assert!(error_prompt.contains("ParallelSkillEvolver._build_map_system_prompt"));
    assert!(error_prompt.contains("UPSTREAM ERROR RECORD SECTION"));
    assert!(error_prompt.contains("UPSTREAM MAP OUTPUT FORMAT"));
    assert!(error_prompt.contains("task_id: 13-1"));
    assert!(error_prompt.contains("This pending fan-out has not executed an analyst model call."));

    let success_call = fanout.by_call("success-14-1-2").unwrap();
    let success_prompt = inline_text(success_call.prompt());
    assert!(success_prompt.contains("SuccessParallelSkillEvolver._build_map_system_prompt"));
    assert!(success_prompt.contains("UPSTREAM SUCCESS RECORD SECTION"));
    assert!(success_prompt.contains("task_id: 14-1"));
}

fn trajectory(task_id: &str) -> AgentTrajectoryEvidence {
    trajectory_with_outcome(
        task_id,
        AgentTrajectoryOutcome::Failure {
            reason: "answer mismatch".to_owned(),
        },
    )
}

fn trajectory_with_outcome(
    task_id: &str,
    outcome: AgentTrajectoryOutcome,
) -> AgentTrajectoryEvidence {
    AgentTrajectoryEvidence::new(AgentTrajectoryEvidenceInput {
        session_id: AgentSessionId::new(),
        case_id: None,
        task_id: task_id.to_owned(),
        outcome,
        model_id: "test-model".to_owned(),
        model_config_fingerprint: Fingerprint::from_bytes([0x13; 32]),
        transcript: OutputRecord::inline("transcript"),
        commands: CommandEvidence::new(Vec::new()),
    })
}

struct UpstreamRunFixture {
    results_file: PathBuf,
    log_dir: PathBuf,
    analysis_dir: PathBuf,
}

fn write_upstream_run_fixture(root: &Path) -> UpstreamRunFixture {
    let results_file = root.join("results.json");
    let log_dir = root.join("logs");
    let analysis_dir = root.join("analysis");
    fs::create_dir_all(&log_dir).unwrap();
    fs::create_dir_all(&analysis_dir).unwrap();
    fs::write(
        &results_file,
        r#"{
          "agent_name": "cli_skill_preloaded_agent",
          "model": "Qwen3.5-122B-A10B",
          "seed": 41,
          "results": [
            {
              "id": "13-1",
              "instruction": "spreadsheet task",
              "success": false,
              "error": "answer mismatch",
              "test_cases": [
                {
                  "input_file": "1_13-1_init.xlsx",
                  "output_file": "outputs/spreadsheet/13-1/1_13-1_init.xlsx",
                  "success": false,
                  "agent_answer": "wrong",
                  "turns": 17,
                  "error": "cell mismatch"
                }
              ]
            }
          ]
        }"#,
    )
    .unwrap();
    fs::write(
        log_dir.join("cli_skill_preloaded_agent_13-1.md"),
        "# chat log",
    )
    .unwrap();
    fs::write(
        analysis_dir.join("error_analysis_13-1.md"),
        "## Error Analysis\n- range mismatch",
    )
    .unwrap();
    UpstreamRunFixture {
        results_file,
        log_dir,
        analysis_dir,
    }
}

fn inline_text(record: &OutputRecord) -> &str {
    let OutputRecord::Inline {
        text, truncated, ..
    } = record
    else {
        panic!("expected inline prompt")
    };
    assert!(!truncated);
    text
}

fn write_prompt_templates(root: &Path) {
    fs::create_dir_all(root.join("skill_evolving_agent")).unwrap();
    fs::create_dir_all(root.join("parallel_evolving_agent")).unwrap();
    fs::create_dir_all(root.join("success_evolving_agent")).unwrap();
    fs::write(
        root.join("skill_evolving_agent/system_prompt_base.txt"),
        "UPSTREAM SYSTEM PROMPT BASE\n## Output Format\n",
    )
    .unwrap();
    fs::write(
        root.join("parallel_evolving_agent/map_output_format.txt"),
        "UPSTREAM MAP OUTPUT FORMAT",
    )
    .unwrap();
    fs::write(
        root.join("skill_evolving_agent/modification_strategies_section.txt"),
        "UPSTREAM ERROR STRATEGIES",
    )
    .unwrap();
    fs::write(
        root.join("skill_evolving_agent/error_record_section_skill.txt"),
        "UPSTREAM ERROR RECORD SECTION",
    )
    .unwrap();
    for file in [
        "error_analysis_records_header",
        "current_skill_folder_header",
        "skill_folder_size_status_header",
        "skill_md_status_line",
        "reference_files_status_line",
        "size_warning",
    ] {
        fs::write(
            root.join(format!("skill_evolving_agent/{file}.txt")),
            format!("UPSTREAM {file}"),
        )
        .unwrap();
    }
    for (file, contents) in [
        ("success_record_section", "UPSTREAM SUCCESS RECORD SECTION"),
        (
            "success_modification_strategies_section",
            "UPSTREAM SUCCESS STRATEGIES",
        ),
        ("success_intro_replacement", "UPSTREAM SUCCESS INTRO"),
        ("success_input_replacement", "UPSTREAM SUCCESS INPUT"),
        ("success_goal_replacement", "UPSTREAM SUCCESS GOAL"),
        (
            "success_first_constraint_replacement",
            "UPSTREAM SUCCESS FIRST CONSTRAINT",
        ),
        (
            "success_traceability_constraint",
            "UPSTREAM SUCCESS TRACEABILITY",
        ),
        (
            "success_output_reasoning_replacement",
            "UPSTREAM SUCCESS OUTPUT REASONING",
        ),
        (
            "success_analysis_records_header",
            "UPSTREAM SUCCESS ANALYSIS HEADER",
        ),
        (
            "current_skill_folder_header",
            "UPSTREAM SUCCESS SKILL HEADER",
        ),
        (
            "skill_folder_size_status_header",
            "UPSTREAM SUCCESS SIZE HEADER",
        ),
        ("skill_md_status_line", "UPSTREAM SUCCESS SKILL STATUS"),
        (
            "reference_files_status_line",
            "UPSTREAM SUCCESS REFERENCE STATUS",
        ),
        ("size_warning", "UPSTREAM SUCCESS SIZE WARNING"),
    ] {
        fs::write(
            root.join(format!("success_evolving_agent/{file}.txt")),
            contents,
        )
        .unwrap();
    }
}

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn fixture_manifest_path(root: &PathBuf) -> PathBuf {
    let path = root.join("dataset.json");
    let rows = (0..400)
        .map(|index| {
            let id = match index {
                0 => "13-1".to_owned(),
                199 => "52575".to_owned(),
                399 => "59902".to_owned(),
                _ => format!("row-{index}"),
            };
            serde_json::json!({
                "id": id,
                "instruction": format!("task {index}"),
                "spreadsheet_path": format!("spreadsheet/{index}"),
                "instruction_type": "synthetic",
                "answer_position": "A1:A1",
                "answer_sheet": null,
                "data_position": null,
            })
        })
        .collect::<Vec<_>>();
    fs::write(&path, serde_json::to_vec(&rows).unwrap()).unwrap();
    path
}
