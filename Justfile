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

coverage:
    python3 scripts/coverage-gate.py --line-floor {{coverage_line_floor}} --branch-floor {{coverage_branch_floor}} --ignore-filename-regex '{{coverage_ignore}}'

check: lint test coverage
