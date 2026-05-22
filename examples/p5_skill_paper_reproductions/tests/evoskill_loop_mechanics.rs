use std::fmt::Write as _;
use std::fs;

use p5_skill_paper_reproductions::evoskill::{ManifestBuildInput, run_evoskill_replica_mechanics};

#[test]
fn no_spend_loop_exercises_frontier_lineage_feedback_and_checkpoint_resume() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);

    let report = run_evoskill_replica_mechanics(&ManifestBuildInput::new(root.path())).unwrap();

    assert_eq!(report.iterations.len(), 4);
    assert_eq!(report.feedback_history_rows, 8);
    assert_eq!(report.checkpoint_resume.after_iteration, 2);
    assert_eq!(
        report.checkpoint_resume.frontier_before,
        report.checkpoint_resume.frontier_after
    );
    assert_eq!(
        report.checkpoint_resume.parent_selector_cursor_before,
        report.checkpoint_resume.parent_selector_cursor_after
    );
    assert_eq!(report.frontier_capacity, 3);
    assert_eq!(report.final_frontier_members.len(), 3);

    assert_eq!(report.iterations[0].train_sample_rows, 2);
    assert_eq!(report.iterations[0].feedback_rows_seen, 0);
    assert_eq!(report.iterations[1].feedback_rows_seen, 2);
    assert_eq!(report.iterations[2].feedback_rows_seen, 4);
    assert_eq!(report.iterations[3].feedback_rows_seen, 6);

    assert!(report.iterations.iter().any(|iteration| iteration.admitted));
    assert!(
        report
            .iterations
            .iter()
            .any(|iteration| !iteration.admitted)
    );
    assert!(
        report
            .iterations
            .iter()
            .all(|iteration| iteration.child_revision != iteration.parent_revision)
    );
    assert!(
        report
            .iterations
            .iter()
            .all(|iteration| iteration.change_expected_parent == iteration.parent_revision)
    );

    let selected_parents = report
        .iterations
        .iter()
        .map(|iteration| iteration.selected_parent)
        .collect::<Vec<_>>();
    assert_ne!(
        selected_parents[0], selected_parents[1],
        "round-robin parent selection must advance after the first admitted child"
    );
}

fn write_officeqa_full_csv(root: &std::path::Path, rows: usize) {
    let path = root.join("tmp/repros/officeqa/officeqa_full.csv");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut csv = String::from("uid,question,answer,source_docs,source_files,difficulty\n");
    for index in 0..rows {
        let uid = format!("UID{:04}", index + 1);
        let difficulty = if index < 113 { "easy" } else { "hard" };
        writeln!(
            csv,
            "{uid},Question {}?,Answer {},https://example.test/doc{},treasury_{:04}.txt,{difficulty}",
            index + 1,
            index + 1,
            index + 1,
            index + 1
        )
        .unwrap();
    }
    fs::write(path, csv).unwrap();
}
