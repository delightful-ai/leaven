# Default Cache Storage

Status: implementation spec.
Date: 2026-05-15.

This spec defines the default cache/storage behavior for durable Leaven runs. It
does not re-open optimizer checkpoint format decisions. It establishes that the
ordinary product surface should not ask users to separately configure run
durability, evaluation cache storage, and LM response cache storage.

It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/durable_runs_and_resume.md`
- `docs/specs/lm_runtime_and_response_cache.md`
- `docs/specs/case_visibility_and_target_isolation.md`

## 1. Product Rule

Durable run configuration is one product concern.

For ordinary users, this:

```rust
optimize(seed)
    .run_dir(".leaven/runs/aime")
    .runner(solver)
    .score(score)
    .using(Gepa::for_surface(surface))
    .budget(Budget::metric_calls(500))
    .run()
    .await
```

must provision the whole durable substrate:

- run checkpoints and resume metadata;
- evidence and report storage;
- engine evaluation-cache storage;
- LM response-cache storage for LM roles used by the run;
- cache status and bypass reasons in reports;
- clean refusal when a required durable component cannot be opened.

The ordinary user should not have to choose one knob for resume, another for
engine assessment caching, and a third for LM response caching. Advanced knobs
may exist for debugging and policy overrides, but omitted configuration must be
usable and safe.

## 2. Default Local Layout

The default local run directory remains the per-run root:

```text
.leaven/runs/<run-id>/
```

The durable local layout should converge on:

```text
.leaven/runs/<run-id>/
  run.sqlite
  lm-cache.sqlite
  blobs/
  checkpoints/
    LATEST
    <checkpoint-id>.checkpoint
  evidence/
    <evidence-key>.json
  reports/
    summary.json
```

`run.sqlite` is the default structured store for run-indexable state, including
evaluation cache rows. `lm-cache.sqlite` is the default LM response cache store.
Separate SQLite files are preferred so provider-cache operations can have their
own schema, migration cadence, and locking behavior without coupling to engine
checkpoint internals.

`blobs/` and `checkpoints/` remain valid for graph snapshots, checkpoint
envelopes, large evidence payloads, and compatibility with the current
`FileStore` implementation. JSON is still acceptable for typed checkpoint blobs,
optimizer continuation payloads, and human-readable sidecars.

## 3. SQLite Defaults

SQLite is the default durable backend for cache-like state because cache entries
need keyed lookup, replacement, versioned schema, and robust crash behavior.

Default SQLite requirements:

1. Use WAL mode for local durable runs unless the platform refuses it.
2. Use bounded busy timeouts instead of immediate lock failure.
3. Use explicit schema versions stored in each database.
4. Use transactions for multi-row writes.
5. Treat corrupt or incompatible databases as typed store errors; do not silently
   clear caches.
6. Keep secrets out of cache keys and rows.
7. Store canonical key bytes or fingerprints plus enough debug metadata to audit
   cache behavior without reconstructing provider secrets.

JSON remains acceptable for:

- checkpoint envelopes;
- optimizer/stage continuation snapshots;
- low-volume evidence sidecars;
- human-readable summaries;
- tests and explicit file-store fixtures.

JSON is not the preferred default for high-volume cache lookup tables.

## 4. Engine Evaluation Cache

The engine evaluation cache deduplicates completed assessment work. It is not
the LM response cache and it is not optimizer private state.

Default product behavior:

1. Durable runs provision evaluation-cache storage automatically.
2. The product-builder default is automatic: ordinary durable
   `optimize(...).run()` / `.run_dir(...)` uses deterministic candidate/case
   caching once runner, scorer, case, split, and candidate identities are
   available.
3. Explicit `CachePolicy::Never` remains the throwaway/debug policy for
   ephemeral runs, smoke fixtures, or evaluators that intentionally refuse
   replay.
4. When an evaluator declares deterministic caching and required identities are
   present, the engine uses the cache without an extra user knob.
5. When the cache cannot be used, the report records an explicit bypass reason.
6. Cache hits do not charge new metric calls or provider/runtime cost.
7. Cache writes are part of clean durable evaluation completion. Resume must not
   pretend an assessment completed unless its graph/evidence/cache records are
   consistent.

SQLite table shape should be equivalent to:

```text
evaluation_cache_entries
  key_hash primary key
  key_json_or_bytes
  assessment_ids_json
  evaluator_fingerprint
  case_set_version
  created_at
  last_hit_at
  hit_count
```

The exact schema belongs with the implementation, but it must preserve the
semantic key defined by `leaven-engine::EvaluationCacheKey`:

- evaluator fingerprint;
- cache policy;
- case-set version;
- resolved case ids;
- candidate cache identities.

Case input, target, and scorer-projected metadata must affect cache validity via
the evaluator/scorer fingerprint, case-set version, or a future explicit case
content fingerprint. Pure provenance metadata such as `source_id` should not
invalidate cached scores unless scorer logic reads it.

## 5. LM Response Cache

The LM response cache memoizes provider-neutral LM completions before they become
optimizer proposals, runner outputs, judge scores, or feedback.

Default product behavior:

1. Durable runs provision a SQLite LM response-cache store automatically.
2. LM calls made through Leaven LM roles use the run's LM cache by default when
   the request is replayable under the LM cache key contract.
3. Non-replayable or explicitly no-cache LM calls bypass with a recorded reason.
4. Cache hits return zero metered cost while preserving original provider usage
   in the cached response.
5. OpenAI prompt caching remains provider-side prefix reuse and does not replace
   Leaven's LM response cache.

SQLite table shape should be equivalent to:

```text
lm_cache_entries
  key_hash primary key
  provider_fingerprint
  model
  request_json
  key_json_or_bytes
  entry_json_or_bytes
  response_json
  stored_at
  last_hit_at
  hit_count
```

The semantic key remains the one in `docs/specs/lm_runtime_and_response_cache.md`:

- provider fingerprint;
- model name;
- full canonical messages;
- sampling options;
- output mode/schema;
- provider hints that affect behavior or routing.

The key must not include API keys, bearer tokens, provider response ids,
continuation tokens, wall-clock time, process ids, or backend file paths.

## 6. Automatic Policy

The user-facing default is automatic safe caching, not a stack of knobs.

The default cache mode for a durable run is:

```rust
CacheMode::Auto
```

`Auto` means:

- storage is opened and available;
- deterministic engine evaluations use the eval cache when their evaluator
  policy and identities allow it;
- LM requests use the LM response cache when the request and role are replayable;
- unsafe cache attempts bypass and report why;
- no stochastic or identity-unsafe work is cached merely because a SQLite file
  exists.

Advanced overrides may exist:

```rust
.cache_mode(CacheMode::Auto)
.cache_mode(CacheMode::ReadOnly)
.cache_mode(CacheMode::Refresh)
.cache_mode(CacheMode::Disabled)
```

These are whole-run defaults. Role-specific overrides may exist for expert use,
but they must not be required for P8 or ordinary GEPA runs.

`ephemeral()` changes storage, not semantics:

- evaluation cache may be in-memory only;
- LM cache may be in-memory only or disabled;
- reports must say the run is non-resumable and caches are non-durable.

## 7. Run Result And Reports

Run results and reports must expose cache facts without forcing users to inspect
SQLite manually.

Required report facts:

- run id;
- run dir;
- storage mode: durable or ephemeral;
- cache mode;
- eval cache counts: hit, miss, bypass by reason, write errors;
- LM cache counts by role/model: hit, miss, bypass by reason, write errors;
- whether cache hits charged zero cost;
- latest checkpoint reference.

Per-assessment reports must preserve the existing engine `CacheStatus`. LM call
reports should use an analogous status vocabulary:

```rust
pub enum LmCacheStatus {
    Hit,
    Miss,
    Bypassed(LmCacheBypassReason),
    WriteFailed,
}
```

Provider-cache status must not be collapsed into free-text metadata.

## 8. Ownership

Ownership boundaries:

| Concern | Owner |
| --- | --- |
| Whole-run default cache mode and run-dir wiring | `leaven-run` |
| Engine evaluation cache key/status and graph-visible assessment reuse | `leaven-engine` |
| Evaluation cache SQLite backend | store/backend crate, composed by `leaven-run` |
| LM response cache key/policy/store trait | `leaven-lm-cache` |
| LM response cache SQLite backend | `leaven-lm-cache` or a focused backend crate |
| Provider request lowering and provider-side prompt caching | provider crates |
| GEPA continuation state | `leaven-gepa` via engine optimizer-state hooks |
| Checkpoint envelopes and graph/budget restore | `leaven-engine` |

Do not put LM response cache rows in GEPA optimizer state.

Do not make provider crates depend on the run graph.

Do not make examples assemble cache backends manually for the ordinary path.

## 9. P8 Requirements

For P8 AIME paper-parity runs:

1. `.run()` or `.run_dir(path)` opens durable cache storage automatically.
2. AIME solver and reflection LM calls use the run LM cache when replayable.
3. GEPA evaluation requests use engine eval cache when the scorer/evaluator is
   declared deterministic and candidate/case identities are safe.
4. Cache hits do not spend new metric calls.
5. Reports include source ids, cache status, run dir, checkpoint, and cost.
6. P8 code must not define a P8-private LM cache schema. Until generic LM-role
   provisioning is owned by `leaven-run`, P8 may instantiate
   `leaven-lm-cache::SqliteLmCache` against `<run-dir>/lm-cache.sqlite` as the
   example-level bridge. It may also expose an explicit eager workspace-cache
   mode backed by `.leaven/lm-cache.sqlite` for release reruns that should reuse
   prior compatible LM responses without resuming the whole run directory.

If P8 must turn caching off for a smoke test, it should use an explicit whole-run
override or `ephemeral()`, and the report must say so.

## 10. Migration From Current Implementation

Current known implementation state:

- ordinary runs are now durable by default;
- engine checkpoints can persist an evaluation-cache snapshot;
- GEPA continuation is JSON optimizer state in checkpoints;
- `leaven-lm-cache` has in-memory and SQLite backends, with run-directory and
  workspace `.leaven/lm-cache.sqlite` open helpers;
- P8 wires live OpenAI solver/reflection roles to run-directory SQLite by
  default and exposes an explicit `eager-sqlite` workspace-cache override for
  release reruns, plus `cache-only` policy for no-spend replays that fail
  closed on cache misses;
- local run store currently uses file/blob/checkpoint/evidence directories.

Migration order:

1. Introduce SQLite LM response-cache backend and contract tests. Done.
2. Wire P8 live LM roles to run-directory SQLite by default and explicit
   workspace eager-cache mode. Done at the example bridge layer.
3. Introduce SQLite evaluation-cache backend or a run-store table that can
   materialize `EvaluationCache` semantics without weakening engine ownership.
4. Wire both stores from `leaven-run` when a durable run dir is opened.
5. Add `CacheMode::Auto` and report cache summaries.
6. Teach product scorers/evaluators to declare deterministic cache eligibility
   without requiring users to set low-level cache policies.
7. Keep JSON optimizer continuation until there is a concrete reason to move it.

## 11. Proof Requirements

Minimum proof set:

1. Durable no-knob run opens cache stores under the run dir.
2. Reopening the same run dir preserves LM cache entries.
3. Reopening the same run dir preserves or reconstructs engine eval-cache
   entries without replaying completed deterministic assessments.
4. Deterministic eval cache hit charges no new metric call.
5. LM response cache hit charges zero provider cost and preserves original usage.
6. Unsafe eval cache attempts bypass with precise reasons.
7. Unsafe LM cache attempts bypass with precise reasons.
8. `ephemeral()` does not create durable SQLite cache files and reports
   non-durable cache mode.
9. Corrupt SQLite store fails with a typed store/cache error and does not silently
   clear data.
10. P8 smoke uses the same cache/run-dir product path as the full run, or labels
    itself explicitly non-benchmark if it opts out.

Narrow verification commands for implementation slices:

```bash
cargo nextest run -p leaven-lm-cache
cargo nextest run -p leaven-engine --test context_services --test engine_loop
cargo nextest run -p leaven-run --test optimize_builder --test scoring_evaluator
cargo test -p leaven --test topology_contract
```

Run `just check` before claiming full product completion.
