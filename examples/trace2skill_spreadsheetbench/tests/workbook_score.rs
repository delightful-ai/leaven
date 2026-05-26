use trace2skill_spreadsheetbench::{
    Trace2SkillOneCaseComparisonInput, compare_trace2skill_one_case_answer,
};

mod support;
use support::ExactCaseFixture;

#[test]
fn scores_golden_workbook_answer_range_as_perfect_match() {
    let fixture = ExactCaseFixture::new();

    let report = compare_trace2skill_one_case_answer(Trace2SkillOneCaseComparisonInput {
        case_file: &fixture.case_file,
        candidate_workbook: &fixture.golden_workbook,
        golden_workbook: &fixture.golden_workbook,
    })
    .unwrap();

    assert_eq!(report.case_id, "13-1");
    assert_eq!(report.answer_sheet.as_deref(), Some("LISTS"));
    assert_eq!(report.answer_position, "A3:D32");
    assert_eq!(report.total_cells, 120);
    assert_eq!(report.matched_cells, 120);
    assert!((report.score - 1.0).abs() < f64::EPSILON);
    assert!(report.passed);
    assert_eq!(report.mismatches, []);
}

#[test]
fn detects_init_workbook_mismatches_against_golden_answer_range() {
    let fixture = ExactCaseFixture::new();

    let report = compare_trace2skill_one_case_answer(Trace2SkillOneCaseComparisonInput {
        case_file: &fixture.case_file,
        candidate_workbook: &fixture.init_workbook,
        golden_workbook: &fixture.golden_workbook,
    })
    .unwrap();

    assert_eq!(report.case_id, "13-1");
    assert_eq!(report.total_cells, 120);
    assert!(report.matched_cells < report.total_cells);
    assert!(report.score < 1.0);
    assert!(!report.passed);
    assert!(!report.mismatches.is_empty());
    assert!(report.mismatches.iter().all(|cell| {
        cell.address
            .chars()
            .next()
            .is_some_and(|column| matches!(column, 'A' | 'B' | 'C' | 'D'))
    }));
}
