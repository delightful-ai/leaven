use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use p5_skill_paper_reproductions::evoskill::{
    ExactnessClass, FinalScoreStatus, ManifestBuildInput, build_evoskill_final_report,
};

#[test]
fn final_report_exposes_score_slots_costs_errors_and_gaps_without_fake_metrics() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);

    let report = build_evoskill_final_report(&ManifestBuildInput::new(root.path())).unwrap();

    assert_eq!(report.exactness, ExactnessClass::BlockedBeforePaperClose);
    assert_eq!(report.cost.llm_calls, 0);
    assert_eq!(report.cost.metric_calls, 0);
    assert_eq!(report.cost.prompt_tokens, 0);
    assert_eq!(report.cost.completion_tokens, 0);
    assert!(
        report
            .loop_report
            .as_ref()
            .expect("OfficeQA mechanics loop should run when the CSV is present")
            .proxy_rejection
            .contains("mechanics only")
    );

    let officeqa = report
        .manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "officeqa")
        .expect("OfficeQA materialization is embedded");
    assert_eq!(officeqa.source_rows, Some(246));
    assert_eq!(
        officeqa.source_row_fingerprint.as_deref().unwrap().len(),
        64
    );
    assert_eq!(
        report.manifest.scorer.id,
        report.scorer_fingerprint.scorer_id
    );

    let train_12_slots = report
        .score_slots
        .iter()
        .filter(|slot| {
            slot.dataset_id == "officeqa" && slot.split_id == "officeqa_difficulty_train_12_val_17"
        })
        .collect::<Vec<_>>();
    assert_eq!(train_12_slots.len(), 6);
    for role in ["train", "validation", "held_out_test"] {
        for candidate in ["baseline", "optimized"] {
            let slot = train_12_slots
                .iter()
                .find(|slot| slot.split_role == role && slot.candidate_role == candidate)
                .unwrap_or_else(|| panic!("missing {candidate} {role} slot"));
            assert_eq!(slot.status, FinalScoreStatus::Blocked);
            assert!(
                slot.score.is_none(),
                "blocked slots must not fake zero scores"
            );
            assert!(
                slot.blocker_ids
                    .contains(&"officeqa_exact_split_membership".to_owned())
            );
        }
    }

    assert!(
        report
            .errors
            .iter()
            .any(|error| error.blocker_id == "sealqa_parquet_row_reader")
    );
    assert!(
        report
            .ablations
            .iter()
            .any(|ablation| ablation.id == "skill_merge" && ablation.status == "blocked")
    );
}

#[test]
fn cli_writes_manifest_and_final_report_artifacts() {
    let root = tempfile::tempdir().unwrap();
    write_officeqa_full_csv(root.path(), 246);
    let manifest_path = root.path().join("out/replica-manifest.json");
    let report_path = root.path().join("out/final-report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_p5_skill_paper_reproductions"))
        .arg("--root")
        .arg(root.path())
        .arg("--out")
        .arg(&manifest_path)
        .arg("--final-report-out")
        .arg(&report_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(manifest_path.exists());
    let report = fs::read_to_string(report_path).unwrap();
    assert!(report.contains("\"score_slots\""));
    assert!(report.contains("\"blocked_before_paper_close\""));
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
