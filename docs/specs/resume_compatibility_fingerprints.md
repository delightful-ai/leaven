# Resume Compatibility Fingerprints

Status: implementation spec.

This spec defines the compatibility contract that makes durable Leaven resume
safe. It complements `durable_runs_and_resume.md`, `default_cache_storage.md`,
and `case_visibility_and_target_isolation.md`.

Durable resume must never continue a stored optimizer run under a different
effective dataset, runner, scorer, evaluator, provider configuration, or
optimizer strategy by accident. If Leaven cannot prove compatibility from stored
state and live capabilities, resume fails before the next runner, scorer,
provider, evaluator, or optimizer step.

## 1. Product Rule

The user-facing product remains one-path:

```rust
let result = optimize(seed)
    .train(train_cases)
    .validate(validation_cases)
    .run_dir(".leaven/runs/aime-p8")
    .runner(aime_solver)
    .scorer(exact_match_scorer)
    .using(Gepa::default())
    .run()
    .await?;
```

The run directory is the resume handle. Leaven derives and stores the
compatibility manifest as part of the durable run. On a later `.run()` or
explicit resume using the same run directory, Leaven computes the live manifest
again and compares it to the stored manifest before doing costful work.

There must not be three product knobs for "runner cache", "eval cache", and
"resume compatibility". Ordinary durable runs get compatibility checking by
default. Advanced APIs may expose explicit fingerprints for user-authored
closures or external systems, but they are compatibility declarations, not
separate cache toggles.

## 2. Manifest

Each durable run stores a `RunCompatibilityManifest` equivalent to:

```rust
pub struct RunCompatibilityManifest {
    pub schema: StateFormatId,
    pub run_kind: RunKind,
    pub problem: ProblemFingerprint,
    pub dataset: DatasetCompatibility,
    pub runner: RuntimeFingerprint,
    pub scorer: RuntimeFingerprint,
    pub evaluator: RuntimeFingerprint,
    pub optimizer: OptimizerCompatibility,
    pub lm_roles: BTreeMap<RoleId, RuntimeFingerprint>,
    pub cache: CacheCompatibility,
    pub budget: BudgetCompatibility,
}
```

Exact names can change. The stored content must distinguish these domains so
errors can say what changed. A single opaque "run hash" is allowed only as a
derived summary; it is not sufficient as the only stored truth.

The manifest may live in `run.sqlite`, a checkpoint envelope, or a JSON sidecar
during the first implementation slice. The durable product target is the
Leaven-managed run store described in `default_cache_storage.md`; the
compatibility contract is the same regardless of physical encoding.

## 3. Compatibility Domains

### 3.1 Problem

The problem fingerprint covers the Leaven problem shape that affects graph
interpretation:

- artifact type/schema and change type/schema;
- proposal annotation schema;
- surface/edit vocabulary when an optimizer depends on it;
- seed artifact identity when the seed is part of the run contract.

It must not include run directory, process id, wall-clock time, or transient
memory addresses.

### 3.2 Dataset And Splits

Dataset compatibility is stronger than current `Dataset::fingerprint()`
membership proof. It covers:

- case ids and order within each split;
- split role mapping for train, validation, and test;
- case input identity;
- target identity for cases with scorer-visible targets;
- scorer-visible metadata projection identity;
- case-set schema/version.

Report-only provenance can be stored separately. It must not affect cache or
resume compatibility unless a runner, scorer, evaluator, or report policy reads
it as behavior. `source_id` for AIME should normally be stable report provenance,
but if it is used to select prompts, filter cases, group metrics, or score, it
becomes scorer/evaluator-visible metadata and must enter the compatibility
fingerprint.

The default `CaseSetVersion("0")` scaffold is not a product-safe durable
identity. Product runs must derive a stable case-set version from the case
envelope content or from a user-supplied immutable dataset version plus split
membership.

### 3.3 Runner

The runner fingerprint covers behavior that can change candidate outputs:

- runner implementation identity;
- prompt template and parser identity;
- provider family, model id, sampling config, tool configuration, and sandbox
  or workspace policy used by the runner;
- role-specific LM cache namespace when the runner calls a language model;
- any behavior-affecting feature flags.

The runner fingerprint must not include secrets, bearer tokens, API keys,
per-process retry jitter, run directory paths, or local cache file paths.

For known Leaven runner adapters, the adapter derives this fingerprint from its
typed configuration. For arbitrary closures, Leaven cannot infer behavior. A
durable closure runner must therefore be paired with an explicit runtime
fingerprint or an explicit durable adapter. If no stable fingerprint is
available, durable `.run()` refuses with `RuntimeFingerprintMissing` before
doing costful work. Tests and throwaway examples can use `.ephemeral()`.

### 3.4 Scorer

The scorer fingerprint covers behavior that can change case scores or feedback:

- scoring implementation identity;
- rubric, answer parser, normalizer, tolerance, and aggregation policy;
- target-use policy;
- scorer-visible metadata projection;
- provider/model configuration when the scorer calls an LM or judge.

For AIME exact match, this includes the answer extraction/parser version,
normalization rules, target answer format, and whether solution text is used for
feedback.

### 3.5 Evaluator

The evaluator fingerprint is the composed identity used by
`leaven-engine::EvaluationCacheKey`. For `leaven-run::ScoringEvaluator`, it must
include at least:

- runner fingerprint;
- scorer fingerprint;
- case content/split compatibility fingerprint;
- evaluation granularity and aggregation behavior;
- parallelism only if it changes behavior or deterministic ordering;
- cache safety policy.

The current label-plus-case-count scaffold is insufficient for durable resume or
cache validity. Product evaluator fingerprints must change when any effective
input, target, scorer-visible metadata, runner behavior, scorer behavior, or
cache policy changes.

### 3.6 Optimizer

Optimizer compatibility includes:

- optimizer implementation/configuration fingerprint;
- private state policy;
- optimizer checkpoint state schema and format;
- strategy roles such as proposer, reflector, selector, batch sampler, gate, and
  validation policy when they affect future decisions.

For the static GEPA slot API, the current implementation includes strategy slot
type names plus checkpointed selector, part-selector, gate, batch-sampler, and
validation-policy state in the optimizer compatibility fingerprint, and restores
that state from the optimizer checkpoint. Custom strategy values whose behavior
can change without appearing in checkpoint state still need an explicit
value-level compatibility declaration before they are safe for durable resume.

`CheckpointableOptimizer::optimizer_fingerprint()` and optimizer state schema
checking are the existing engine primitive. Optimizer crates own the optimizer
state payload and restore validation; `leaven-run` must not inspect GEPA private
strategy state.

### 3.7 LM Roles

LM calls used by runner, scorer, reflector, proposer, or other stages are
fingerprinted by role, not by a single global provider:

- role id, for example `runner`, `scorer`, `gepa_reflector`;
- provider family and model id;
- request-shaping defaults, including temperature, seed, max tokens, response
  format, tool choice, and system/developer prompt identity;
- retry semantics only when they can change observable outputs.

The SQLite LM response cache may reuse entries only when the provider-neutral
request key and role fingerprint are compatible.

### 3.8 Cache

Cache compatibility records storage and semantic mode, but changing physical
paths is not a resume mismatch by itself.

Required distinctions:

- `Auto`, `Off`, and explicit deterministic cache modes;
- engine evaluation-cache schema;
- LM response-cache schema;
- cache namespaces that affect key interpretation.

`CacheMode::Auto` remains the default product behavior. It provisions durable
stores but only caches evaluator results when the evaluator/scorer declares the
work deterministic and all key identities are present.

### 3.9 Budget

Budget compatibility covers the relationship between the stored ledger and the
new run limits:

- spent cost is restored from the stored ledger;
- a lower new budget than already-spent cost refuses before work;
- a higher or equal compatible budget may continue;
- provider-specific units must be comparable before resume.

Budget policy mismatches are typed resume refusals, not optimizer errors.

## 4. Comparison Policy

Resume comparison happens before any new candidate proposal, runner call, scorer
call, LM call, cache write, or optimizer step.

The algorithm is:

1. Open the run store and read the latest clean checkpoint plus compatibility
   manifest.
2. Build the live compatibility manifest from the supplied builder inputs and
   live capabilities.
3. Compare each compatibility domain independently.
4. Restore graph, budget, cache index, and optimizer private state only after
   incompatible live capabilities have been rejected. If graph restore is needed
   to compute an optimizer validation context, no new work may run before the
   manifest comparison completes.
5. Return a typed refusal naming the first incompatible domain, or all domains
   if the API supports aggregate diagnostics.

Exact equality is the default. Equivalence classes are allowed only where the
owning domain can prove they preserve behavior. For example, an increased
overall budget can be compatible with a restored budget ledger, but a changed
scorer parser is not compatible even if the scorer label stayed the same.

## 5. Errors

Typed classes must remain distinct:

- `RuntimeFingerprintMissing`: a durable run uses a live capability whose
  behavior cannot be fingerprinted.
- `DatasetFingerprintMismatch`: case ids, split roles, input identity, target
  identity, or scorer-visible metadata changed.
- `RunnerFingerprintMismatch`: runner behavior changed.
- `ScorerFingerprintMismatch`: scorer behavior changed.
- `EvaluatorFingerprintMismatch`: composed evaluator/cache identity changed.
- `OptimizerFingerprintMismatch`: optimizer configuration changed.
- `OptimizerStateSchemaMismatch`: optimizer continuation payload schema changed.
- `LmRoleFingerprintMismatch`: role-specific LM behavior changed.
- `CacheCompatibilityMismatch`: cache key/schema semantics changed.
- `BudgetPolicyMismatch`: restored ledger cannot be continued under the new
  budget policy.

Exact enum names may differ, but these cases must not collapse into string-only
errors.

## 6. AIME/P8 Requirements

P8 AIME via Leaven is not real until its durable run records and validates:

- immutable AIME case ids;
- problem input identity;
- target answer identity;
- solution/feedback source identity when scorer-visible;
- train/validation/test split identity;
- exact-match answer parser fingerprint;
- solver runner fingerprint, including model/prompt/parser/config;
- GEPA proposer/reflector/selector/gate/batch/validation fingerprints;
- role-specific LM fingerprints for solver and reflection calls;
- stored budget ledger and metric-call cap;
- cache mode and cache schema summary;
- report-only provenance such as source id and source corpus version.

If any of those cannot be produced, P8 may run as an explicit mechanics smoke,
but it must not be reported as a resumable product proof.

## 7. Implementation Routing

- `leaven-kernel` owns primitive fingerprint and state-format identifiers.
- `leaven-eval` owns case envelope and dataset compatibility vocabulary, but not
  runner or scorer behavior.
- `leaven-engine` owns checkpoint persistence, evaluation cache keys, graph
  restore, budget restore, and optimizer state restore primitives.
- `leaven-run` owns product lowering: building manifests from cases, splits,
  runner, scorer, evaluator, cache mode, store, and optimizer; refusing ordinary
  durable runs with missing fingerprints; and surfacing readable result/report
  compatibility summaries.
- Optimizer crates own optimizer-specific fingerprints and private-state
  schemas.
- LM/provider crates own provider-neutral request and role fingerprints.

Do not move GEPA private state into `leaven-run`, do not move product builder
ergonomics into `leaven-engine`, and do not treat low-level cache keys as a
complete resume manifest.

## 8. Proof Requirements

Required tests:

- same run directory, same case envelopes, same runner/scorer/evaluator, same
  optimizer config resumes without repeating committed evaluations;
- changed runner fingerprint refuses before a runner call;
- changed scorer fingerprint refuses before a scorer call;
- changed target answer with the same `CaseId` refuses;
- changed input with the same `CaseId` refuses;
- changed split membership refuses;
- report-only metadata change does not refuse unless projected into scorer or
  evaluator compatibility;
- missing closure/runtime fingerprint refuses in durable mode and succeeds only
  under explicit ephemeral mode or explicit fingerprint;
- optimizer fingerprint mismatch still routes through optimizer compatibility,
  not dataset/runtime compatibility;
- budget lower than already-spent cost refuses before work;
- secrets are excluded from serialized manifests and debug reports.

Focused implementation gates:

- `cargo nextest run -p leaven-run --test optimize_builder --test scoring_evaluator`
- `cargo test -p leaven-engine --test engine_contract engine_loop`
- `cargo nextest run -p leaven-gepa --test gepa_smoke`
- `cargo test -p leaven --test topology_contract`

Completion gate remains `just check`.
