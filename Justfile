set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

coverage_ignore := '(^|/)(tests|target)/|/src/main\.rs$|crates/leaven-(agent.*|artifact.*|artifacts|cuda|derive|dsrs|gepa|kernel|lm.*|mipro|python|render|std|store-(file|object|sqlite)|surface|textgrad|trace|workspace.*)/|crates/leaven-core/src/(evaluation|proposal)\.rs|crates/leaven-engine/src/context/render_context\.rs|crates/leaven-engine/src/stage/(evaluator|population|preference|proposer|renderer|stopper)\.rs'
coverage_line_floor := '98.0'
coverage_branch_floor := '85.0'

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
    cargo run -p p5_skill_paper_reproductions

milestone-examples: milestone-p0 milestone-p1 milestone-p2 milestone-p3 milestone-p4 milestone-p5

coverage:
    python3 scripts/coverage-gate.py --line-floor {{coverage_line_floor}} --branch-floor {{coverage_branch_floor}} --ignore-filename-regex '{{coverage_ignore}}'

check: lint test coverage
