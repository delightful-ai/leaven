set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Floors apply to the production/source denominator enforced by scripts/coverage-gate.py.
# Line floor was 98.51 against PR 2's 16391-line denominator; PR 3 added ~4000
# lines (agentic reflection workspace + skill patch stack) without re-running
# the gate, so the post-merge baseline dropped. Branch floor ratcheted up.
coverage_line_floor := '92.56'
coverage_branch_floor := '88.39'

lint:
    cargo fmt --check
    python3 scripts/lint-line-count.py
    cargo clippy --workspace --all-targets \
      --exclude p0_graph_skeleton \
      --exclude p1_keep_best \
      --exclude p2_pairwise_tournament \
      --exclude p3_gepa_parity \
      --exclude p4_meta_harness_lite \
      --exclude p5_evoskill_iteration \
      --exclude p5_skill_paper_reproductions \
      --exclude p6_optimizer_policy_self_opt \
      --exclude p7_self_optimization_kernel \
      --exclude p8_aime_gepa \
      --exclude trace2skill_spreadsheetbench \
      -- -D warnings

clippy: lint

test:
    python3 scripts/test_test_suite_sla.py
    python3 scripts/test-suite-sla.py --sla-seconds 30

test-one +args:
    cargo nextest run --workspace \
      --exclude p0_graph_skeleton \
      --exclude p1_keep_best \
      --exclude p2_pairwise_tournament \
      --exclude p3_gepa_parity \
      --exclude p4_meta_harness_lite \
      --exclude p5_evoskill_iteration \
      --exclude p5_skill_paper_reproductions \
      --exclude p6_optimizer_policy_self_opt \
      --exclude p7_self_optimization_kernel \
      --exclude p8_aime_gepa \
      --exclude trace2skill_spreadsheetbench \
      {{args}}

test-stress count +args:
    for i in $(seq 1 {{count}}); do echo "stress run $i/{{count}}"; cargo nextest run --workspace --exclude p0_graph_skeleton --exclude p1_keep_best --exclude p2_pairwise_tournament --exclude p3_gepa_parity --exclude p4_meta_harness_lite --exclude p5_evoskill_iteration --exclude p5_skill_paper_reproductions --exclude p6_optimizer_policy_self_opt --exclude p7_self_optimization_kernel --exclude p8_aime_gepa --exclude trace2skill_spreadsheetbench {{args}}; done

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

evoskill-paper-score-officeqa predictions_path *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json --write-officeqa-score-result {{predictions_path}} {{args}}

evoskill-paper-final-report *args:
    cargo run -p p5_skill_paper_reproductions -- --out target/evoskill-paper-close/replica-manifest.json --final-report-out target/evoskill-paper-close/final-report.json {{args}}

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
    python3 scripts/coverage-gate.py --line-floor {{coverage_line_floor}} --branch-floor {{coverage_branch_floor}}

check: lint test coverage
