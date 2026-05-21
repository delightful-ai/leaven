use std::{fs, process::Command};

#[test]
fn cli_inspects_one_case_as_json() {
    let fixture = Fixture::new();

    let output = Command::new(env!("CARGO_BIN_EXE_trace2skill_spreadsheetbench"))
        .arg("--inspect-one-case")
        .arg("--case")
        .arg(&fixture.case_file)
        .arg("--spreadsheet-dir")
        .arg(&fixture.spreadsheet_dir)
        .arg("--system-prompt")
        .arg(&fixture.system_prompt)
        .arg("--released-skill")
        .arg(&fixture.released_skill)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["case_id"], "13-1");
    assert_eq!(json["answer_sheet"], "LISTS");
    assert_eq!(json["answer_position"], "A3:D32");
    assert!(json["init_workbook"]["path"]
        .as_str()
        .unwrap()
        .ends_with("1_13-1_init.xlsx"));
    assert!(json["output_workbook"]
        .as_str()
        .unwrap()
        .ends_with("13-1_output.xlsx"));
}

#[test]
fn cli_renders_one_case_prompt() {
    let fixture = Fixture::new();

    let output = Command::new(env!("CARGO_BIN_EXE_trace2skill_spreadsheetbench"))
        .arg("--render-one-case-prompt")
        .arg("--case")
        .arg(&fixture.case_file)
        .arg("--spreadsheet-dir")
        .arg(&fixture.spreadsheet_dir)
        .arg("--system-prompt")
        .arg(&fixture.system_prompt)
        .arg("--released-skill")
        .arg(&fixture.released_skill)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let prompt = String::from_utf8(output.stdout).unwrap();
    assert!(prompt.contains("# Trace2Skill SpreadsheetBench Case 13-1"));
    assert!(prompt.contains("You are a spreadsheet expert."));
    assert!(prompt.contains("# xlsx\nUse workbook tooling."));
    assert!(prompt.contains("answer_position: A3:D32"));
    assert!(prompt.contains("13-1_output.xlsx"));
}

#[test]
fn cli_compares_one_case_answer_as_json() {
    let fixture = ExactCaseFixture::new();

    let output = Command::new(env!("CARGO_BIN_EXE_trace2skill_spreadsheetbench"))
        .arg("--compare-one-case-answer")
        .arg("--case")
        .arg(&fixture.case_file)
        .arg("--output-workbook")
        .arg(&fixture.golden_workbook)
        .arg("--golden-workbook")
        .arg(&fixture.golden_workbook)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["case_id"], "13-1");
    assert_eq!(json["total_cells"], 120);
    assert_eq!(json["matched_cells"], 120);
    assert_eq!(json["score"], 1.0);
    assert_eq!(json["passed"], true);
}

#[test]
fn cli_prepares_one_case_run_dir_as_json() {
    let fixture = Fixture::new();
    let run_dir = fixture.temp.path().join("run");

    let output = Command::new(env!("CARGO_BIN_EXE_trace2skill_spreadsheetbench"))
        .arg("--prepare-one-case-run")
        .arg("--case")
        .arg(&fixture.case_file)
        .arg("--spreadsheet-dir")
        .arg(&fixture.spreadsheet_dir)
        .arg("--system-prompt")
        .arg(&fixture.system_prompt)
        .arg("--released-skill")
        .arg(&fixture.released_skill)
        .arg("--run-dir")
        .arg(&run_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["case_id"], "13-1");
    assert_eq!(json["status"], "blocked_missing_live_spreadsheet_agent");
    assert!(json["prompt_file"]["path"]
        .as_str()
        .unwrap()
        .ends_with("agent_prompt.md"));
    assert!(json["manifest_file"]["path"]
        .as_str()
        .unwrap()
        .ends_with("manifest.json"));
    assert!(run_dir.join("agent_prompt.md").is_file());
    assert!(run_dir.join("manifest.json").is_file());
    assert!(run_dir.join("1_13-1_init.xlsx").is_file());
}

struct Fixture {
    temp: tempfile::TempDir,
    case_file: std::path::PathBuf,
    spreadsheet_dir: std::path::PathBuf,
    system_prompt: std::path::PathBuf,
    released_skill: std::path::PathBuf,
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

struct ExactCaseFixture {
    case_file: std::path::PathBuf,
    golden_workbook: std::path::PathBuf,
}

impl ExactCaseFixture {
    fn new() -> Self {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let spreadsheet_dir =
            repo.join("tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1");
        Self {
            case_file: repo.join(
                "tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json",
            ),
            golden_workbook: spreadsheet_dir.join("1_13-1_golden.xlsx"),
        }
    }
}
