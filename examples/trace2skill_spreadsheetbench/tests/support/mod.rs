#![allow(dead_code)]

use std::{fs, path::PathBuf};

use tempfile::TempDir;
use trace2skill_spreadsheetbench::Trace2SkillOneCaseInput;

pub struct Fixture {
    pub temp: TempDir,
    pub case_file: PathBuf,
    pub spreadsheet_dir: PathBuf,
    pub system_prompt: PathBuf,
    pub released_skill: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
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

pub struct ExactCaseFixture {
    pub case_file: PathBuf,
    pub spreadsheet_dir: PathBuf,
    pub system_prompt: PathBuf,
    pub released_skill: PathBuf,
    pub init_workbook: PathBuf,
    pub golden_workbook: PathBuf,
}

impl ExactCaseFixture {
    pub fn new() -> Self {
        let repo = workspace_root();
        let spreadsheet_dir =
            repo.join("tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/13-1");
        Self {
            case_file: repo.join(
                "tmp/paper_exact_samples/trace2skill/spreadsheetbench_verified/dataset_first_case.json",
            ),
            spreadsheet_dir: spreadsheet_dir.clone(),
            system_prompt: repo.join(
                "tmp/repros/trace2skill-upstream/spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt",
            ),
            released_skill: repo.join(
                "tmp/repros/trace2skill-upstream/released_skills/trace2skill-xlsx-35B-combined/SKILL.md",
            ),
            init_workbook: spreadsheet_dir.join("1_13-1_init.xlsx"),
            golden_workbook: spreadsheet_dir.join("1_13-1_golden.xlsx"),
        }
    }

    pub fn case_input(&self) -> Trace2SkillOneCaseInput<'_> {
        Trace2SkillOneCaseInput {
            case_file: &self.case_file,
            spreadsheet_dir: &self.spreadsheet_dir,
            system_prompt_file: &self.system_prompt,
            released_skill_file: &self.released_skill,
        }
    }
}

fn workspace_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
