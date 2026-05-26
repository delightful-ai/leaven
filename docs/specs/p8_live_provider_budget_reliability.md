# P8 Live Provider Budget Reliability

Status: implementation spec.

This spec defines the live-provider, cost, retry, and budget semantics P8 needs
before AIME via Leaven can be called real. It builds on:

- `gepa_aime_paper_parity.md`;
- `default_cache_storage.md`;
- `lm_runtime_and_response_cache.md`;
- `durable_runs_and_resume.md`;
- `resume_compatibility_fingerprints.md`;
- `p8_run_report_operator_ux.md`.

## 1. Product Rule

P8 live mode may spend provider resources. Every costful boundary must be
metered, durable, and resumable:

- solver LM calls;
- reflection LM calls;
- scorer calls if they use a provider;
- retries that reach a provider;
- final train/validation/test report work.

No provider spend may hide inside example-local logs or unmetered closures.

## 2. Budget Semantics

`Budget::metric_calls(500)` in the AIME GEPA path is the search budget. It
controls optimizer search work, not final report work.

Required distinctions:

- search budget: optimization loop evaluations/proposals/reflection;
- final-report budget: final train/validation/test reporting;
- provider cost: LM calls, tokens, cached-token usage, and provider-specific
  reported usage;
- metric calls: benchmark-level unit used for GEPA parity.

Budget reached is a clean stop:

```text
if search_spent.metric_calls >= search_budget.metric_calls:
    checkpoint clean boundary
    return current best with StopReason::BudgetReached
```

`BudgetExceeded` is a refusal to charge a work item that would exceed hard
limits. It is not the normal P8 `max_metric_calls` stop path.

## 3. Parallel Overshoot

Parallel evaluation may schedule multiple in-flight jobs. Overshoot is allowed
only if:

- all overshoot work was already scheduled before the cap was observed;
- no new optimizer step starts after the cap is observed;
- the report records actual spent metric calls honestly;
- resume starts from the clean checkpoint after the completed in-flight batch.

The report must distinguish cap, spent, and overshoot.

## 4. Provider Runtime Identity

Each live role has a runtime fingerprint:

- role id, for example `aime_solver` or `gepa_reflector`;
- provider family;
- model id;
- sampling/options;
- output format/parser;
- prompt/template identity;
- max output tokens;
- concurrency policy when it can affect behavior;
- cache policy/mode and namespace.

Fingerprints must exclude API keys, bearer tokens, organization secrets, and
local cache paths.

Changing provider/model/sampling/parser must refuse incompatible resume or use a
new cache namespace.

## 5. Retry And Idempotency

Provider retries must be explicit:

- retry policy belongs in provider/runtime config;
- retry attempts that reach a provider are metered if the provider reports cost;
- retry attempts that fail before provider execution do not charge provider cost;
- idempotency keys or request ids should be stable where the provider supports
  them;
- cached LM responses must not call the provider and must report zero new cost.

If Leaven cannot tell whether a provider charged for a failed attempt, the
report must use an explicit unknown/estimated state rather than silently dropping
cost.

## 6. LM Cache Cost Rules

LM response cache hits:

- return the original response and usage record;
- charge zero new provider cost;
- preserve original usage for audit;
- increment hit counters;
- must not consume metric-call budget as a new provider call.

LM response cache misses:

- call the provider;
- record provider usage/cost;
- write cache entry when policy allows;
- surface write failures separately from provider success.

Non-replayable requests bypass with reason.

## 7. Evaluation Cache Cost Rules

Engine evaluation cache hits:

- reuse completed assessments/evidence;
- charge zero new runner/scorer/metric cost;
- do not call runner/scorer/provider;
- preserve evidence refs and cache status in report.

Evaluation cache misses:

- evaluate normally;
- record cost from runner/scorer output;
- write cache identity only after graph/evidence records are consistent.

Unsafe evaluator/scorer paths bypass even when storage exists.

## 8. Provider Failure States

Typed live-provider failures must distinguish:

- missing credentials;
- provider authentication failure;
- rate limit/throttle;
- retry exhaustion;
- malformed provider response;
- answer parse failure;
- scorer parse failure;
- budget refusal before provider call;
- cache read/write failure.

P8 reports should summarize failures without exposing secrets or full raw
provider payloads by default.

## 9. Durable Evidence

Live provider work must leave durable evidence:

- provider-neutral request fingerprint;
- provider/model/role;
- response id when available;
- token usage and cost;
- cache status;
- output text or blob ref;
- parse result;
- error class for failures.

Raw provider payloads may be blob-stored under an explicit debug/audit policy,
but public reports should default to summaries and evidence refs.

## 10. P8 Requirements

The P8 live run must report:

- solver model and reflection model;
- solver/reflection role fingerprints;
- search metric-call cap and spent calls;
- final-report metric calls separately;
- LM calls and token usage by role;
- cache hit/miss/bypass by role;
- provider failures and retry counts;
- whether the run is deterministic smoke, live solver, live reflection, or full
  live reproduction attempt.

The deterministic default must not pretend to prove live provider quality. Live
paths must be explicit opt-ins.

## 11. Implementation Routing

- `leaven-lm` owns provider-neutral usage and request/response vocabulary.
- `leaven-lm-cache` owns LM response cache hit/miss cost semantics.
- `leaven-lm-openai` owns OpenAI retry/throttle/auth mapping.
- `leaven-engine` owns budget charging and cache hit zero-cost behavior.
- `leaven-run` owns product summary/report projection.
- `examples/p8_aime_gepa` owns AIME role wiring and live-mode CLI/env policy.

Do not put OpenAI-specific retry details in GEPA. Do not make P8 shell out to a
Python provider path for live proof.

## 12. Proof Requirements

Required tests:

- budget reached returns a clean resumable result with current best;
- cache hit for evaluation charges zero metric calls and skips runner/scorer;
- cache hit for LM charges zero new provider cost and preserves original usage;
- retry failure is typed and summarized without secrets;
- final report work does not consume search budget by default;
- parallel overshoot is recorded honestly;
- live-mode report includes provider/runtime/cache/budget summaries.

Focused commands:

- `cargo test -p leaven-engine --test engine_contract context_services`
- `cargo test -p leaven-engine --test engine_contract engine_loop`
- `cargo test -p leaven-lm-cache`
- `cargo test -p leaven-lm-openai`
- `cargo test -p p8_aime_gepa`
- `cargo test -p leaven --test topology_contract`

Completion gate remains `just check`.
