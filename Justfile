set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

coverage_line_floor := '98.51'
coverage_branch_floor := '88.29'

lint:
    cargo fmt --check
    python3 scripts/lint-line-count.py
    cargo clippy --workspace --all-targets -- -D warnings

clippy: lint

test:
    python3 scripts/test-suite-sla.py --sla-seconds 30

test-one +args:
    cargo nextest run --workspace {{args}}

test-stress count +args:
    for i in $(seq 1 {{count}}); do echo "stress run $i/{{count}}"; cargo nextest run --workspace {{args}}; done

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
