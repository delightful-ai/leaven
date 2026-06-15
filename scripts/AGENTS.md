## Boundary
This subtree holds repository scripts with local side effects. Scripts are part of the proof path, so they must be deterministic, rerunnable, and explicit about what they invoke.

Current scripts:
- `lint-line-count.py`: enforces production Rust source size limits.
- `test-suite-sla.py`: builds default workspace libtests with nextest, runs those libtests in parallel plus doctests under the suite deadline, warns on the `<30s` suite target, and enforces a hard completion timeout. It prewarms doctests before starting the runtime timer and excludes milestone examples from the default lane.
- `coverage-gate.py`: runs coverage over default workspace tests plus a tiny `xtask git-trust-bench` smoke with its focused trust-test preflight, then enforces line and branch floors over production/source behavior. It excludes milestone packages from the default coverage lane and excludes test harness files and `#[cfg(test)] mod ...` blocks from the denominator after execution. Its `--package`, `--test`, `--skip-clean`, `--skip-smoke`, and `--skip-report` flags are an explicit developer feedback lane, not the canonical coverage gate; `--skip-clean` still clears stale profraw files while preserving compiled artifacts.
- `p8-gepa-debug-sqlite.py`: exports an existing P8 `reports/p8-aime.json` file, and optionally an upstream GEPA `gepa_state.bin`, into local SQLite tables for optimizer debugging. It does not call providers, fetch datasets, or mutate source.
- `ensure_leaven_workspace.sh`: guard for paper-lane shell examples that must run from the main `/Users/darin/src/personal/leaven` jj workspace. It performs no network or provider work.
- `check_trace2skill_config_fidelity.py`: no-spend ARA checker that verifies Trace2Skill SpreadsheetBench protocol/config values against the paper source bundle, upstream README/code, dataset manifest, Qwen generation config files, and generated full-denominator runbook.
- `check_trace2skill_prompt_manifest.py`: no-spend ARA checker/generator that records every upstream prompt and released skill file path, byte size, line count, and SHA-256 without copying full prompt text into the ARA.
- `check_trace2skill_upstream_code_manifest.py`: no-spend ARA checker/generator that records the upstream run/eval/analysis/evolution Python entrypoints used by the full-denominator runbook.
- `check_trace2skill_one_case_artifacts.py`: no-spend ARA checker that verifies deterministic one-case ACP artifacts, workbook score evidence, non-overlay Leaven result JSONL, and closeout one-case consistency.
- `check_trace2skill_plot_provenance.py`: no-spend ARA checker/generator for the target-plot provenance JSON; verifies plot PNG identity, table input hashes, computed panel values, and current overlay/non-overlay result separation.
- `check_trace2skill_result_intake.py`: no-spend ARA checker for result JSONL intake; verifies artifact paths and skill-source paths are inspectable, mechanics/one-case rows cannot overlay, and overlay rows use denominator-appropriate units/classifications.
- `check_trace2skill_evidence_bindings.py`: no-spend ARA checker for claim/experiment/evidence wiring; verifies every evidence file is indexed, every indexed source exists, every indexed claim exists, and claim/experiment references are not dangling.
- `check_trace2skill_status_docs.py`: no-spend ARA checker for human status docs; verifies current file counts, result-row summaries, and known overlay/non-overlay state stay synchronized with artifacts.
- `check_trace2skill_rigor_followup.py`: no-spend ARA checker for Seal Level 2 follow-up claims; verifies addressed review findings are actually reflected in the exploration tree, C07 proof links, tolerance/prompt artifacts, and review prose.
- `check_trace2skill_runbook_labels.py`: no-spend ARA checker for result proof classifications; verifies generated runbook labels, result docs, importer/plotter/validator constants, and non-overlay restrictions stay synchronized.
- `check_trace2skill_runbook_freshness.py`: no-spend ARA checker that regenerates the full-denominator runbook in a temp directory and verifies the committed JSON/Markdown outputs are not stale.
- `check_trace2skill_artifact_contract.py`: no-spend ARA checker for full-denominator artifact expectations; verifies approval-packet expected artifacts and per-stage runbook expected artifacts keep the future result-intake evidence contract visible.

## Local Rules
- Keep script defaults local and credential-free. Network, cloud, live model, or destructive behavior must be an explicit flag or environment opt-in.
- Print the commands or major actions a script runs before it runs them.
- Return non-zero on failed subprocesses and failed policy checks; do not hide failures behind warnings.
- Generated reports and temporary run directories belong under `target/` unless the caller explicitly provides another output path.
- If a script changes a canonical check, update `Justfile` and `docs/testing/README.md` in the same change.
- Coverage scripts do not execute milestone binaries by default. If a script starts reporting example proof status, it must preserve the product-proof / mechanics-smoke / proxy-demo distinction from `examples/AGENTS.md`.
- Coverage denominator exclusions are for test harness code and explicit non-default milestone packages only. Keep production source, scripts, and scaffold crates in the report once they have executable behavior.
- Do not add implicit credentials, provider calls, dataset downloads, or destructive filesystem cleanup to canonical scripts. Make those opt-in flags with printed side effects.
- Keep generated coverage summaries, lcov files, run stores, and temporary example outputs under `target/` by default; scripts should not dirty the repo root.

## Decision Cards
- when: changing `coverage-gate.py`
  do: state which packages it runs and which live paths it deliberately avoids
  preserve: coverage as denominator/execution proof, not product-maturity proof
  avoid: using coverage to bless P8 reflection, LM/cache roles, or live-provider paths
  verify: run a targeted `just coverage-fast --package <crate>` for feedback-mode behavior, then `python3 scripts/coverage-gate.py --line-floor 0 --branch-floor 0` for canonical script behavior, then `just coverage` when feasible

- when: changing `test-suite-sla.py`
  do: keep workspace libtest binaries plus doctests in one timed default lane
  preserve: the `<30s` full-suite target and hard completion timeout in `docs/testing/README.md`
  avoid: adding a hidden slow lane, silently dropping doctests from the measured suite, falling back to serial libtest execution, or counting empty generated binaries as suite proof
  verify: run `python3 scripts/test-suite-sla.py --warn-seconds 30 --timeout-seconds 600`

- when: adding a new repo script
  do: add the command to this map and wire it through `Justfile` only if it is part of the canonical operator path
  preserve: idempotent local defaults and explicit failure exits
  avoid: scripts that mutate source, fetch network data, or spend provider resources without an obvious flag/env gate
  verify: run the script with the smallest meaningful arguments and check that generated files land under `target/` or the requested output path

## Verification
- Run the touched script directly with the smallest meaningful arguments.
- For lint/test/coverage scripts, also run the corresponding `just lint`, `just test`, or `just coverage` target when feasible.
