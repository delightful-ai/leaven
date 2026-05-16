## Boundary
This subtree is the durable product and architecture contract. Specs here outrank dated plans and philosophy notes, but each spec's own status line still matters.

The top of the stack is:
- `initial_library.md`: governing product and architecture spec for the current Leaven library shape.
- `guiding_principles.md`: requirements and product constraints, not implementation layout.
- `leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`: topology intent and `lib.rs` map contract. Its crate inventory can lag the live workspace; verify live membership against root `Cargo.toml` and `crates/leaven/tests/topology_contract.rs`.
- `durable_runs_and_resume.md`: default-durable run/resume semantics, `RunStore` vocabulary, optimizer continuation policy, and the explicit `ephemeral` escape hatch.
- `resume_compatibility_fingerprints.md`: durable resume compatibility domains,
  manifest comparison, runtime fingerprint obligations, cache/budget
  compatibility, and P8 resume refusal requirements.
- `default_cache_storage.md`: one-knob durable cache defaults, SQLite-backed
  evaluation and LM response caches, automatic safe caching policy, report
  requirements, and explicit override modes.
- `case_visibility_and_target_isolation.md`: case id/input/target/metadata
  visibility rules, target-free runner views, scorer target access, metadata
  projection, and proof requirements for preventing hidden-answer leaks.
- `aime_case_report_adapter.md`: P8 AIME import-record lowering into target-safe
  Leaven cases, source-derived case identity, report source projection, and
  AIME-specific cache/resume identity obligations.
- `gepa_reflection_evidence_visibility.md`: optimizer-visible reflection data,
  scorer-feedback boundary, target-safe case input projection, hidden split
  defaults, and durable reflection evidence requirements.
- `per_case_assessment_rows.md`: case-targeted assessment row law for
  `AssessmentGranularity::PerCase`, GEPA normalization, report grouping, and
  cache/row restoration requirements.
- `p8_run_report_operator_ux.md`: P8 result/report surface, run-dir and
  resumability facts, cache/cost/source summaries, proof classification, and
  durable report files.
- `p8_live_provider_budget_reliability.md`: live-provider role identity, budget
  split, retry/idempotency semantics, cache hit cost rules, and P8 live-mode
  cost reporting.
- `gepa_aime_paper_parity.md`: implementation spec for the GEPA/AIME paper-parity path, including budget stopping, GEPA loop continuation, reflection, LM/provider/cache use, AIME runner/scorer/reporting, and the proof denominator for P8.
- `milestone_examples_behavioral_contract.md`: executable behavior contract for milestone examples.
- `docs/testing/README.md`: proof model and coverage/SLA contract.

## Status Discipline
- `first_two_subsystems.md` is explicitly superseded. Read it only as historical context unless you first update it to current topology.
- Specs marked `planning` or `pre-implementation` are design contracts, not proof that code exists. Verify current code before routing work from them.
- Specs marked `implementation spec` can govern a slice even before the whole product story is solved, but still require code/test proof before public maturity claims.
- Do not route from directory presence alone. `crates/leaven-dsrs` is currently an orphan placeholder rather than a workspace crate, while `crates/leaven/tests/topology_contract.rs` is the executable inventory guard.
- Provider-adapter specs such as Codex CLI/app-server own provider boundaries only. They do not move agent, engine, GEPA, or skill concepts into provider crates.
- Eval/GEPA nomenclature specs distinguish public product words from lowered machinery. Do not expose lowered graph/eval vocabulary at the Layer 1 user surface.
- `milestone_examples_behavioral_contract.md` currently covers P0 through P4. For P5-P8, use the example package docs, `docs/testing/README.md`, and the recent review tree until a durable spec is added.
- `docs/specs/tracing-vision/README.md` still references `first_two_subsystems.md` as governing context. Reconcile that stale reference before using tracing-vision text to route new code.

## Local Rules
- Preserve spec vocabulary in code and tests unless the spec is being corrected in the same change.
- If code and spec disagree, resolve the mismatch deliberately: update code toward the spec, or update the spec because the implemented boundary is now the durable truth.
- Keep implementation plans, audits, and incident narratives out of this subtree. Put dated execution notes in `docs/plans/`.
- When adding a spec that changes crate ownership, update `AGENTS.md`, the topology contract test, and the nearest crate docs in the same change.
- Do not treat topology success as product success. If a spec adds public exports, default features, examples, or placeholders, name the maturity proof or explicitly mark the surface as scaffold.

## Decision Cards
- when: implementing from `initial_library.md`
  do: find the narrow companion spec first, then inspect current crate APIs before editing
  preserve: product vocabulary and user-layer intent
  avoid: filling missing product behavior with local example fixtures or provider bypasses
  verify: the companion spec's narrow gate, then `just check` before completion

- when: touching topology or crate inventory specs
  do: compare against root `Cargo.toml` and `crates/leaven/tests/topology_contract.rs`
  preserve: live workspace membership over stale directory/spec inventory
  avoid: reintroducing `crates/leaven-dsrs` or skeleton provider/backend crates as real routing targets without full workspace/test updates
  verify: `cargo test -p leaven --test topology_contract`

- when: adding a public example or default import promise to a spec
  do: state whether the proof is product-proof, mechanics-smoke, or proxy-demo
  preserve: the review-tree distinction between executed code and honest product maturity
  avoid: using `ReflectiveMutation::new(fixed_edit)`, shell-provider scripts, or compile-error derives as ordinary product evidence
  verify: the named milestone command plus the relevant public import/export test

## Verification
- Path and command references in specs must resolve.
- Crate topology changes: `cargo test -p leaven --test topology_contract`.
- Milestone behavior changes: the matching `just milestone-pN` command plus focused crate tests.
- Completion gate for implemented behavior remains `just check`.
