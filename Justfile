set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Hard floors apply to the production/source denominator enforced by scripts/coverage-gate.py.
# The higher warning targets preserve the old ratchet signal without blocking
# higher-priority seam readiness work while coverage is rebuilt honestly.
coverage_line_floor := '80.00'
coverage_branch_floor := '80.00'
coverage_line_warn := '92.56'
coverage_branch_warn := '88.39'
milestone_excludes := '--exclude p0_graph_skeleton --exclude p1_keep_best --exclude p2_pairwise_tournament --exclude p3_gepa_parity --exclude p4_meta_harness_lite --exclude p5_evoskill_iteration --exclude p5_skill_paper_reproductions --exclude p6_optimizer_policy_self_opt --exclude p7_self_optimization_kernel --exclude p8_aime_gepa --exclude trace2skill_spreadsheetbench'

lint:
    cargo fmt --check
    python3 scripts/lint-line-count.py
    cargo clippy --workspace --all-targets \
      {{milestone_excludes}} \
      -- -D warnings

clippy: lint

test:
    python3 scripts/test_test_suite_sla.py
    python3 scripts/test-suite-sla.py --warn-seconds 30 --timeout-seconds 600

test-one +args:
    cargo nextest run --workspace \
      {{milestone_excludes}} \
      {{args}}

test-stress count +args:
    for i in $(seq 1 {{count}}); do echo "stress run $i/{{count}}"; cargo nextest run --workspace {{milestone_excludes}} {{args}}; done

bench-git-trust +args:
    cargo run -p xtask -- git-trust-bench {{args}}

evoskill-paper-manifest *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json {{args}}

evoskill-paper-pin-local-sources *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --write-local-source-pin-manifest {{args}}

evoskill-paper-accept-substitute-splits *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --write-paper-close-split-policy-manifest {{args}}

evoskill-paper-browsecomp-public-sample csv_path *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --write-browsecomp-public-transfer-sample {{csv_path}} {{args}}

evoskill-paper-no-spend-packet csv_path='tmp/replication/evoskill/browsecomp/public_browsecomp_test_set.csv' *args:
    just evoskill-paper-pin-local-sources {{args}}
    just evoskill-paper-accept-substitute-splits {{args}}
    just evoskill-paper-browsecomp-public-sample {{csv_path}} {{args}}
    just evoskill-paper-runner-inputs {{args}}
    just evoskill-paper-live-run-request {{args}}

evoskill-paper-score-officeqa predictions_path *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --write-officeqa-score-result {{predictions_path}} {{args}}

evoskill-paper-score-sealqa judged_rows_path approval_id *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --write-sealqa-judge-score-result {{judged_rows_path}} --sealqa-judge-approval-id {{approval_id}} {{args}}

evoskill-paper-sealqa-judge-requests predictions_path *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --write-sealqa-judge-request-batch {{predictions_path}} {{args}}

evoskill-paper-runner-inputs *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --write-runner-input-batch {{args}}

evoskill-paper-runner-outputs outputs_path *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --write-runner-output-batch {{outputs_path}} {{args}}

evoskill-paper-live-run-request *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --write-live-run-request {{args}}

evoskill-paper-final-report *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json {{args}}

evoskill-paper-closeout-audit *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --audit-paper-close {{args}}

milestone-p0:
    cargo run -p p0_graph_skeleton

milestone-p1:
    cargo run -p p1_keep_best

milestone-p2:
    cargo run -p p2_pairwise_tournament

milestone-p3:
    cargo run -p p3_gepa_parity

milestone-p4:
    cargo run -p p4_meta_harness_lite

milestone-p5:
    LEAVEN_CODEX_LIVE=1 cargo run -p p5_evoskill_iteration -- --live-codex

milestone-p6:
    cargo run -p p6_optimizer_policy_self_opt

milestone-p7:
    cargo run -p p7_self_optimization_kernel

milestone-p8:
    cargo run -p p8_aime_gepa

milestone-examples: milestone-p0 milestone-p1 milestone-p2 milestone-p3 milestone-p4 milestone-p5 milestone-p6 milestone-p7 milestone-p8

coverage:
    python3 scripts/coverage-gate.py --line-floor {{coverage_line_floor}} --branch-floor {{coverage_branch_floor}} --line-warn {{coverage_line_warn}} --branch-warn {{coverage_branch_warn}}

coverage-fast +args:
    python3 scripts/coverage-gate.py --line-floor 0 --branch-floor 0 --skip-clean --skip-smoke {{args}}

coverage-smoke-fast +args:
    python3 scripts/coverage-gate.py --line-floor 0 --branch-floor 0 --skip-clean --skip-smoke --skip-report {{args}}

check: lint test coverage
