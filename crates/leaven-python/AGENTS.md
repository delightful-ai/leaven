## Boundary
This crate is the Python domain adapter placeholder: Python artifacts,
evaluators, and runtime integration.

Current public names are scaffolding. They do not prove Python interpreter
selection, environment isolation, dependency installation, execution, or error
capture.

## Local Bait
- Python command execution and filesystem isolation must compose
  `leaven-workspace`; do not add ad hoc process spawning here that bypasses the
  workspace contract.
- Python artifact/surface semantics should preserve source, environment, and
  runtime evidence instead of hiding them behind a scalar result.

## Verification
- `cargo check -p leaven-python` proves only scaffold exports.
- Real behavior needs deterministic artifact/evaluator tests and opt-in
  interpreter/runtime tests with explicit environment assumptions.
