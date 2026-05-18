# GEPA Parity Working Ledger

Status: active.
Updated: 2026-05-18T09:27:04Z.

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
or intentional deltas. The remaining P0 row is P8/AIME result parity: a live
release run/report now proves the operator path, source counts/cache hash,
cache/resume behavior, search budget, and model profile. The completed report
predates durable provider-failure counters, so it cannot by itself prove zero
provider failures across the failed/resumed release attempt. The completed run
also did not improve over the seed and therefore does not prove "as good as
GEPA" benchmark quality.

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
- parallel proposal workers, lazy validation/certification, active failure
  sampling, evaluator pyramids, and trace distillation should be modeled as
  explicit follow-on library profiles/seams, not P8-only patches;
- cheap/proxy stages may filter or prioritize, but only the real evaluator and
  full validation can admit/crown reference candidates;
- current concrete speed work is observability plus safe opt-in API shape:
  expose accepted-but-unadmitted children, attempt counts, and validation
  counts in reports so long runs can be cut off or resumed from data, and keep
  `proposal_count` labeled as serial rather than async island GEPA.

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

## Current Audit-Wave Disposition

2026-05-18 verifier wave disposition against the current tree:

- true blocker: the completed live report
  `.leaven/release-runs/p8-aime-gepa-20260518-043717/reports/p8-aime.json`
  remains stale proof and result-quality gap evidence only. It does not prove
  current report schema, zero durable provider failures, or "as good as GEPA"
  result quality because it kept `best_index=0` and `baseline == optimized`;
- true blocker: the current in-flight JSON-fallback run below must finish and
  emit reports before its profile/model/dataset/cache/budget/provider-failure
  and result fields can be used as proof;
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

Current in-flight JSON-fallback run:

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
  failure file had been emitted yet;

Do not treat this run as a completed live proof until it emits reports and the
profile/model/dataset/cache/budget/provider-failure/result fields are inspected.
Do not start another provider run while this one is active.

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

1. Diagnose why the completed live run produced no improvement while the older
   live artifact improved validation/test. Start from emitted prompts,
   candidate lineage, reflection outputs, child admission history, minibatch
   cases, and parser outcomes.
2. Diff the completed live report against the prior live artifact and upstream
   GEPA/DSPy AIME traces where available. Treat result-quality parity as open.
3. Regenerate a live P8 report after the durable provider-failure field lands
   before using resumed live runs as provider-reliability proof.
4. Re-run live P8 only after the diagnosis identifies a concrete fix or
   intentional runtime/profile delta worth testing.
5. Future release reports should be generated with the post-run report-schema
   fixes so live-proof checks do not have to re-aggregate role telemetry by hand
   and so resumed `gepa_events` are cumulative.
