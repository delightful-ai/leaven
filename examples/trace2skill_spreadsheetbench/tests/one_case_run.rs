use std::fs;

use trace2skill_spreadsheetbench::{
    prepare_trace2skill_one_case_run, score_trace2skill_one_case_run, Trace2SkillOneCaseInput,
    Trace2SkillOneCaseRunInput, Trace2SkillOneCaseRunScoringInput, Trace2SkillOneCaseRunStatus,
};

#[test]
fn prepares_run_dir_with_prompt_manifest_and_staged_workbooks() {
    let fixture = Fixture::new();
    let run_dir = fixture.temp.path().join("run");

    let report = prepare_trace2skill_one_case_run(Trace2SkillOneCaseRunInput {
        case: Trace2SkillOneCaseInput {
            case_file: &fixture.case_file,
            spreadsheet_dir: &fixture.spreadsheet_dir,
            system_prompt_file: &fixture.system_prompt,
            released_skill_file: &fixture.released_skill,
        },
        run_dir: &run_dir,
        output_workbook: None,
    })
    .unwrap();

    assert_eq!(report.case_id, "13-1");
    assert_eq!(
        report.status,
        Trace2SkillOneCaseRunStatus::BlockedMissingLiveSpreadsheetAgent
    );
    assert_eq!(
        report.missing_primitive.as_deref(),
        Some("live_spreadsheet_agent_execution")
    );
    assert_eq!(report.output_workbook, run_dir.join("13-1_output.xlsx"));
    assert_eq!(report.score_report, None);

    assert_eq!(
        fs::read(run_dir.join("1_13-1_init.xlsx")).unwrap(),
        b"init workbook"
    );
    assert_eq!(
        fs::read(run_dir.join("1_13-1_golden.xlsx")).unwrap(),
        b"gold workbook"
    );
    assert!(!report.output_workbook.exists());

    let prompt = fs::read_to_string(run_dir.join("agent_prompt.md")).unwrap();
    assert!(prompt.contains("# Trace2Skill SpreadsheetBench Case 13-1"));
    assert!(prompt.contains("You are a spreadsheet expert."));
    assert!(prompt.contains("# xlsx\nUse workbook tooling."));
    assert!(prompt.contains("Populate the LISTS sheet from RANGES."));
    assert!(prompt.contains(&format!("working_directory: {}", run_dir.display())));
    assert!(prompt.contains(&format!(
        "spreadsheet_path: {}",
        run_dir.join("1_13-1_init.xlsx").display()
    )));
    assert!(prompt.contains(&format!(
        "output_path: {}",
        run_dir.join("13-1_output.xlsx").display()
    )));
    assert!(prompt.contains("instruction_type: Sheet-Level Manipulation"));
    assert!(!prompt.contains("golden"));

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["case_id"], "13-1");
    assert_eq!(manifest["status"], "blocked_missing_live_spreadsheet_agent");
    assert_eq!(
        manifest["missing_primitive"],
        "live_spreadsheet_agent_execution"
    );
    assert!(manifest["score_report"].is_null());
    assert!(manifest["source_artifacts"]["case_file"]
        .as_str()
        .unwrap()
        .ends_with("dataset_first_case.json"));
}

#[test]
fn scores_prepared_run_dir_and_writes_trajectory_evidence() {
    let fixture = ExactCaseFixture::new();
    let temp = tempfile::tempdir().unwrap();
    let run_dir = temp.path().join("run");
    prepare_trace2skill_one_case_run(Trace2SkillOneCaseRunInput {
        case: Trace2SkillOneCaseInput {
            case_file: &fixture.case_file,
            spreadsheet_dir: &fixture.spreadsheet_dir,
            system_prompt_file: &fixture.system_prompt,
            released_skill_file: &fixture.released_skill,
        },
        run_dir: &run_dir,
        output_workbook: None,
    })
    .unwrap();
    fs::copy(
        run_dir.join("1_13-1_golden.xlsx"),
        run_dir.join("13-1_output.xlsx"),
    )
    .unwrap();
    let transcript_file = run_dir.join("agent_transcript.md");
    fs::write(&transcript_file, "ACTION: TASK_COMPLETE\n").unwrap();

    let report = score_trace2skill_one_case_run(Trace2SkillOneCaseRunScoringInput {
        run_dir: &run_dir,
        model_id: "fixture-spreadsheet-agent",
        transcript_file: &transcript_file,
    })
    .unwrap();

    assert_eq!(report.case_id, "13-1");
    assert_eq!(
        report.status,
        Trace2SkillOneCaseRunStatus::ScoredCandidateWorkbook
    );
    assert!((report.score_report.score - 1.0).abs() < f64::EPSILON);
    assert!(report.score_report.passed);
    assert_eq!(report.score_report.matched_cells, 120);
    assert!(report.score_file.path.ends_with("score_report.json"));
    assert!(report.trajectory_file.path.ends_with("trajectory.json"));

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "scored_candidate_workbook");
    assert!(manifest["missing_primitive"].is_null());
    assert_eq!(manifest["score_report"]["score"], 1.0);

    let trajectory: serde_json::Value =
        serde_json::from_slice(&fs::read(run_dir.join("trajectory.json")).unwrap()).unwrap();
    assert_eq!(trajectory["task_id"], "13-1");
    assert_eq!(trajectory["model_id"], "fixture-spreadsheet-agent");
    assert_eq!(trajectory["outcome"], "Success");
    assert!(trajectory["analysis_records"][0]["source_file"]
        .as_str()
        .unwrap()
        .ends_with("score_report.json"));
}

struct Fixture {
    temp: tempfile::TempDir,
    case_file: std::path::PathBuf,
    spreadsheet_dir: std::path::PathBuf,
    system_prompt: std::path::PathBuf,
    released_skill: std::path::PathBuf,
}

struct ExactCaseFixture {
    case_file: std::path::PathBuf,
    spreadsheet_dir: std::path::PathBuf,
    system_prompt: std::path::PathBuf,
    released_skill: std::path::PathBuf,
}

impl ExactCaseFixture {
    fn new() -> Self {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        Self {
            case_file: repo.join(
                "tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json",
            ),
            spreadsheet_dir: repo.join(
                "tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1",
            ),
            system_prompt: repo.join(
                "tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt",
            ),
            released_skill: repo.join(
                "tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/SKILL.md",
            ),
        }
    }
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let case_file = temp.path().join("dataset_first_case.json");
        let spreadsheet_dir = temp.path().join("13-1");
        let system_prompt = temp.path().join("system_prompt.txt");
        let released_skill = temp.path().join("SKILL.md");
        fs::create_dir_all(&spreadsheet_dir).unwrap();
        fs::write(
            &case_file,
            r#"{
              "answer_position": "A3:D32",
              "answer_sheet": "LISTS",
              "data_position": "A1:E56",
              "id": "13-1",
              "instruction": "Populate the LISTS sheet from RANGES.",
              "instruction_type": "Sheet-Level Manipulation",
              "spreadsheet_path": "spreadsheet/13-1"
            }"#,
        )
        .unwrap();
        fs::write(
            spreadsheet_dir.join("prompt.txt"),
            "Populate the LISTS sheet.",
        )
        .unwrap();
        fs::write(spreadsheet_dir.join("1_13-1_init.xlsx"), b"init workbook").unwrap();
        fs::write(spreadsheet_dir.join("1_13-1_golden.xlsx"), b"gold workbook").unwrap();
        fs::write(&system_prompt, "You are a spreadsheet expert.").unwrap();
        fs::write(&released_skill, "# xlsx\nUse workbook tooling.").unwrap();

        Self {
            temp,
            case_file,
            spreadsheet_dir,
            system_prompt,
            released_skill,
        }
    }
}
