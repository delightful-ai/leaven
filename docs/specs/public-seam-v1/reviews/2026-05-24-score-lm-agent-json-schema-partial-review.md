# Score, LM, and Agent Contract Partial Review

Date: 2026-05-24
Reviewer: Fermat (`019e59c2-ef3c-7f92-9b58-7bc00c5f62f1`)

Scope revset:

- Base: `vquknnuz` / `b9e54978` (`public-seam: harden blocked-row evidence audit`)
- Head: `mqmpkprk` / `c5bf6bee` (`public-seam: exercise json-schema agent path`)
- Included:
  - `urnmwvqp` / `92a10ac4` (`public-seam: project grouped score outputs from evidence`)
  - `pwxqsxqz` / `92c51600` (`public-seam: exercise json-schema LM trait path`)
  - `mqmpkprk` / `c5bf6bee` (`public-seam: exercise json-schema agent path`)

## Reviewed Claim

This review covered a partial public-seam V1 tranche:

- `ps1.evaluator.score_output`: pairwise/listwise `RunContext` assessment
  Plan IR projection now uses stored candidate-bound assessed outputs rather
  than guessing from a joined score-output string.
- `ps1.lm.contract`: JSON-schema LM output now executes through the
  provider-neutral `leaven_lm::Lm` trait harness and rejects invalid parsed
  provider payloads.
- `ps1.agent.contract`: JSON-schema agent output now executes through the
  provider-neutral `leaven_agent::AgentRunRequest` / `PlanAgentRunOutcome`
  harness and rejects invalid parsed agent payloads.

The reviewer did not rerun tests as the basis of sign-off. The review was a
semantic inspection of the revset, specs, implementation, tests, matrix, and
fake-pass traps.

## Findings

Fermat found no Critical, Important, or Minor issues.

## Matrix Promotion

No matrix row should be promoted from this tranche.

- `ps1.evaluator.score_output` remains pending. The grouped score-output
  tranche is real and improves fake-pass resistance, but blob-backed score
  outputs still intentionally fail at the run-layer lowering boundary because
  public blob metadata is not available there.
- `ps1.lm.contract` remains pending. The JSON-schema LM trait path is real
  contract evidence, but it is not full live provider or ACP closeout.
- `ps1.agent.contract` remains pending. The JSON-schema agent contract path is
  real contract evidence, but it does not prove full runtime transcript/stdout
  binding, ACP delivery, or row closeout.

## Residual Risks

The main residual risk is public-maturity overclaiming. These changes remain
honest partial evidence. They improve fake-pass resistance for joined score
strings, provider-specific LM shortcuts, and unvalidated parsed JSON, but they
do not prove full evaluator score-output provenance for blob-backed outputs,
production LM provider behavior, production agent runtime behavior, ACP
delivery, or durable receipt persistence.

## Main-Agent Verification

- `cargo fmt --check`
- `cargo test -p leaven-evidence --test feedback -- --nocapture`
- `cargo test -p leaven-run --test public_seam -- --nocapture`
- `cargo test -p leaven-run --test scoring_evaluator runtime_score_outputs_project_through_public_seam_for_all_assessment_shapes -- --exact --nocapture`
- `cargo test -p leaven-run --test scoring_evaluator judging_evaluator_preserves_pairwise_and_listwise_report_outputs -- --exact --nocapture`
- `cargo test -p leaven-run --test scoring_evaluator judging_evaluator_preserves_candidate_artifact_reportable_outputs -- --exact --nocapture`
- `cargo clippy -p leaven-evidence --tests -- -D warnings`
- `cargo clippy -p leaven-run --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`
- `cargo test -p leaven-public-seam --test lm_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document lm_complete_lowering_preserves_json_schema_output_and_provider_hints -- --exact --nocapture`
- `cargo clippy -p leaven-public-seam --test lm_contract -- -D warnings`
- `cargo test -p leaven-public-seam --test agent_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document agent_run_lowering_preserves_json_schema_output_contract -- --exact --nocapture`
- `cargo clippy -p leaven-public-seam --test agent_contract -- -D warnings`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
