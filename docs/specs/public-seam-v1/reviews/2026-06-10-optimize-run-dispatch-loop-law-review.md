# Optimize Run Dispatch Loop-Law Review

Date: 2026-06-10

Scope:
- `ps1.optimize.run_dispatch`
- `crates/leaven-public-seam/src/optimize_run.rs`
- `crates/leaven-public-seam/tests/public_seam_contract/optimize_run.rs`
- `crates/leaven-seam-service/src/optimize_run_service/`
- `crates/leaven-seam-service/src/optimize_run_service/tests.rs`

Reviewers:
- Adversarial loop-law verification session (Opus, workflow `wf_551d9c05-75b`,
  review rounds 1-3), with independent spec-compliance and code-quality review
  sessions in the same workflow.
- Controller verification (Fable session, 2026-06-10): re-ran
  `cargo test -p leaven-seam-service` (44 passed),
  `cargo test -p leaven --test topology_contract` (8 passed), and
  `cargo clippy -p leaven-seam-service --all-targets` clean against slice
  commit `pkyvxvpz`.

Review method:
- Adversarial semantic review of the deterministic loop-law scenario
  (`optimize_run_drives_the_real_gepa_loop_to_a_changed_re_evaluated_child`)
  with mandatory live mutation testing, explicitly hunting for gamed fixtures:
  a worker that answers correctly regardless of the dispatched template, a
  reflection mock that bypasses the parse-propose-apply path, assertions loose
  enough to pass with mechanics-only behavior, and metric-call accounting that
  tolerates a skipped re-evaluation.

Findings:
- The worker fixture answers correctly only when the improvement marker is
  present in the candidate template it actually receives in the runner
  payload (`payload.case_input.candidate_template`), not via a hardcoded
  reply. The scorer reads the hidden target through a capability-gated
  `leaven/case.target` callback and compares genuinely.
- Frontier assertions pin both scores (seed entry 0.0 and best 1.0) from
  `GepaReport.candidates[].validation_score` graph truth; lineage asserts the
  child's parent is the seed; the child template carries the marker while the
  seed does not.
- Live mutation 1: forcing the worker to always answer correctly regardless
  of template makes the test FAIL (no strict improvement; the child is
  rejected and the loop errors after exhausting the scripted LM). Restored to
  a clean working copy afterwards.
- Live mutation 2: emptying the reflection text so no usable child can be
  proposed makes the test FAIL (the child evaluates to 0.0, is rejected, and
  best stays the seed). Restored to a clean working copy afterwards.
- Metric-call accounting matches GEPA reference semantics by exact equality:
  seed validation (1) + parent minibatch (3) + child minibatch (3) + accepted
  child validation (1) = 8, confirmed against the live `ProfileResolved`,
  `ParentEvaluated`, `ChildEvaluated`, and `AcceptedValidationCompleted`
  event trace. A skipped child re-evaluation yields 5 and a reused parent
  screen drops below 8; both fail the `== 8` assertion.
- Earlier review rounds in the same workflow surfaced and fixed one blocking
  executor defect (OpenAI-backed reflection could not run under
  `futures::executor::block_on`; resolved with a current-thread tokio runtime
  plus scoped-thread worker LM callbacks, each guarded by a test) and one
  duplication debt (sanitizer consolidation). Both fixes are folded into the
  slice commit and re-verified.

Verdict:
- The loop-law proof is genuine and unriggable on all checks performed: the
  real `leaven-gepa` loop authored a changed artifact through reflection,
  applied it through `RunContext`, and re-evaluated it onto the frontier as
  the best candidate, with exact reference metric accounting.

Signed-off rows:
- `ps1.optimize.run_dispatch`: signed off — may be promoted to proven.
