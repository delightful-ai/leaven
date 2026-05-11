# Leaven Tracing Vision

> Status: stakeholder alignment spec.  
> Date: 2026-05-11.  
> Governing specs: `docs/specs/initial_library.md`,
> `docs/specs/first_two_subsystems.md`,
> `docs/specs/agentic_stage_runtime.md`, and
> `docs/specs/lm_runtime_and_response_cache.md`.  
> Purpose: define what Leaven means by tracing before committing to concrete
> APIs, storage formats, exporters, or backend integrations.

This document is a requirements and vision spec for tracing across Leaven. It
is intentionally more precise than a brainstorm and less binding than an API
contract. The goal is to align humans, agents, implementers, and downstream
users on the shape of the problem.

The central claim:

```text
Leaven needs two related trace surfaces:

1. durable optimizer truth for replay, audit, datasets, and research
2. runtime observability for operators watching live optimizer execution
```

Both matter. They must share IDs and vocabulary. They must not collapse into
one mechanism.

---

## 1. Problem

Leaven is an optimizer data set library, not just an application framework.
Its traces need to answer questions that ordinary service logs do not:

- why did this candidate exist?
- which earlier candidates, proposals, assessments, or external facts informed
  it?
- which evidence did the optimizer see before choosing the next action?
- what evidence was withheld by trust/read-scope policy?
- what evaluator, model, prompt/config, tool environment, cache state, and
  budget state produced each assessment?
- what did the optimizer spend per improvement?
- can this trial be replayed or at least audited after sampling, crashes, or
  backend migration?
- can a researcher turn the trace into a clean training/evaluation data set
  without scraping logs?

Ordinary Rust `tracing` spans are necessary for live operation, but they are
not sufficient for this job. Spans may be sampled, filtered, exported to
different backends, dropped when no subscriber is installed, or shaped by a
consumer application's subscriber stack. That is correct for operational
telemetry and unacceptable as the only source of optimizer truth.

The durable source of optimizer truth remains Leaven-owned typed records:
`RunGraph`, `RunEvent`, evidence references, budget records, cache records,
checkpoint metadata, and optimizer-owned state. Runtime `tracing` mirrors those
facts for operators.

---

## 2. Non-Negotiables

### 2.1 Library crates emit; binaries subscribe

Leaven library crates must never initialize a global subscriber, call
`tracing_subscriber::fmt::init()`, install an OpenTelemetry exporter, or choose
process-wide filters.

Library crates may emit spans and events. Consuming binaries own subscriber
composition:

```text
Registry
  + EnvFilter
  + fmt layer for local debugging
  + OpenTelemetry layer for OTLP export
  + independent per-layer filters
```

This keeps Leaven composable inside applications that already have observability
stacks.

### 2.2 Durable trace is not sampled

Operational spans may be sampled. Durable optimizer trace records must not be
silently sampled away.

If a user explicitly configures a lossy durable sink in the future, that sink
must advertise its loss policy in metadata. The default record of optimizer
truth is complete for the run.

### 2.3 `RunContext` remains the graph mutation authority

Tracing must not create a second graph-writing path. Durable records that
describe graph mutation originate at the existing mutation seams:

- seed insertion
- proposal batch recording
- proposal application
- evaluation request recording
- assessment/evidence recording
- population event recording
- budget charging
- error recording
- checkpointing
- run/iteration lifecycle

No trace exporter, observer, runtime telemetry layer, or agent adapter may
mutate `RunGraph` directly.

### 2.4 Payloads are referenced, not inlined

Large values do not belong in span fields or durable event payloads:

- prompts
- full LM responses
- rollout transcripts
- workspace snapshots
- command stdout/stderr beyond bounded summaries
- large evidence bodies
- generated artifacts

Trace records carry references, hashes, content IDs, byte counts, MIME/format
metadata, and redacted summaries. The bytes live in artifact stores, evidence
stores, checkpoint stores, workspace stores, or explicit blob sinks.

### 2.5 Causal and informational provenance stay separate

Tracing must preserve the existing distinction:

- causal lineage explains what a candidate was made from
- informational provenance explains what a proposal or decision read

`CausalInputs` and `InfoRef` are not interchangeable. A trace that flattens
both into "parent ids" destroys optimizer truth.

### 2.6 Trust boundaries are traceable

Train/validation/test boundaries, read scopes, and actor capabilities are
part of the trace contract.

The trace must make it possible to audit:

- what the optimizer could read
- what a proposer could read
- what an evaluator could read
- what a renderer/materializer wrote into a workspace
- whether forbidden evidence was requested
- whether a trust violation was prevented or recorded

Trust policy is not just a runtime guard. It is part of the reproducibility and
leakage story.

### 2.7 Secrets and private payloads are redacted by construction

Tracing must assume optimizer workloads can contain secrets, credentials,
private data, proprietary prompts, customer data, unpublished research data, or
PII.

Default spans and durable events must prefer IDs, hashes, typed summaries, and
external references. Raw payload capture, if ever supported, must be opt-in,
targeted, clearly labeled, and easy to disable by target/level or durable sink
policy.

---

## 3. Three Trace Surfaces

Leaven should treat tracing as three coordinated surfaces.

### 3.1 Durable semantic trace

This is the dataset-grade record.

It is typed, Leaven-owned, complete by default, and tied to graph/checkpoint
truth. It exists even when no `tracing` subscriber is installed.

It should answer:

```text
what happened in optimizer terms?
what did each decision depend on?
what did each decision cost?
what evidence was produced or consumed?
what state can be replayed, audited, or exported later?
```

Canonical inputs include:

- `RunGraph` records
- `RunEvent` records
- evidence references and evidence metadata
- cache records
- budget records
- checkpoint records
- optimizer private-state snapshots when provided
- artifact/workspace/blob references

### 3.2 Runtime observability trace

This is the operator-facing `tracing` surface.

It is structured, filterable, compatible with Rust ecosystem tooling, and
exportable to local logs or OpenTelemetry collectors. It is allowed to be
sampled or absent.

It should answer:

```text
where is the run spending time?
which async task is currently executing?
which provider call failed?
which parallel rollout is slow?
which workspace command is noisy?
what is the live cost/token/error profile?
```

Runtime spans should mirror durable IDs so operators can jump from telemetry
back to Leaven-owned truth.

### 3.3 Programmatic observer surface

Users will want typed callbacks:

```text
trial finished
candidate accepted
evaluation completed
new best found
budget threshold crossed
error occurred
checkpoint written
```

They should not need to parse logs or implement a `tracing::Layer` to integrate
with Weights & Biases, Weave, MLflow, a notebook UI, a dashboard, or a custom
experiment database.

Leaven already has callback/event direction in the engine. The long-term
observer surface should build on typed events and graph views, not create a
parallel logging protocol.

---

## 4. Required Durable Facts

The durable semantic trace must preserve at least the following fact families.

### 4.1 Run identity and configuration

Each run needs a stable envelope:

- `run_id`
- library/crate versions where available
- optimizer type and optimizer configuration identity
- problem type identity where available
- case set identity and version
- trust policy identity
- budget configuration
- cache policy defaults
- artifact/evidence/store backends
- start/end status
- stop reason

The exact type shape can evolve, but trace exports must not force users to
reverse-engineer run configuration from event order.

### 4.2 Logical ordering

Records need enough ordering to reconstruct execution:

- monotonic sequence number or equivalent logical clock
- wall-clock timestamp where useful
- parent span/event IDs for runtime correlation where available
- iteration ID when inside an iteration
- stage ID when inside a stage
- actor identity when trust policy matters

Async execution means wall-clock order alone is not enough.

### 4.3 Candidate lineage

For every candidate:

- candidate ID
- artifact identity
- originating proposal or seed index
- causal inputs
- proposal effect kind
- apply outcome
- parent/child graph links
- creation time/order
- associated metadata refs

This is what makes candidate histories analyzable instead of anecdotal.

### 4.4 Proposal provenance

For every proposal:

- proposal ID
- proposal batch ID
- proposer/stage ID
- request identity or request summary
- proposal effect
- causal inputs
- informational inputs
- cost charged
- generation status
- apply status if applied
- bounded metadata

The trace must support "why did the optimizer try this?" without inlining the
entire rendered prompt or workspace.

### 4.5 Evaluation and assessment provenance

For every evaluation request and assessment:

- evaluation request ID
- evaluator ID and evaluator fingerprint
- unresolved evaluation request
- resolved evaluation set identity
- case IDs or case-set partition identity as appropriate
- request shape: independent, pairwise, or listwise
- assessment granularity
- purpose: search, validation, test, audit, etc.
- assessment IDs
- target candidate IDs
- evidence refs
- evidence schema identity or problem evidence type identity
- cache status
- cost charged
- evaluator/runtime error records if any

Evaluation trace is the spine of optimizer data sets. It must preserve the
difference between "not evaluated", "evaluated and failed", "evaluated from
cache", and "evaluation hidden by trust policy".

### 4.6 Budget and cost

Cost must be numeric and structured:

- cost units, not opaque strings
- stage ID
- charged amount
- remaining budget snapshot
- overflow or exhaustion errors
- model/provider token counts when available
- currency or price estimates when available
- workspace/runtime cost when available

Leaven should make it straightforward to compute:

```text
cost per improvement
cost per accepted candidate
cost per case solved
cost per evaluator
cost lost to retries/errors/cache misses
```

### 4.7 Cache decisions

Cache trace must record:

- cache policy
- cache key identity or redacted fingerprint
- hit/miss/bypass status
- bypass reason
- reused assessment IDs when safe to expose
- evaluator fingerprint and resolved request identity

Cache records are optimizer truth because cache hits can change both cost and
timing while preserving assessment semantics.

### 4.8 Population and strategy state

Population events must be traceable without making the engine own optimizer
strategy:

- population ID
- candidate admitted/rejected/updated
- compared assessment IDs
- frontier/niche/partition affected
- ranking/preference decision summary
- fitted-model update summary where applicable
- reason for rejection or non-admission where available

Optimizer-owned private state can be checkpointed separately, but the trace
must still expose enough public strategy events to inspect optimizer behavior.

### 4.9 Agent, LM, tool, and workspace execution

Agentic/provider/runtime crates need operational trace facts that join back to
the durable semantic trace:

- stage ID
- candidate/proposal/evaluation IDs when applicable
- workspace ID/ref
- materialization manifest ref
- renderer output ref or hash
- LM provider/model ID
- prompt/config hash
- response ID/ref
- token counts
- tool call IDs
- command/process IDs
- exit status
- retry/backoff details
- transcript/log/blob refs

These facts should live in the crates that own the corresponding runtime
knowledge. Cold core must not learn provider, process, workspace, or cloud
details.

### 4.10 Replay keys

Each trial-like span or durable decision boundary should aim to carry a replay
key:

- seed or RNG state where applicable
- candidate ID
- artifact identity
- evaluator/proposer/agent runtime fingerprint
- model ID
- model parameters such as temperature/top-p where applicable
- prompt/rendered-config hash
- materialization manifest ref
- tool/workspace image or environment identity
- case/evaluation set identity
- code/version identity where available

Some workloads are not exactly replayable. The trace must distinguish:

```text
replayable
partially replayable
audit-only
unreplayable, with reason
```

---

## 5. Runtime `tracing` Requirements

### 5.1 Stable targets

Leaven should use stable target prefixes so users can filter sensibly:

```text
leaven::engine::run
leaven::engine::iteration
leaven::engine::proposal
leaven::engine::evaluation
leaven::engine::budget
leaven::engine::cache
leaven::engine::population
leaven::engine::checkpoint
leaven::lm
leaven::agent
leaven::workspace
leaven::store
leaven::trace
```

Exact names can change before the API contract lands, but the final schema
should treat target names and field names as semver-relevant public surface.

### 5.2 Natural span hierarchy

The engine-level runtime hierarchy should follow Leaven concepts:

```text
run
  iteration
    proposer
      proposal_batch
        proposal
        apply
          candidate
    evaluator
      evaluation_request
        assessment
    population_update
    checkpoint
```

Agentic/provider crates can extend inside stage spans:

```text
rollout
  materialize_workspace
  lm_call
  tool_call
  workspace_command
  parse_stage_output
```

The hierarchy should make parallel rollout visualization useful. Orphan spans
are a correctness bug in async instrumentation.

### 5.3 Fields over log lines

Events should use structured fields:

```rust
tracing::info!(
    candidate_id = %candidate_id,
    assessment_id = %assessment_id,
    score = score,
    cost_metric_calls = cost.metric_calls(),
    "assessment completed",
);
```

Avoid embedding machine-readable data only inside message strings. Messages are
for humans; fields are for filtering, indexing, and joining.

### 5.4 Explicit instrumentation

Default `#[instrument]` behavior captures arguments via `Debug`, which is
dangerous for optimizer workloads.

Preferred default:

```rust
#[tracing::instrument(skip_all, fields(run_id = %run_id, stage = %stage))]
```

Fields learned after span creation should be recorded onto the active span when
they are small, safe, and useful:

```rust
tracing::Span::current().record("score", score);
```

Raw candidate payloads, prompts, transcripts, and credentials must not be
captured by accidental `Debug`.

### 5.5 Async propagation

Spans do not automatically cross all task boundaries. Any spawned future that
belongs to a run, iteration, rollout, or provider call must be instrumented
with its parent span or an explicitly linked child span.

This matters especially for:

- parallel rollouts
- concurrent evaluator calls
- background checkpointing
- workspace cleanup
- provider retries
- streaming LM responses

When visualization loses the parent-child relation, the runtime trace has lost
the optimizer story.

### 5.6 OpenTelemetry is a consumer concern

Leaven should be compatible with `tracing-opentelemetry`, but should not force
OTLP, a collector, a backend, or a sampling strategy.

Recommended application deployments may use:

```text
OpenTelemetry collector
  tail-based sampling
  keep all error traces
  sample successful traces by workload policy
```

This belongs in examples and operator docs, not library initialization.

---

## 6. Metrics Requirements

Traces tell the narrative. Metrics power dashboards.

Leaven should not expect production users to derive every aggregate from trace
queries. That is expensive and backend-specific.

Useful aggregate metrics include:

- run duration
- iteration duration
- proposal batch size
- proposal success/failure count
- candidate acceptance rate
- evaluator latency p50/p95/p99
- evaluator error rate
- cache hit/miss/bypass counts
- budget consumed by stage
- token counts by provider/model/stage
- cost per accepted candidate
- cost per improvement
- score distribution/quantiles when evidence shape supports it
- workspace command duration/error counts
- checkpoint duration/size/count

Metrics emission should be optional and should not replace durable trace
records.

---

## 7. Crate Placement

Tracing must follow Leaven topology.

### 7.1 `leaven-core`

Cold core should not depend on runtime telemetry crates.

It may define semantic vocabulary that is intrinsically part of optimizer
truth, such as proposal/evaluation/provenance types. It should not know about
OpenTelemetry, subscribers, span hierarchy, workspace commands, LM providers,
or trace export formats.

### 7.2 `leaven-engine`

The engine owns durable run events and graph-backed truth.

Expected responsibilities:

- emit durable `RunEvent`s
- maintain event ordering
- attach run/iteration/stage/candidate/evaluation IDs
- invoke typed callbacks/observers
- emit runtime spans around engine-owned operations
- never let tracing bypass `RunContext`

### 7.3 `leaven-trace`

`leaven-trace` should be a projection and export crate, not a hidden mutation
path.

Expected responsibilities:

- project `RunGraph` plus event streams into trace views
- build lineage DAGs and decision trees
- produce run summaries
- produce dataset/export formats
- join evidence refs and artifact refs without inlining large payloads by
  default
- validate trace completeness and schema invariants

It may eventually own stable export schemas, but it should consume engine truth
rather than invent it.

### 7.4 `leaven-run`

`leaven-run` owns public product-builder ergonomics.

Expected responsibilities:

- expose easy tracing/observer configuration hooks for ordinary users
- document how applications install subscribers
- provide default typed observer wiring where appropriate
- avoid hiding hard choices behind global process state

### 7.5 `leaven-lm-*`, `leaven-agent-*`, and workspace crates

Provider/runtime/backend crates own their operational facts:

- model IDs
- prompt/config hashes
- provider response IDs
- token accounting
- tool calls
- workspace commands
- materialization refs
- retry/backoff details

These crates should join to engine truth through IDs, not by importing graph
internals.

### 7.6 `leaven-std` and `leaven`

Facade crates may curate tracing/export/observer pieces once they are real.
They should not become implementation buckets.

---

## 8. Schema Stability

Trace schema is a user-facing contract.

Once stabilized, the following changes are semver-relevant:

- renaming span targets
- renaming span fields
- changing field meaning
- removing durable event fields
- changing event ordering guarantees
- changing replay key semantics
- changing redaction defaults
- changing export schema interpretation

Before stabilization, schemas may change freely, but each implementation slice
should state whether it is scaffolding, experimental, or contractual.

Golden tests should protect:

- event ordering
- span field presence for stable runtime spans
- redaction defaults
- no accidental payload capture
- durable export schema shape
- replay key completeness for representative workloads

---

## 9. Privacy and Redaction Model

The default posture is conservative:

```text
IDs, hashes, typed summaries, counts, costs, refs: yes
raw prompts, transcripts, artifacts, credentials: no
```

Redaction should be explicit and testable. Useful mechanisms may include:

- typed `TraceSafe` summaries
- payload classification
- redacted display wrappers
- per-target runtime filters
- durable sink policies
- blob-store refs with access controls
- opt-in raw capture for local research runs

The important requirement is that safe defaults do not depend on every caller
remembering to avoid `Debug`.

---

## 10. Failure Semantics

Failures are first-class trace facts.

The trace must distinguish:

- stage returned an error
- budget charge failed before mutation
- trust policy rejected a request
- provider call failed
- provider call retried then succeeded
- workspace command failed
- parser failed to lower runtime output into a proposal/assessment
- checkpoint failed
- cache key could not be formed
- cache was bypassed by policy
- run stopped because an error was fatal
- run continued after an error by policy

Error records should preserve kind, stage/actor, policy, causal operation, and
safe source context. A single generic "error" event is not enough for optimizer
analysis.

---

## 11. Dataset and Research Use Cases

The trace should support generating data sets such as:

- candidate lineage data sets
- proposal-to-outcome data sets
- prompt/config hash to assessment outcome data sets
- per-case improvement trajectories
- pairwise/listwise judgment data sets
- cost-per-improvement tables
- cache-effectiveness reports
- strategy decision audits
- trust/leakage audits
- agent rollout summaries joined to candidate outcomes
- replay manifests for selected trials

This implies export tooling must preserve typed IDs and references. CSV can be
a convenience view. It cannot be the canonical format for rich trace data.

---

## 12. Open Questions

These are real design choices, not reasons to delay the vision.

1. **Observer shape.**  
   Should the typed observer surface be exactly the existing callback system
   evolved, a new trait, or both with one delegating to the other?

2. **Durable event envelope.**  
   Should `RunEvent` carry sequence/timestamp metadata directly, or should the
   graph/event store wrap events in a separate `RunEventRecord` envelope?

3. **Runtime span stability timing.**  
   Which target/field names should become contractual first, and which should
   remain experimental until agentic/provider work lands?

4. **Replay key completeness.**  
   What is the minimum replay key for non-agentic evaluators, LM-backed stages,
   and full agentic workspace rollouts?

5. **Metrics crate choice.**  
   Should Leaven emit through the `metrics` ecosystem, OpenTelemetry metrics,
   an internal observer projection, or a small adapter layer?

6. **Trace export format.**  
   Should canonical exports be JSONL, postcard/CBOR, Parquet, Arrow, a custom
   segment format, or multiple projections from one internal schema?

7. **Raw payload capture.**  
   How should local-only research captures opt into raw prompts/transcripts
   without weakening default safety?

8. **Sampling metadata.**  
   If a durable sink is intentionally lossy, how should that loss policy be
   represented so downstream analysis does not mistake a sample for complete
   truth?

---

## 13. Implementation Direction

The likely staged implementation path is:

1. Define durable trace/event envelope requirements around existing
   `RunEvent`s.
2. Add missing durable facts at existing `RunContext` seams.
3. Add contract tests for event order, IDs, and no-payload defaults.
4. Add runtime `tracing` spans in `leaven-engine` with explicit fields and
   `skip_all`.
5. Add async span propagation where the engine or runtime crates spawn work.
6. Turn `leaven-trace` scaffolding into graph/event projections.
7. Add export/report tests for representative P1 and GEPA-style runs.
8. Add provider/runtime/workspace operational spans in their owning crates.
9. Add public observer ergonomics in `leaven-run`.
10. Add operator examples showing subscriber setup, local fmt logging, OTLP
    export, and sampling outside library code.

This is deliberately a hard cutover path. We should not preserve a parallel
old logging surface once typed trace and structured spans exist.

---

## 14. Definition of Alignment

Stakeholders are aligned when they agree to these statements:

- Leaven traces are optimizer records, not just logs.
- Runtime `tracing` is required, but it is not the only source of truth.
- Durable trace data is complete by default and typed.
- Operational telemetry may be sampled and exported by the consuming
  application.
- Large/private payloads are referenced by default.
- Causal lineage and informational provenance stay distinct.
- Trust boundaries are auditable.
- Replay keys are a first-class goal.
- Programmatic observers are part of the user-facing experience.
- `leaven-trace` projects and exports truth owned by the engine/graph; it does
  not become a hidden mutation layer.

If any of those are rejected, the trace design needs another alignment pass
before implementation.
