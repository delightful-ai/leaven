# GEPA Parity Working Ledger

Status: active.
Updated: 2026-05-18T14:18:00Z.

## Authority

Product and behavior authority remains:

- `docs/specs/initial_library.md`
- `docs/specs/gepa_reference_behavior.md`
- `docs/specs/gepa_aime_paper_parity.md`
- `docs/plans/2026-05-17-gepa-upstream-parity-matrix.md`
- current code/tests and emitted P8 reports

This file is a continuation ledger only. Verify every referenced artifact before
claiming completion.

## Current Goal

Make Leaven GEPA parity-or-better than pinned upstream GEPA through the public
`optimize(seed).using(Gepa...)` route, with intentional Leaven-better deltas
documented, tested, and reported.

## Current Matrix State

The parity matrix currently records the core GEPA reference-loop rows as proven
or intentional deltas. The P8 finished live report is now historical evidence:
it proves a real live improvement and the completed operator path for the
pre-`OptimizeAnything` profile slice, while the current-code cache-only failure
report proves current profile/runtime/failure-report evidence without provider
spend. The remaining P0 gap is P8/AIME result quality: historical finished
live quality evidence improves over the seed, but the held-out score still
trails the pinned GEPA CAIS artifact target and predates the current
`gepa_profile=optimize-anything` default.

Important currently proven rows include:

- public Layer 1 `optimize(...).using(Gepa...)` route with typed `GepaReport`;
- seed full validation before train;
- validation-Pareto parent selection and checkpointed selector state;
- epoch-shuffled minibatches and resume;
- same-case parent/child screening;
- strict improvement acceptance;
- accepted-child full validation before admission;
- per-case evaluation cache reuse and zero-cost hit accounting;
- skip-perfect/no-reflective-examples before LM work;
- upstream-style generic and AIME reflection prompt/parser snapshots;
- target-safe reflective dataset projection and hidden-target isolation;
- GEPA-specific phase events and P8 JSON event projection;
- materialized AIME cache proof: `target/leaven-aime-cache/aime.json`,
  SHA-256 `0f39c54861fd37a609d5bf397902a2086c245ebee879704dbd74b485115402c3`,
  570246 bytes, train 45, validation 45, test 30, 120/120 unique source IDs.

Do not claim DSPy-default parity. Current claims are core GEPA or
optimize-anything/AIME profile; DSPy merge and DSPy trace defaults are not
implemented as default parity.

## Latest No-Spend Strict-Reflector Rehearsal

2026-05-18 cache-only rehearsal:

- command class: no-spend cache-only live-role rehearsal with
  `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1`,
  `LEAVEN_AIME_SOLVER_CACHE_POLICY=cache-only`,
  `LEAVEN_AIME_REFLECTION_CACHE_POLICY=cache-only`,
  `LEAVEN_AIME_LM_CACHE_BACKEND=eager-sqlite`, and
  `LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS=32`;
- run dir:
  `.leaven/release-runs/p8-aime-gepa-upstream-reflector-cache-only-current-20260518-124009`;
- run id: `5bce6614-03f3-47bc-a8ae-060bfdd4bcd1`;
- observed work before refusal: seed validation 45 metric calls plus one
  three-case train minibatch, `metric_calls=48`, `llm_calls=0`;
- refusal: first missing `gpt-5.1` reflection cache row, recorded as
  `lm response cache failed: required lm cache entry was missing`;
- durable failure row:
  `lm-provider-failures.jsonl` contains one reflection/cache failure;
- start/failure reports classify the reflection model as `upstream-matched`
  and record the CAIS denominator caveat: published validation `26/45`,
  published test `0.600`, current upstream source search cap `500`,
  checkpoint metric calls `621`, checkpoint candidates `10`, and missing
  local `run.log`.

Follow-up fix in current code: P8 failure reports now carry the same
role/cache/provider-failure evidence as successful reports. In particular,
cache-only or provider-failure stops include `lm_roles`, `live_provider_proof`,
`provider_failures`, search/final-report budget caps, and LM cache read/write
paths. This is required by `p8_run_report_operator_ux.md` and
`p8_live_provider_budget_reliability.md`: an operator should not need terminal
history to tell cache-only miss, provider failure, model alignment, and cache
topology apart.

2026-05-18 current-code rerun:

- run dir:
  `.leaven/release-runs/p8-aime-gepa-upstream-reflector-cache-only-current-20260518-055000`;
- run id: `50777a7b-205a-4abb-b6c3-d8d38940eeac`;
- proof class: `cache_only_aime_replay_not_live_proof`;
- profile evidence: CLI, start report, and failure report now all show
  `gepa_profile=optimize-anything`;
- observed work before refusal: seed validation 45 metric calls plus one
  three-case train minibatch, `metric_calls=48`, `llm_calls=0`;
- failure: first missing `gpt-5.1` reflection cache row,
  `lm response cache failed: required lm cache entry was missing`;
- failure report evidence now includes `search_metric_call_cap=500`,
  `final_report_metric_call_cap=unlimited`, eager-cache read paths
  `[run-dir/lm-cache.sqlite, .leaven/lm-cache.sqlite]`, write path
  `.leaven/lm-cache.sqlite`, two live OpenAI role fingerprints, and durable
  provider failure totals with reflection/cache count `1`;
- the first rerun attempt also caught a production compile break from making
  failure-role reports production code while leaving
  `openai_provider_fingerprint_for_runtime(...)` behind `#[cfg(test)]`; that
  helper is now production code because failure reports need live role
  fingerprints outside tests.

## Current Prompt-Parity Finding

2026-05-18 prompt audit:

- upstream generic GEPA reflection prompt:
  `/Users/darin/vendor/github.com/gepa-ai/gepa/src/gepa/strategies/instruction_proposal.py`
  `InstructionProposalSignature.default_prompt_template`;
- Leaven generic GEPA reflection prompt:
  `crates/leaven-gepa/src/reflection.rs`
  `DEFAULT_REFLECTION_PROMPT_TEMPLATE`;
- byte check: exact match, length `942` on both sides;
- upstream optimize-anything reflection prompt:
  `/Users/darin/vendor/github.com/gepa-ai/gepa/src/gepa/optimize_anything.py`
  `optimize_anything_reflection_prompt_template`;
- Leaven P8 optimize-anything reflection prompt:
  `examples/p8_aime_gepa/src/main.rs`
  `OPTIMIZE_ANYTHING_REFLECTION_PROMPT_TEMPLATE`;
- byte check: exact match, length `1287` on both sides;
- upstream renderer:
  `InstructionProposalSignature.prompt_renderer` emits one prompt string for
  non-image data, with `# Example N`, ordered `## field` sections, recursive
  `###`/`Item N` headings for structured values, and `str(value).strip()`
  followed by blank lines;
- Leaven renderer:
  `DefaultReflectionRenderer` emits a single user message for text-only
  reflection and uses ordered `ReflectiveExample.side_info` fields with the
  same markdown structure;
- upstream optimize-anything AIME side-info fields:
  `score`, `input`, `prompt`, `output`, `reasoning`,
  `execution_feedback`;
- Leaven P8 AIME side-info fields:
  `aime_reflection_side_info_example(...)` emits the same fields in the same
  order;
- direct upstream Python renderer check on a toy AIME record produced the same
  markdown prompt as the Leaven snapshot test
  `aime_full_reflection_prompt_renders_upstream_optimize_anything_markdown`;
- actual `.leaven/lm-cache.sqlite` live reflection request rows for
  `gpt-5.4-mini` show the reflection LM saw the optimize-anything prompt as a
  single user prompt with markdown examples and the expected side-info keys.

Conclusion: for P8 optimize-anything AIME, prompt template/renderer/side-info
formatting is currently proven 1:1 with pinned upstream GEPA for text-only
records. Remaining prompt-surface deltas are not P8 blockers: upstream image
side-info returns multimodal messages, while Leaven currently only represents
text/mapping/list side-info; DSPy-default trace rendering remains a separate
non-default parity row.

## Current Speed/FastGEPA Position

The speed ideas are plausible as a future opt-in GEPA profile, but they must
not replace the reference `Gepa::reference()` / P8 AIME behavior while the
parity proof is open. Full validation before reference admission and final
selection from fully certified candidates remain parity invariants.

Current speed stance:

- reference GEPA stays serial and fully validating until the P8 live quality
  row is closed;
- `GepaProfile::FastCertified` is the first implemented speed profile: smaller
  train probes and two serial proposal attempts per selected parent, while
  preserving full validation before reference admission;
- P8 live AIME defaults to `GepaProfile::OptimizeAnything`, which names the
  upstream optimize-anything knobs Leaven uses for AIME: minibatch 3, one
  serial proposal, full validation before admission, and skip-perfect disabled.
  Deterministic smoke may still explicitly choose `GepaProfile::Reference`.
  `LEAVEN_AIME_GEPA_PROFILE=fast-certified` remains an opt-in speed preset;
  release reports must disclose the selected GEPA profile before any
  speed/result claim;
- parallel proposal workers, lazy validation/certification, active failure
  sampling, evaluator pyramids, and trace distillation should be modeled as
  explicit follow-on library profiles/seams, not P8-only patches;
- cheap/proxy stages may filter or prioritize, but only the real evaluator and
  full validation can admit/crown reference candidates;
- current concrete speed work is observability plus safe opt-in API shape:
  expose accepted-but-unadmitted children, attempt counts, and validation
  counts in reports so long runs can be cut off or resumed from data, and keep
  `proposal_count` labeled as serial rather than async island GEPA.
- `GepaReport.profile` and `gepa.profile_resolved` now carry the optimizer's
  resolved profile facts: label, known train minibatch size, serial proposal
  count/mode, validation policy, certification mode, skip-perfect policy, and
  perfect-score threshold. P8 JSON mirrors those facts under
  `gepa_report.profile`, so an operator can distinguish `reference`,
  `fast-certified`, and custom AIME overrides without reading builder code.
- Serial multi-proposal reflection requests carry `gepa_attempt_index` in
  provider hints while keeping the rendered prompt text identical. This makes
  cache/eager-cache behavior inspectable for repeated parent/minibatch
  proposals without changing the model-visible prompt.
- public `.proposal_count(0)` no longer creates parent-only iterations. It is
  normalized to one serial proposal, matching upstream's "parallel proposal
  count <= 1 means serial" behavior, and the resolved profile reports
  `proposal_count=1`.
- P8 README now documents the opt-in `LEAVEN_AIME_GEPA_PROFILE=fast-certified`
  profile explicitly. Keep calling the implemented speed preset
  `FastCertified`: it is a full-validation certified profile, not lazy or
  async FastGEPA.
- P8 reports now include `case_deltas`: per split/source baseline score,
  optimized score, score delta, and outcome (`improved`, `regressed`,
  `unchanged_correct`, `unchanged_wrong`, or missing-row states), plus split
  summary counts. This keeps future live quality diagnosis data-first and
  target-safe without manual reconstruction from flat final-report case rows.
- `GepaReport` now includes `quality_summary`: train-screened proposal counts,
  accepted/rejected/admitted/unadmitted counts, and accepted-child full
  validation outcomes versus each parent. This is report-only diagnostic state
  for spotting train-minibatch overfit; it must not influence parent selection,
  acceptance, admission, or final GEPA result choice.
- P8 `gepa_report` now includes `reflection_summary`: attempted/observed
  reflection counts, visible prompt duplicate count, and request/assistant/
  proposed-text character summaries. This is report-only diagnosis for the
  current speed/quality question; it must not influence GEPA search behavior.
- Level 2 GEPA report access now has an umbrella convenience route:
  `leaven::gepa::GepaOptimizedExt::gepa_report()`. The method is a facade over
  generic `Optimized::optimizer_report::<GepaReport>()`, lives under
  `leaven::gepa` instead of `leaven::prelude`, and keeps `leaven-run`
  optimizer-agnostic.
- The pinned upstream checkout's DSPy 3.2.1 `JSONAdapter` output for
  `ChainOfThought(MathSolverSignature)` includes a blank line after "Inputs
  will have the following structure:" and after "Outputs will be a JSON object
  with the following fields."; P8 now preserves those exact fallback prompt
  bytes. The primary ChatAdapter prompt already matched the upstream renderer
  byte-for-byte for the audited sample.
- P8 reports now classify the actual reflection model alignment as
  `upstream-matched`, `model-delta`, or `not-applicable`. This makes a
  `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1` strict upstream-reflector run
  distinguishable from the default `gpt-5.4-mini` Leaven stronger-reflector
  run before anyone reads terminal history.
- P8 start, failure, and final reports now all carry the same comparison block.
  Cache-only strict upstream-reflector rehearsal at
  `.leaven/release-runs/p8-aime-gepa-upstream-reflector-cache-only-20260518-050427`
  failed closed before provider use (`llm_calls=0`) because the selected
  `gpt-5.1` reflection row was absent, and both `reports/p8-aime-start.json`
  and `reports/p8-aime-failure.json` recorded
  `comparison.reflection_model_alignment=upstream-matched`.
- P8 comparison blocks now make the local CAIS denominator caveat structured:
  published validation `26/45`, published test `0.600`, configured upstream
  search cap `500`, inspectable checkpoint metric calls `621`, checkpoint
  candidate count `10`, and `upstream_run_log_available=false`. This keeps
  future live reports from hiding the difference between the published CAIS
  target, the inspectable `gepa_state.bin`, and the missing local `run.log`.
- P8 reports now expose LM cache read order and write destination separately.
  This matters for `eager-sqlite`: exact selected-run rows are read before
  compatible workspace rows, while new responses write to the workspace cache.
  The old single `lm_path` field is still present but is no longer the only
  cache-location evidence operators should inspect.
- P8 cache-only replay now reports the LM cache path topology explicitly:
  `eager-sqlite` reads the selected run-dir cache first, then workspace
  `.leaven/lm-cache.sqlite`, and writes through to the workspace cache. Failure
  reports also include structured `resume_compatibility` details plus a CLI
  `resume_compatibility_mismatch=...` line. A no-spend replay copy of the
  historical completed run at
  `.leaven/release-runs/p8-aime-gepa-current-release-cache-only-replay-20260518-120826`
  correctly refused before LM work because the stored runner fingerprint
  `51c456483b4d49c646aa738f9651642928714e2616a927872216f3a658504fd4` predates
  the live runner fingerprint
  `1b1fdd24eab5a9bb6035c2b71172a2877c888005650b8ca83d6ef507bc0a1d43`.
  This is an intentional compatibility refusal, not a cache miss or provider
  spend.

Current feedback/reflection answer:

- P8 scoring generates real feedback in `score_answer(...)` via
  `aime_score_feedback(...)`, including the reference solution visible to the
  scorer;
- `AimeReflectiveDataset` reads the parent's case assessments, recovers solver
  output/reasoning from `AimeSolverSideInfoStore`, and passes ordered
  optimize-anything side-info fields `score`, `input`, `prompt`, `output`,
  `reasoning`, and `execution_feedback` to reflection;
- `DefaultReflectionRenderer` renders those side-info fields directly and does
  not fall back to the generic `## Feedback` block when adapter side-info is
  present, matching upstream optimize-anything rendering;
- focused proof: `aime_scorer_feedback_matches_upstream_gepa_aime_wording`,
  `aime_side_info_renders_upstream_optimize_anything_keys`, and
  `aime_full_reflection_prompt_renders_upstream_optimize_anything_markdown`.

## Current Model-Quality Position

Do not collapse the live quality gap into "the model is bad" or "the prompt is
bad" without a fresh completed report.

Current evidence split:

- the stale completed run
  `.leaven/release-runs/p8-aime-gepa-20260518-043717/reports/p8-aime.json`
  used solver `gpt-4.1-mini` and reflection `gpt-5.4-mini`, admitted children,
  and finished with `baseline == optimized`;
- the older summary
  `.leaven/release-runs/p8-aime-gepa-20260516-221231/reports/summary.json`
  improved validation `0.444 -> 0.533` and held-out test `0.367 -> 0.467`,
  but lacks the current P8 proof fields and cannot close parity;
- the current JSON-fallback run reached live traffic with 200 metric calls and
  130 LM calls, then failed on an old-binary cache-row bug before final reports;
- current code fixes that cache-row bug by rematerializing same-content cache
  hits for the requested candidate, and a cache-only replay proves the old
  wrong-candidate assertion no longer reproduces;
- a later read-write resume against the same run dir replayed the rematerialized
  cache hits and spent 12 new metric calls before being stopped because its
  compile log showed it predates the latest P8 profile/failure-report slices;
- the historical completed live report
  `.leaven/release-runs/p8-aime-gepa-current-release-20260518-094902-d2d15a36d364/reports/p8-aime.json`
  used current report/cache code at the time it was produced, zero provider
  failures, solver `gpt-4.1-mini`, reflection `gpt-5.4-mini`, reference GEPA
  profile, 45/45/30 AIME splits, and improved validation `0.444 -> 0.489`
  plus held-out test `0.433 -> 0.500`. It predates the current
  `GepaProfile::OptimizeAnything` default and failure-report evidence fields;
  the next live report should show `gepa_profile=optimize-anything`.
- a model-matched paid rerun should set `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1`
  and confirm `comparison_reflection_model_alignment=upstream-matched` in
  `reports/p8-aime.json` before interpreting result quality against the pinned
  upstream target.

Conclusion: the historical completed live report proves live improvement and
the finished release-report operator path for the then-current reference-profile
AIME run, while the latest cache-only failure report proves current
`optimize-anything` profile disclosure and failure evidence. The stack still
does not prove "as good as GEPA" because the held-out test score is `0.500`
versus the pinned upstream target `0.600`, and no completed live report has yet
used the current `gepa_profile=optimize-anything` default. Further quality work
should start from the emitted candidate lineage, reflections, proposal attempts,
and prompt examples in the completed report.

## Current No-Spend Quality Diagnosis

Current completed live report:

```text
.leaven/release-runs/p8-aime-gepa-current-release-20260518-094902-d2d15a36d364/reports/p8-aime.json
```

No-spend inspection of that report found:

- optimization admitted six children and selected candidate index `3`, produced
  by attempt `24` from parent index `2`;
- the winning prompt is a generic exact/conservative math instruction, not a
  domain-specific AIME strategy:
  "First identify the key theorem/structure", exact arithmetic, justified
  assumptions, extremum/boundary checks, and final integer only;
- accepted children had validation scores `0.400`, `0.467`, `0.489`, `0.378`,
  `0.444`, and `0.489`, so strict train-screen wins frequently regressed or
  only weakly improved validation;
- held-out test changed from 13/30 to 15/30: improved source ids
  `MathArena/aime_2025:default:train:{4,6,7,28}` and regressed source ids
  `MathArena/aime_2025:default:train:{10,11}`;
- validation changed by six improvements and four regressions, leaving 19
  validation cases still wrong under both baseline and optimized prompts;
- reflection requests remain very large for a minibatch of three examples:
  `40` reflection prompts, min `23519`, max `92309`, average `49386.525`
  characters.

Interpretation: the current bottleneck is probably not malformed prompt
rendering. It is weak search signal and expensive, generic reflective updates:
the reflector sees full reasoning/feedback, but mostly proposes broad "be
careful" rules. The next no-spend quality pass should compare accepted and
rejected reflections against upstream GEPA/AIME traces if available, then test
whether side-info compression, active failure sampling, stronger reflector model,
or multi-proposal diversity produces more specific proposals. Do not run another
paid release attempt before this diagnosis is translated into an explicit
profile/experiment plan.
- post-run report audit found one duplicate reflection request/response pair in
  the completed run. The generic reflection renderer now adds stable
  `gepa_attempt_index` provider metadata when GEPA calls the LM-backed
  reflector. Because LM cache keys include provider hints, resume of the same
  attempt remains cacheable while identical prompt text in a later GEPA attempt
  no longer burns search diversity by replaying the older stochastic response.
- next paid/live release reports should use the new `case_deltas` section
  instead of ad hoc jq scripts for improved/regressed/still-wrong source ids.

## Current Audit-Wave Disposition

2026-05-18 verifier wave disposition against the current tree:

- true blocker: the completed live report
  `.leaven/release-runs/p8-aime-gepa-20260518-043717/reports/p8-aime.json`
  remains stale proof and result-quality gap evidence only. It does not prove
  current report schema, zero durable provider failures, or "as good as GEPA"
  result quality because it kept `best_index=0` and `baseline == optimized`;
- true blocker: the current JSON-fallback run below is not completion proof. It
  found the old-binary wrong-candidate cache bug, then a newer read-write
  resume was stopped after cache-fix proof because its binary predates latest
  P8 report/profile slices;
- no longer blocker for historical operator proof: the fresh current-binary
  live report below proves a completed live operator/report path and zero
  process-local/durable provider failures. It is no longer current-profile
  proof after the `GepaProfile::OptimizeAnything` default slice;
- already fixed in current code: P8 role fingerprints include the actual
  `OpenAiLm::fingerprint()` for timeout/base-URL/retry/provider runtime
  compatibility, while cache replay controls are intentionally ignored by the
  role identity. The focused proof is
  `p8_role_fingerprints_include_observed_openai_provider_runtime` plus
  `p8_role_fingerprints_ignore_lm_cache_replay_controls`;
- already fixed in current code: non-reference no-validation train acceptance
  does not fill `GepaProposalAttempt.admitted_index`; `accept_child` only sets
  it when accepted-child validation returns a GEPA candidate index. The focused
  proof is `train_accepted_child_without_validation_is_not_reference_admitted`,
  and `MinibatchThenValidation` is not in `leaven_gepa::prelude`;
- already fixed in current code: malformed solver output after DSPy
  ChatAdapter and JSONAdapter parsing records `AnswerParse`, and LM cache
  telemetry separates required cache-only misses, read errors, and write
  errors. The focused proofs are
  `aime_solver_records_answer_parse_failure_after_json_fallback_parse_failure`
  and `lm_cache_failures_distinguish_required_miss_read_and_write_errors`.
- rejected as an exact-parity bug for now: parent-selection tie order. Upstream
  `select_program_candidate_from_pareto_front` builds frequencies from
  `set[int]` fronts after dominance pruning, so there is no stable first-seen
  order to reproduce. Leaven keeps deterministic candidate-index order plus
  Python-compatible RNG and treats this as reportable implementation detail
  unless a pinned upstream trace proves a material mismatch.
- intentional operator-safety delta: reflection/proposal provider/config errors
  remain hard errors in Leaven. Upstream catches broad reflection exceptions
  and returns `proposal=None`, but doing that for live P8 would let missing
  credentials, cache backend failures, or provider errors turn into a completed
  no-op run. A future profile can add typed non-fatal proposal failures, but
  live-provider release proof should fail loudly on provider/config errors.

## Live P8 Run Ledger

Current quality diagnosis from completed stale report:

- report:
  `.leaven/release-runs/p8-aime-gepa-20260518-043717/reports/p8-aime.json`;
- search stopped with `search_metric_calls_spent=493`,
  `search_metric_call_cap=500`, `stop_reason=budget_reached`;
- GEPA report had 8 reference candidates and 24 proposal attempts;
- seed stayed `best_index=0` / `validation_best_index=0` with validation
  `0.5333333333333333` and test `0.43333333333333335`;
- accepted children were full-validation admitted as candidates 1-7, but none
  strictly beat the seed validation score. Candidate 5 tied seed validation at
  `0.5333333333333333`, so strict validation-best update kept seed;
- attempt 24 produced a train-accepted child
  `43a538f8-3677-49d2-9180-4cb7130d1795` with train minibatch score
  `1.0 > 0.6666666666666666`, but budget stopped before full validation, so it
  was never reference-admitted and cannot be claimed as an optimized result;
- therefore the stale completed report is useful diagnosis data, not parity
  proof. It currently supports "seed stochasticity plus budget-boundary
  unresolved child" more than "reflection prompt/parser was malformed."

Current JSON-fallback run:

```text
.leaven/release-runs/p8-aime-gepa-current-json-fallback-20260518-084304
```

Pointer file currently points at this run. Observed process:

- PID `2771`, command `target/debug/p8_aime_gepa`;
- start time `Mon May 18 01:43:04 2026`, elapsed about 43 minutes at the
  2026-05-18T09:27Z check;
- run directory contained 200 evidence JSON files at the check;
- no run-local `run.log` was present at the check; inspect the process/tee
  owner or emitted reports instead of assuming a sidecar log path;
- no `reports/summary.json`, `reports/p8-aime.json`, or durable provider
  failure file had been emitted at the first check;
- the process later exited with
  `p8_aime_gepa_failed=optimizer failed: GEPA evaluation returned a row for the wrong candidate`
  after `metric_calls=200`, `llm_calls=130`, and `2632809` ms;
- root cause was engine casewise content-cache reuse returning raw cached
  `AssessmentId`s for a same-content child under a different `CandidateId`;
- current code rematerializes cache-hit assessment rows for the requested
  candidate without charging metric calls or overwriting the content cache;
- cache-only replay log:
  `.leaven/release-runs/p8-aime-gepa-current-json-fallback-20260518-084304.cache-only-after-engine-fix.log`;
- replay proof: the wrong-candidate assertion did not recur before cache-only
  mode refused the next uncached reflection request;
- a read-write resume log:
  `.leaven/release-runs/p8-aime-gepa-current-json-fallback-20260518-084304.resume-after-engine-fix.log`;
- read-write resume proof before stop: cache hits rematerialized requested
  candidate rows, the wrong-candidate assertion did not recur, and the process
  spent 12 new metric calls before stop;
- stop reason: the compile log showed the run binary predates latest P8
  profile/failure-report code, so letting it continue overnight would not close
  the current release-report freshness row.

Do not treat this run as completed live proof. It is a live infrastructure and
cache-fix proof, not result-quality evidence. The next release proof needs a
fresh binary and a completed `reports/p8-aime.json`.

Current live release run directory:

```text
.leaven/release-runs/p8-aime-gepa-20260518-043717
```

Pointer file:

```text
.leaven/release-runs/latest-gepa-aime-run.txt
```

Command shape used:

```bash
set -a
source ~/plans/.env
set +a
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_AIME_PROFILE=gepa-aime
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
export LEAVEN_AIME_LM_CACHE_BACKEND=eager-sqlite
export LEAVEN_AIME_RUN_DIR=.leaven/release-runs/p8-aime-gepa-20260518-043717
export LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS="${LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS:-32}"
cargo run -p p8_aime_gepa
```

Observed first attempt:

- same run id: `03ee6ad5-3234-4a24-81ac-f17a7358b275`;
- progressed to 357 search metric calls and 294 LM calls;
- admitted multiple children and ran accepted-child full validation batches;
- failed closed before final reports with OpenAI transport timeout:
  `error sending request for url (https://api.openai.com/v1/responses)`,
  `operation timed out`;
- no completed `reports/p8-aime.json` should be treated as emitted from that
  failed attempt.

Observed resume attempt with default 120s timeout:

- reused same run id;
- skipped seed validation and resumed from the run directory;
- failed again at the same search position with an OpenAI transport timeout.

Completed 600s-timeout resume attempt:

- added `LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS=600`;
- same run id accepted resume;
- observed evaluation-cache hits with zero metric calls inside the resumed run;
- emitted:
  - `.leaven/release-runs/p8-aime-gepa-20260518-043717/reports/summary.json`;
  - `.leaven/release-runs/p8-aime-gepa-20260518-043717/reports/p8-aime.json`.

The 600s timeout changes the OpenAI provider fingerprint. The run accepted the
resume and disclosed role runtime fingerprints, but this live report predates
two report-schema fixes added after the run:

- top-level `live_provider_proof` / `provider_failures` JSON fields now derive
  directly from `lm_roles`;
- top-level `gepa_events` now uses checkpoint-restored `GepaReport.events` when
  available, so resumed reports do not expose only the fresh observer tail.

Duplicate run guard:

- a second older provider process was found using `/tmp/leaven-gepa-live-run-dir.txt`
  and run directory `.leaven/release-runs/p8-aime-gepa-20260517-213546`;
- it did not have the 600s timeout override and was appending to the legacy
  `.log` sidecar rather than the current ledger run `output.log`;
- stopped PID pairs `27627`/`27655` and respawned `56611`/`56646`;
- stopped a later bare respawned child `59403`;
- after the second stop, only the intended 600s-timeout process for
  `.leaven/release-runs/p8-aime-gepa-20260518-043717` remained;
- during the report-contract test pass, found and stopped another throttled
  live process (`LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS=1`) using
  `/tmp/leaven-gepa-live-throttle-run-dir.txt` and run directory
  `.leaven/release-runs/p8-aime-gepa-throttle1-20260517-233940`
  (PIDs `72859`/`72891`);
- after final report emission, no `p8_aime_gepa` or `leaven-gepa-live` provider
  process remained.

Completed report snapshot:

- proof classification: `full_live_aime_reproduction_attempt`;
- profile: `gepa-aime`;
- solver model: `gpt-4.1-mini`;
- reflection model: `gpt-5.4-mini`;
- OpenAI concurrency: `32`;
- OpenAI request timeout: `600s` in the resumed process environment;
- dataset cache: `target/leaven-aime-cache/aime.json`;
- dataset cache SHA-256:
  `0f39c54861fd37a609d5bf397902a2086c245ebee879704dbd74b485115402c3`;
- dataset bytes: `570246`;
- source splits: train `45`, validation `45`, test `30`;
- baseline train: `0.600`;
- optimized train: `0.600`;
- baseline validation: `0.533`;
- validation: `0.533`;
- baseline held-out test: `0.433`;
- held-out test: `0.433`;
- stop reason: `budget_reached`;
- search metric calls spent: `493` of cap `500`;
- final-report metric calls: `75`;
- total metric calls: `568`;
- total LM calls: `436`;
- evaluation cache: `15` hits, `56` misses, durable SQLite run-store,
  zero-cost hit accounting true;
- solver role metrics: `259` calls, `81` cache hits, `178` misses, `0`
  failures in the successful resumed process;
- reflection role metrics: `10` calls, `0` cache hits, `10` misses, `0`
  failures in the successful resumed process;
- GEPA report: best index `0`, candidates `8`, full-validation evals `8`,
  proposal attempts `24`, skip-perfect `false`.

Report caveat from the diagnosis pass:

- the final JSON role metrics are LM telemetry from the successful resumed
  process, not a durable aggregate across every failed/resumed process in the
  release attempt;
- the earlier OpenAI transport timeouts are real attempt history and are only
  captured in this ledger/output logs, not in the final `lm_roles[].metrics`;
- post-run P8 code now appends provider failures to
  `lm-provider-failures.jsonl` in the run directory and projects those counts
  under `provider_failures.durable`, while keeping process-local role metrics
  separate;
- the completed `.leaven/release-runs/p8-aime-gepa-20260518-043717` report
  predates that durable field, so it still must not be used alone to prove
  "zero provider failures" for the multi-process release attempt.

Result-parity conclusion:

- this run proves the live P8 operator/reporting path, not GEPA-quality parity;
- best candidate remained the seed (`best_index = 0`);
- optimized validation and held-out test did not beat baseline;
- do not cite this run as "as good as GEPA" or paper parity.

Prompt-audit finding:

- the completed report preserved solver/reflection request prompts, but only
  recorded `"output": "Text"` for LM output mode; actual assistant text had to
  be recovered from `.leaven/lm-cache.sqlite`;
- post-run P8 code now records `observed_requests[].response.assistant.content`
  plus provider response/continuation IDs in future reports, so reflection
  proposals and solver transcripts are inspectable from the report artifact
  without another provider call.
- P8 GEPA proposal attempts now include a compact `reflection` object with the
  reflection request index, full request record, assistant text, parsed
  proposed text, and provider response ID so child admission history can be
  audited from the report row itself.
- post-run P8 code now also projects `system_prompt` onto
  `gepa_report.candidates[]` for AIME by combining the seed prompt with
  accepted reflection proposals. This is intentionally P8-local: generic
  `GepaReport` keeps optimizer state and candidate ids, while the AIME report
  carries domain prompt text needed for live-quality audits.
- P8 candidate prompt projection now also emits `system_prompt_source`.
  `seed_config` and `observed_reflection_response` mean the report carries the
  prompt text; `unavailable_process_local_lm_telemetry` means the candidate was
  restored from durable GEPA state but the reflection response was not observed
  in the current process. This keeps resumed reports honest until prompt text is
  recovered from durable graph/checkpoint state instead of process-local LM
  telemetry.
- Public `optimize(...).budget(Budget::metric_calls(n))` now feeds the engine
  metric-call stopper rather than a hard metric-call ledger cap. That matches
  GEPA `max_metric_calls`: started evaluator batches/full validations can
  finish, and the run stops before the next optimizer step. Non-metric budget
  axes remain hard ledger caps.
- GEPA sampler and validation-frontier parent selection now use a shared
  Python `random.Random`-compatible MT19937 implementation for the needed
  `randbelow`/`shuffle` paths. This removes the old splitmix delta and the old
  separate-selector/sampler RNG delta for the reference profile. The public
  sampler hook names only an opaque doc-hidden `GepaRandom`; explicit
  `EpochShuffled::with_seed(...)` still uses sampler-local custom RNG state.
- P8 live-role telemetry now counts malformed DSPy solver output as
  `answer_parse` even when the LM call itself succeeded. LM response-cache
  failures are split into cache-only required misses, read errors, write
  errors, and other cache errors, so cache-only replay reports do not imply a
  failed cache write.
- Live AIME data showed GPT-4.1-mini can answer a DSPy ChainOfThought request
  with plain markdown reasoning followed by `[[ ## answer ## ]]`, omitting the
  `[[ ## reasoning ## ]]` header. Upstream `dspy.ChatAdapter` still treats that
  as a parse failure, but its public call path catches adapter errors and
  reruns through `JSONAdapter`. The P8 solver now mirrors that fallback instead
  of aborting the GEPA evaluation at the first ChatAdapter parse error.
- post-run P8 proposal attempts now cross-link accepted children back to
  `child_index` and `child_validation_score`, so an operator can inspect why
  an accepted train-screening child did or did not become validation-best
  without manually joining candidate ids.
- GEPA proposal attempts now distinguish train-screen acceptance from
  candidate admission with `admitted_index`, and the P8 JSON projects
  `admitted` / `admitted_index`. This matters for the live budget-boundary
  case where a child can pass the train screen but stop before full validation
  and therefore must not be reported as an admitted GEPA candidate.
- Generic `summary.json` and P8 `p8-aime.json` report writes now use
  write-temp/fsync/rename/dir-sync replacement instead of direct writes. This is
  operator-path hardening for the next live release run: interrupted report
  writes should leave either the prior complete report or the next complete
  report, not a torn JSON artifact.
- Successful runner/scorer trace lines now have a public ordinary-run path:
  `RunOutput::with_trace` and `Score::with_trace` feed
  `CaseAssessmentEvidence::with_trace`, generic report trace refs, and P8
  non-empty `trace_refs`. P8 solver outputs attach deterministic/live reasoning
  and raw live solver response text as target-safe runner trace evidence. This
  improves one-prompt AIME/operator proof without claiming DSPy module-local
  trace selection parity.
- Verifier wave 3 found P8 live resume compatibility was still using local
  model/cache/runtime fields but not the observed OpenAI provider fingerprint.
  Current P8 runner and LM-role fingerprints now include the constructed
  provider fingerprint, so timeout/base-url/retry/throttle drift changes the
  durable compatibility manifest before runner/LM work. The old completed live
  report still predates this repair and must not be cited as provider-runtime
  compatibility proof.
- Verifier wave 3 also found the scaffold no-validation policy could mark a
  train-screen accepted child as `admitted_index=Some(_)`. `accept_child` now
  assigns `admitted_index` only when accepted-child validation returns a GEPA
  reference-state index, and `MinibatchThenValidation` is no longer in the
  ordinary `leaven_gepa::prelude` route.

Comparison notes against the older improving artifact:

- older artifact `.leaven/runs/2a582001-8fb7-4cdc-9926-6054ab5a1846` is not a
  clean apples-to-apples proof for the current materialized-cache run: it lacks
  the current dataset proof fields, reports doubled case rows, and starts from
  a different baseline validation score (`0.444` instead of `0.533`);
- the current materialized-cache run accepted eight train-screening children,
  but candidate `5` only tied seed validation (`0.533`) and no child exceeded
  seed validation, so strict result-quality parity remains open;
- the completed report's top-level `cases` listed final train/test rows only
  because final validation reused cache rows from GEPA casewise validation and
  the run summary rebuilt split reports by scanning partition requests;
- post-run `leaven-run` now builds `splits_reported` from explicit final
  evaluation summaries, so future P8 reports keep baseline/optimized
  train/validation/test case rows with a `candidate_role` label even when best
  equals seed and cache hits reuse the same assessment rows.
- direct data inspection found the current failed-quality run's reflection
  proposals were short generic solver instructions, while the older improving
  artifact used longer AIME/contest-specific instructions that forced exact
  integer-only final answers. Future regenerated reports should expose this
  comparison directly through candidate `system_prompt` rows instead of forcing
  reconstruction from `.leaven/lm-cache.sqlite`.

## Existing Prior Live Artifact

Prior report worth comparing, but not sufficient as current proof:

```text
.leaven/runs/2a582001-8fb7-4cdc-9926-6054ab5a1846/reports/p8-aime.json
```

It records a real read-write live run with OpenAI roles and no provider
failures:

- profile: `gepa-aime`;
- target: `gepa_cais_aime_math_artifact`;
- baseline train: `0.6`;
- optimized train: `0.6666666666666666`;
- validation: `0.5555555555555556`;
- held-out test: `0.5666666666666667`;
- search metric calls: `472`;
- live role calls: `440`;
- live role cache policies: `read-write,read-write`;
- live role cache misses: `276`;
- provider failures: `0`.

It predates the current materialized-cache hash/report proof stack and should
not by itself close the current live release row.

## Next Actions

Cache/replay attempts after the report-schema fixes:

- fresh cache-only replay into
  `.leaven/release-runs/p8-aime-gepa-cache-replay-20260518-071621` failed
  closed before provider work with `lm response cache failed: required lm cache
  entry was missing`. A fresh run does not reproduce the exact reflection
  prompt/cache key stream from the completed live run.
- cache-only replay against the completed run directory
  `.leaven/release-runs/p8-aime-gepa-20260518-043717` failed compatibility with
  `stored runner fingerprint does not match live runner fingerprint` because
  changing LM cache policy from read-write to cache-only changes the live role
  fingerprint.
- current-code JSON-fallback cache probe in
  `.leaven/release-runs/p8-aime-gepa-current-json-fallback-cache-probe` failed
  closed before provider work at the first reflection call:
  `lm response cache failed: required lm cache entry was missing`. It reached
  seed validation plus one train minibatch (`metric_calls=48`, `llm_calls=0`),
  wrote one durable `reflection/cache` failure row, and did not reach solver
  ChatAdapter/JSONAdapter fallback. The next paid run must expect missing
  reflection cache rows before any solver-fallback cache deltas matter.
- post-run P8 error output now prints safe profile/cache/run-dir/proof context
  on failures, so future cache-only or compatibility refusals are diagnosable
  without reconstructing the shell environment from logs.
- durable SQLite evaluation-cache flushing now stops at the optimizer search
  checkpoint boundary. Final-report-only evaluations remain available to the
  returned summary/report but are not persisted into `run.sqlite` as resume
  authority unless a separate report-resume snapshot is introduced later.
- P8 role report fingerprints now reuse the actual solver/reflection runtime
  fingerprints supplied to `.runner_fingerprint(...)` and
  `.lm_role_fingerprint(...)`, including OpenAI runtime timeout/throttle
  configuration and provider fingerprint. This keeps report disclosure aligned
  with compatibility checks.
- current-code live run
  `.leaven/release-runs/p8-aime-gepa-current-json-fallback-20260518-084304`
  reached 200 metric calls and 130 LM calls. It proved the DSPy JSONAdapter
  fallback path in live traffic: the first accepted-child full validation
  evaluated 45 rows with 46 LM calls instead of aborting on the missing
  `reasoning` header shape. The run then failed closed with
  `GEPA evaluation returned a row for the wrong candidate` after zero-cost
  casewise cache hits for a same-content/new-candidate request. Root cause:
  engine evaluation cache keys use candidate content identity, but cache values
  were raw assessment ids whose target still named the older candidate id.
  Fixed in current code: casewise content-cache hits rematerialize zero-cost
  assessment rows for the requested candidate.
- cache-only resume against that same run after the engine fix replayed the
  three formerly bad rows as zero-cost cache hits, did not reproduce the
  wrong-candidate assertion, and failed closed at the next uncached reflection
  request with no additional LM calls. Log:
  `.leaven/release-runs/p8-aime-gepa-current-json-fallback-20260518-084304.cache-only-after-engine-fix.log`.
- read-write resume against that same run after the engine fix replayed the
  same zero-cost cache hits, then spent 12 additional metric calls before being
  stopped because the compile log showed it predates latest P8
  profile/failure-report slices. Log:
  `.leaven/release-runs/p8-aime-gepa-current-json-fallback-20260518-084304.resume-after-engine-fix.log`.
- failed P8 runs with `LEAVEN_AIME_RUN_DIR` now write
  `reports/p8-aime-failure.json` with safe profile/runtime/error context, so the
  next failed live attempt does not depend on terminal scrollback for diagnosis.
- configured P8 run directories now write `reports/p8-aime-start.json` before
  optimizer/provider work. This fixes the run-operator rough edge exposed by
  the active release run: profile/model/cache/timeout/budget facts no longer
  depend on an ad hoc shell wrapper if the process is interrupted before final
  or failure reports.
- now-historical current-binary release run completed before the
  `GepaProfile::OptimizeAnything` default slice:
  `.leaven/release-runs/p8-aime-gepa-current-release-20260518-094902-d2d15a36d364`.
  It uses `gepa-aime`, reference GEPA profile, read-write solver/reflection LM
  cache policies, eager SQLite LM cache, 600s request timeout, and 32 OpenAI
  workers. Its original operator note recorded the temporary empty jj child
  `d2d15a36d364`; that child had the same source tree as current proof-boundary
  commit `427067b745d5` after cleanup, and the run-local `operator-notes.txt`
  now records that correction plus solver/reflection models.
- terminal proof: run id `ec039f2d-45b0-4cfa-8615-4d21dcdfbfda`, latest
  checkpoint `bb29dd4b-0ef5-4fc2-b979-618e50153a7f`, optimizer wall time
  `4941717` ms, `reports/summary.json` and `reports/p8-aime.json` emitted.
- budget proof: search stopped with `BudgetReached`, spent `530/500` search
  metric calls with `30` calls of GEPA-style started-work overshoot, then spent
  `150` final-report metric calls; total reported budget was `680` metric calls
  and `401` LM calls.
- report proof: proof class `full_live_aime_reproduction_attempt`,
  `gepa_profile=reference`, live provider proof for solver and reflection,
  zero process-local and durable provider failures, solver cache
  `hits=300 misses=382`, reflection cache `hits=21 misses=19`, and source
  splits/counts/hash match the AIME cache proof.
- GEPA proof: `gepa_best_index=3`, `candidate_count=7`,
  `proposal_attempt_count=40`, `accepted_count=6`,
  `accepted_unadmitted_count=0`, and `full_validation_evals=7`.
- score proof: baseline/optimized validation `0.444 -> 0.489`; baseline/
  optimized held-out test `0.433 -> 0.500`; optimized train was lower
  `0.600 -> 0.556`. This is a real live improvement over seed on validation
  and held-out test, but remains below the pinned GEPA CAIS target `0.600`.
  The next paid report must be treated as the current-profile release proof and
  should show `gepa_profile=optimize-anything`.
- no-spend case-delta audit of the completed report: train changed by
  `4` improved / `6` regressed / `35` unchanged cases; validation changed by
  `6` improved / `4` regressed / `35` unchanged cases; held-out test changed
  by `4` improved / `2` regressed / `24` unchanged cases. The next P8 report
  schema now emits these target-safe deltas directly as `case_deltas` instead
  of forcing auditors to reconstruct them from per-candidate rows.
- post-run audit found the completed report's aggregate metric counters were
  internally consistent, but `gepa_events` did not carry seed-validation or
  accepted-full-validation metric deltas. The owning event contract now carries
  validation score plus `metric_calls_delta` for both validation-completed
  phases so future reports can reconstruct GEPA search metric spend from the
  phase stream itself. The current release artifact predates that schema fix.
- Audit-agent wave after the stale no-improvement report found no obvious
  prompt-contract, parser, dataset-split, or source-id mismatch explaining the
  old `best_index=0` result. Their useful diagnosis is that the stale completed
  run's seed was stronger on the same validation case IDs than the older
  improving run's seed, while accepted children did not beat the seed after full
  validation. Treat model stochasticity/cache state as a plausible explanation,
  not a proof: the current live report must still show whether current code
  improves seed and how its emitted reflections/proposals compare to upstream.
- Several audit findings were stale against the current tree: answer-parse
  telemetry, LM cache required-miss/read/write split, and P8 role fingerprints
  now have current-code tests/implementation. Re-check live report artifacts
  before reopening those as active gaps.
- Post-wave stale finding dispositions against current code:
  - DSPy ChatAdapter same-line output fields and required ChainOfThought
    `reasoning` are already covered by `aime_solver_parser_accepts_dspy_same_line_field_content`
    and `aime_solver_parser_requires_all_dspy_output_fields`;
  - P8 JSON already discloses `comparison.upstream_reflection_model` beside
    the Leaven reflection model;
  - public `Budget::metric_calls(...)` is converted into a GEPA-style
    metric-call stopper by `search_ledger_budget(...)`, leaving the hard ledger
    unlimited for metric calls so started validation can finish;
  - `EpochShuffled` and parent selection now use the doc-hidden
    Python-compatible GEPA RNG, with seed/shuffle sequence tests;
  - P8 role fingerprints ignore LM cache policy/backend replay controls, so
    switching a paid run to `cache-only` / `eager-sqlite` should not block
    resume compatibility;
  - stale after the latest cache audit: `leaven-run` no longer flushes
    final-report-only rows into the durable evaluation cache, because those
    assessment ids are not graph-valid from the search checkpoint resume point.
- Current cache/resume audit reopened two real P8 replay gaps and one deeper
  engine/run-dir gap:
  - fixed in current code: `cache-only` P8 OpenAI replay now constructs the
    provider identity with a placeholder key, because OpenAI fingerprints do
    not include the secret and `CacheOnly` never calls the provider on a miss;
  - fixed in current code: `eager-sqlite` now reads the selected run-dir
    `lm-cache.sqlite` first, falls back to workspace `.leaven` cache, and
    writes through to the workspace cache. Exact paid-run replay takes
    precedence over compatible workspace reuse so a stochastic response from
    another run cannot mask the selected run-dir row;
  - fixed conservatively in current code: final-report evaluation-cache rows are
    no longer flushed to SQLite when the latest resume point remains the search
    checkpoint. Do not claim completed final-report replay proof until either
    the final graph checkpoint becomes the resume target without corrupting
    search budget semantics or the evaluation cache can restore/backfill
    report-visible assessment rows.
- Final verifier wave disposition:
  - still open and current: result-quality parity. The historical completed
    report improved seed but still trails the pinned GEPA CAIS target, so
    another paid run should be justified by a specific profile/model
    experiment, not by hope that duration alone fixes it;
  - partially closed: strict upstream-reflector reporting proof. Cache-only
    rehearsal with `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1` now proves the
    start/failure reports classify `openai/gpt-5.1` vs `gpt-5.1` as
    `upstream-matched` without provider spend. The latest current-code rerun
    also proves `gepa_profile=optimize-anything` and failure-report
    role/cache/provider-failure evidence. The paid quality proof is still open
    because the cache lacks the required `gpt-5.1` reflection row;
  - still open and current: final-report replay proof. Solver/reflection LM
    cache replay is proven for search checkpoints, but final-report-only rows
    are not yet checkpoint-restored as report-visible graph evidence. A
    no-edit audit on 2026-05-18 found no small safe fix: `leaven-run` flushes
    the SQLite evaluation cache from the search checkpoint before final
    reporting, then runs final train/validation/test report evaluations against
    the live engine, and deliberately points latest checkpoint back at the
    search checkpoint. `RunContext` refuses cached assessment ids absent from
    the restored graph, and `run_builder_sqlite_cache_keeps_only_search_rows_at_search_checkpoint`
    locks in that final-report-only rows are not resume-authoritative. Do not
    "fix" this by flushing final-report rows into the search checkpoint cache;
    the next safe design is either a post-final-report snapshot contract or a
    report-row backfill/materialization contract.

2026-05-18T12:23:38Z:

- Local upstream artifact audit anchored the GEPA CAIS AIME comparison target to
  `/Users/darin/vendor/github.com/gepa-ai/gepa-cais26-artifact/acm_cais_artifact_evaluation/domains/aime_math/`.
  The useful local files are `logs/best_prompt.txt`, `logs/gepa_state.bin`,
  `logs/aime_plot.png`, and `logs/generated_best_outputs_valset/**`.
  `README.md` and `OFFLINE_ARTIFACTS.md` both describe `logs/run.log` as the
  source for the `46.67% -> 60.00%` test line and `57.78%` validation line, but
  the file is not present in this local checkout. Treat those score lines as
  artifact-provenance claims unless/until the missing log is recovered; use the
  bundled checkpoint/best prompt/validation outputs for local qualitative
  comparison.
- Upstream `optimize_anything.py` AIME config confirms the strict comparison
  knobs: solver `gpt-4.1-mini` at `temperature=1.0`, `max_tokens=32000`,
  `max_metric_calls=500`, `parallel=True`, `max_workers=32`,
  `cache_evaluation=True`, `frontier_type="instance"`, and reflection
  `openai/gpt-5.1`. P8
  `configured_gepa_aime_profile_matches_optimize_anything_knobs` covers the
  Leaven-side runtime/profile knobs except the deliberate reflection model
  delta (`gpt-5.4-mini` by default, `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1` for
  strict upstream-reflector comparison).
- Upstream `InstructionProposalSignature.prompt_renderer` renders
  optimize-anything side-info records as Markdown sections:
  `# Example N`, `## key`, nested `###` mappings/lists, and plain trimmed
  values. Leaven's GEPA reflection renderer and P8 AIME side-info projection
  already match that structural format for the target-safe keys `score`,
  `input`, `prompt`, `output`, `reasoning`, and `execution_feedback`.
- The CAIS best prompt instructs the solver to reason thoroughly and isolate a
  single final line. Leaven's current best prompt from the completed paid run is
  stricter about returning only the final integer. That is an optimization
  outcome/quality gap rather than a confirmed renderer mismatch; do not claim
  model-experience parity from the current run because the search result still
  trails the pinned target (`0.500` held-out test versus `0.600`).
- Loading the local `logs/gepa_state.bin` with upstream `GEPAState.load()`
  showed `i=32`, 10 candidates, instance frontier, `621` total metric calls,
  candidate validation scores `[0.4667, 0.4889, 0.5111, 0.4444, 0.4889,
  0.4000, 0.5778, 0.4889, 0.5556, 0.5556]`, and candidate-discovery metric
  calls `[0, 75, 123, 183, 231, 291, 363, 459, 513, 579]`. Leaven's current
  completed P8 report stopped with 7 candidates, best validation `0.4889`, and
  search metric calls `530/500`. A later source audit found current AIME source
  leaves `num_parallel_proposals` at the default `1`, so source parity is serial
  proposals plus 32-way case evaluation. Because `run.log` is missing, record
  the checkpoint as an inspectable artifact/source conflict with the documented
  500-call config, not as proof that Leaven should raise its cap or implement
  proposal fanout for the current-source AIME profile.
- The same checkpoint exposes upstream candidate prompt history but not
  proposal/reflection assistant text. `full_program_trace` has 33 compact rows
  with selected parent, three validation ids, old/new train-screen scores, and
  accepted child index/full-validation marker; `adapter_state` is empty. The
  checkpoint therefore supports prompt-outcome comparison, not a direct
  accepted/rejected reflection-text diff.
- The CAIS prompt trajectory is materially more directive than the current
  Leaven live outcome. The upstream winning candidate is candidate `6`, parent
  `[4]`, validation `0.5778`, and `1627` prompt chars; it keeps concrete
  directives for restating the problem, setting notation/constraints, naming
  applicable theorems, handling dead ends, avoiding approximation, and placing
  the final answer alone. Later upstream candidates `8` and `9` are even more
  specialized (`2372` and `6705` chars) while scoring `0.5556`. The completed
  Leaven report has 7 candidates, best validation `0.4889`, and best candidate
  `3` is `760` prompt chars; accepted child prompts often stayed generic or
  overfit to a few topics. This keeps the quality gap centered on reflection
  model/search outcome, not on the already-proven optimize-anything prompt
  renderer.
- Bead tracker check on 2026-05-18 found only one open GEPA/reflection design
  task besides the P8 proof epic: `leaven-338`, "Multi-part
  (multi-component) GEPA reflection as an opt-in path." Its own constraints say
  single-part `RoundRobinPart` reflection stays the GEPA-parity default and
  multi-part reflection is an explicit divergence for coupled agent kits. Do
  not block current GEPA/AIME parity on this bead or silently fold it into
  `GepaProfile::Reference` / `GepaProfile::OptimizeAnything`; if implemented,
  it needs a named opt-in profile/surface and separate proof.
- Intentional delta still current: `FastCertified` and future FastGEPA ideas
  are Leaven-plus profiles, not reference parity. Spec the profile before
  adding lazy certification, active sampling, async islands, evaluator pyramids,
  or trace distillation to ordinary GEPA.
- No-spend prompt/trace diagnosis of the completed paid report:
  - accepted children came from same-case train-screen wins, but several were
    classic minibatch overfits. Accepted attempts were `15` (`1/3 -> 2/3`,
    validation `0.400`), `23` (`0/3 -> 1/3`, validation `0.4667`), `24`
    (`1/3 -> 3/3`, validation `0.4889`, current best), `25` (`2/3 -> 3/3`,
    validation `0.3778`), `27` (`1/3 -> 3/3`, validation `0.4444`), and `40`
    (`1/3 -> 3/3`, validation `0.4889`);
  - the reflection request for accepted attempt `15` included a correct example
    and two rich incorrect examples with full `execution_feedback`, including
    the `244` region-count solution and the `127` torus solution. The rendered
    prompt shape matched optimize-anything, but the `gpt-5.4-mini` response was
    a short generic first-principles instruction. This is the dominant
    qualitative difference versus the bundled CAIS best prompt, which retained
    detailed directives about notation, constraints, theorem use, dead-end
    handling, exact arithmetic, and answer isolation;
  - current-source prompt/render parity therefore remains plausible, but model
    experience parity is not established by the paid run because it used the
    deliberate `gpt-5.4-mini` reflection-model delta and produced less specific
    reflection edits than the CAIS checkpoint. The next paid quality run should
    either use strict `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1` or be labeled as a
    Leaven-plus model/profile experiment.

2026-05-18T14:13:00Z:

- P8 report UX now turns the prompt-specificity audit into report data instead
  of ad hoc JSON scraping: `reflection_summary` includes
  `accepted_proposed_text_chars` and `rejected_proposed_text_chars`, each using
  the existing length-summary shape. This is diagnostic-only and does not feed
  GEPA parent selection, acceptance, admission, or final result selection.
  `docs/specs/p8_run_report_operator_ux.md` now names the accepted/rejected
  breakdown as part of the operator report contract.

2026-05-18T14:18:00Z:

- No-spend parser audit found one small Leaven-looser-than-upstream mismatch in
  `PlainTextEditParser`: upstream `InstructionProposalSignature.output_extractor`
  strips a fence language line only when it is an immediate non-whitespace
  token after the opening fence. Leaven trimmed leading whitespace before
  language detection, so ``` text\n...``` was parsed as if `text` were a
  language label. Current code now matches the upstream extractor and
  `plain_text_parser_matches_upstream_language_fence_detection` pins the edge.
  `plain_text_parser_matches_upstream_output_extractor_cases` ports the
  upstream `tests/test_instruction_proposal.py` extractor table through
  Leaven's public parser path. This is a source-parity fix, not a
  quality-profile change.

1. Continue no-spend quality diagnosis only where it can produce new evidence:
   use emitted Leaven prompts, candidate lineage, reflection outputs, child
   admission history, minibatch cases, parser outcomes, and the CAIS checkpoint
   candidate prompt trajectory. Future current-code reports can start from the
   accepted/rejected proposed-text summaries before drilling into individual
   attempts. Do not wait for upstream reflection assistant text from this
   checkout; `gepa_state.bin` did not persist it and `run.log` is absent.
2. Treat result-quality parity as open until a current-profile live report with
   comparable held-out quality exists, or the specs label the remaining
   model/runtime delta. The clean paid comparison is a strict upstream-reflector
   run with `LEAVEN_AIME_REFLECTION_MODEL=gpt-5.1`; Leaven-plus profile
   experiments should be labeled as such before spend.
3. Future release reports should be generated with the post-run event-schema
   fix so live-proof checks can sum `gepa_events[*].metric_calls_delta` back to
   the GEPA report metric total directly. The historical completed release
   report remains valid for aggregate budget proof but predates those per-phase
   validation deltas.
4. Current-code release reports include `case_deltas.summary` and
   `case_deltas.cases` so quality diagnosis starts from exact improved,
   regressed, unchanged-correct, and unchanged-wrong case IDs without exposing
   raw hidden targets or reference solutions. The historical paid report
   predates this schema, and cache-only regeneration from its run directory is
   blocked by an intentional runner-fingerprint mismatch after the DSPy
   JSON-fallback runner cutover.
