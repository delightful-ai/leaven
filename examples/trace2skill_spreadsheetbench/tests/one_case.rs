use std::fs;

use trace2skill_spreadsheetbench::{
    Trace2SkillOneCaseInput, inspect_trace2skill_one_case, render_trace2skill_one_case_prompt,
};

#[test]
fn inspects_exact_one_case_artifacts_without_solving_workbook() {
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

    let inspection = inspect_trace2skill_one_case(Trace2SkillOneCaseInput {
        case_file: &case_file,
        spreadsheet_dir: &spreadsheet_dir,
        system_prompt_file: &system_prompt,
        released_skill_file: &released_skill,
    })
    .unwrap();

    assert_eq!(inspection.case_id, "13-1");
    assert_eq!(inspection.instruction_type, "Sheet-Level Manipulation");
    assert_eq!(inspection.answer_sheet.as_deref(), Some("LISTS"));
    assert_eq!(inspection.answer_position, "A3:D32");
    assert_eq!(inspection.init_workbook.bytes, 13);
    assert_eq!(inspection.golden_workbook.bytes, 13);
    assert_eq!(inspection.prompt.bytes, 25);
    assert_eq!(inspection.system_prompt.bytes, 29);
    assert_eq!(inspection.released_skill.bytes, 28);
    assert_eq!(
        inspection.output_workbook,
        spreadsheet_dir.join("13-1_output.xlsx")
    );
}

#[test]
fn renders_one_case_prompt_from_exact_case_and_upstream_prompt_fragments() {
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

    let prompt = render_trace2skill_one_case_prompt(Trace2SkillOneCaseInput {
        case_file: &case_file,
        spreadsheet_dir: &spreadsheet_dir,
        system_prompt_file: &system_prompt,
        released_skill_file: &released_skill,
    })
    .unwrap();

    assert!(prompt.contains("# Trace2Skill SpreadsheetBench Case 13-1"));
    assert!(prompt.contains("You are a spreadsheet expert."));
    assert!(prompt.contains("# xlsx\nUse workbook tooling."));
    assert!(prompt.contains("Populate the LISTS sheet from RANGES."));
    assert!(prompt.contains("answer_sheet: LISTS"));
    assert!(prompt.contains("answer_position: A3:D32"));
    assert!(prompt.contains("1_13-1_init.xlsx"));
    assert!(prompt.contains("13-1_output.xlsx"));
}
