use std::fs;
use std::path::Path;

use leaven_eval::SplitRole;
use leaven_evidence::{AgentTrajectoryAnalysisKind, AgentTrajectoryOutcome, OutputRecord};
use leaven_kernel::CaseId;
use trace2skill_spreadsheetbench::{
    Trace2SkillRunArtifactInput, build_training_corpus_from_run_artifacts,
    load_verified_400_manifest,
};

#[test]
fn builds_training_corpus_from_upstream_results_and_logs_without_model_work() {
    let root = unique_temp_dir("trace2skill-artifacts");
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

    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../tmp/repros/trace2skill-upstream/data/spreadsheetbench_verified/spreadsheetbench_verified_400/dataset.json",
    );
    let manifest = load_verified_400_manifest(&manifest_path).unwrap();
    let corpus = build_training_corpus_from_run_artifacts(
        &manifest,
        Trace2SkillRunArtifactInput {
            results_file: &results_file,
            log_dir: Some(&log_dir),
            log_format: "markdown",
            analysis_dir: Some(&analysis_dir),
        },
    )
    .unwrap();

    let train = manifest
        .splits
        .cases(&SplitRole::Train.partition_id())
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
        matches!(trajectory.transcript(), OutputRecord::BlobRef(reference) if reference.key.ends_with("cli_skill_preloaded_agent_13-1.md"))
    );
    assert_eq!(trajectory.analysis_records().len(), 1);
    assert_eq!(
        trajectory.analysis_records()[0].kind(),
        AgentTrajectoryAnalysisKind::Error
    );
    assert!(
        matches!(trajectory.analysis_records()[0].payload(), OutputRecord::BlobRef(reference) if reference.key.ends_with("error_analysis_13-1.md"))
    );
    assert_eq!(train[0], CaseId::from_index(0));
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
