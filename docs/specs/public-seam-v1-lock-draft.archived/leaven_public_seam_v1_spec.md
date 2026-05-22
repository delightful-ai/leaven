# Leaven Public Seam V1: Plan IR, Capability Spine, Evaluator Privilege Model

Status: **candidate normative draft**  
Protocol versions: `leaven.plan.v1`, `leaven.plan_result.v1`, `leaven.capability.v1`, `leaven.evaluation_job.v1`, `leaven.evidence_envelope.v1`, `leaven.watch.v1`  
Audience: Leaven engine implementers, SDK authors, artifact adapter authors, stage-worker authors, security reviewers, optimizer authors.

This spec captures the architectural decisions reached for Leaven's public stage-extension seam. It is intentionally opinionated. The goal is not just to define JSON shapes; it is to transfer the judgment behind the shapes so that future implementations make the same calls under pressure.

## 0. The compressed judgment

The whole design compresses into these positions:

1. **One public IR family, three semantic node classes.**  
   `leaven.plan.v1` is one value-shaped language. It has `Let` nodes for pure reads, `Call` nodes for metered external effects, and `Write` nodes for append-only graph mutation intents. The public wire is unified; the semantics are not flattened. Reads are deterministic at a resolved graph revision. Calls are effectful and replayable only by receipt. Writes are staged intents and commit only through `RunContext`.

2. **Capability token as the authorization spine.**  
   Every transport — CLI, Python worker, TS worker, workspace agent using `leaven query`, local Unix socket, remote HTTPS — enters the same authorization kernel. The bearer token is opaque and secret; the grant document is structured, fingerprinted, auditable, attenuable, and derived from engine policy. A grant is action/resource/constraints, Cedar-shaped in spirit; delegation is macaroon-shaped in spirit.

3. **Managed boundaries, arbitrary host-language interior.**  
   Reflectors, scorers, evaluators, proposers, and callbacks can be Python/TS/Rust/anything. Their local control flow is normal code. Privileged or costful boundaries cross Leaven portals: graph reads, case reads, workspace reads, sandbox exec, LM complete, agent run, human review, proposal submit, assessment submit. This avoids turning Leaven into a workflow DSL while preserving budget, cache, replay, audit, and policy.

4. **Evaluator privilege is real, but request-scoped and egress-scoped.**  
   Evaluators may see hidden targets, execute candidates, inspect workspaces, call grader LMs, and spawn judge agents. That privilege is scoped to one evaluation request and its resolved candidate/case set. Target access does not imply target egress. Data-class labels and input-class constraints determine where target-derived data may flow.

5. **Visibility is a result property, not just an ingress filter.**  
   Results carry redactions, source refs, read receipts, effect receipts, graph revisions, projection fingerprints, and evidence visibility. An evaluator's evidence is split into public and private channels. Scorer output is the ordinary bridge from hidden targets into optimizer-visible evidence; therefore that bridge must be labeled and auditable.

6. **`RunContext` remains the only graph mutation authority.**  
   IR writes are not database mutations. They are creation-side intents: submit proposals, submit assessments, request evaluation, apply a proposal batch, emit a run event. The engine validates capabilities, preconditions, graph revision, schemas, hidden-partition policy, and stage authority before committing through `RunContext`.

7. **Blessed LM and agent dispatch are first-class `Call` nodes.**  
   The default path for expensive calls is through Leaven. That gives cache, budget, replay, provider policy, telemetry, receipts, and provenance. BYO network/subprocess is an explicitly degraded trust profile controlled by both capability grants and OS/container execution policy.

8. **Query IR is richer than the current `StageQuery` enum.**  
   The public surface is an algebra over graph/case/workspace sources, traversal steps, predicates, projections, and extension ops. The old enum shape remains useful internally but is not the forever wire. Time-travel/diff queries are in v1. Subscriptions are a sibling watch protocol, not infinite query plans.

9. **Artifact semantics live in adapters.**  
   Core Leaven knows candidate refs, surface fingerprints, schema fingerprints, projections, changes, diffs, source refs, and receipts. DSPy modules, skill banks, Inspect solvers, theorem-prover states, git trees, multimodal artifacts, and browser traces are adapter namespaces, not core ontology.

10. **Determinism is precise.**  
   Same pure read IR + same graph revision + same projection/adaptor/policy fingerprints gives the same result. Fresh LM/agent/sandbox/human calls are not deterministic. Same effect receipt gives the same replayed result.

## 1. Scope

This spec defines:

- `leaven.plan.v1`: finite value-shaped IR evaluated by the engine.
- `leaven.plan_result.v1`: result envelope for evaluated plans.
- `leaven.capability.v1`: structured grant document behind an opaque bearer token.
- `leaven.evaluation_job.v1`: evaluator invocation payload.
- `leaven.evidence_envelope.v1`: visibility-labeled evaluator/scorer evidence.
- `leaven.watch.v1`: finite subscription/watch request shape.
- `leaven.worker_protocol.v1`: stage-worker JSON-RPC envelopes.

This spec does not define:

- Internal Rust type layout.
- SQL/SQLite persistence layout.
- Provider-specific LM or agent APIs.
- DSPy/Inspect/SkillBank artifact schemas.
- Optimizer internals. Python optimizer authoring is not v1.

## 2. Transport and service model

The canonical service method is:

```text
leaven.ir.eval(plan: LeavenPlanV1) -> LeavenPlanResultV1
```

Authorization is out-of-band:

```text
Authorization: Bearer $LEAVEN_TOKEN
```

or the equivalent stdio/Unix-socket credential. The bearer token itself is never stored in run artifacts. Run artifacts store:

- token id or grant id,
- capability fingerprint,
- policy fingerprint,
- issuer,
- subject,
- expiration,
- parent fingerprint,
- execution policy fingerprint.

The same `leaven.ir.eval` method is used by:

- CLI commands such as `leaven query lineage ...`,
- Python/TS/Rust SDKs,
- workspace agents using `leaven` like `git log`,
- local worker processes over stdio or Unix sockets,
- remote workers over HTTPS,
- future MCP adapters.

## 3. `leaven.plan.v1` semantics

A plan is a finite DAG of named ops. Dependencies are either explicit (`deps`) or implicit by variable references. The engine may run independent ops concurrently subject to capability and budget limits.

### 3.1 Node classes

#### `Let`

A `Let` binds a pure expression result.

Examples:

- graph query: candidate lineage, siblings, costs, recent failures,
- case query: case input/target/metadata, resolved set,
- workspace query: read file, git diff, digest, snapshot,
- projection/filter/sort/limit/template/extract over prior values.

A `Let` must not charge money. It may be quota-limited by rows, bytes, depth, wall time, or materialized count. A `Let` always returns a read receipt when it crosses graph/case/workspace state.

#### `Call`

A `Call` invokes a metered external capability.

Core call kinds:

- `lm_complete`,
- `agent_run`,
- `sandbox_exec`,
- `workspace_materialize`,
- `human_review`,
- `extension_call`.

A call may charge budget. It returns an effect receipt. It is replayable by receipt, not intrinsically deterministic.

#### `Write`

A `Write` submits an append-only graph mutation intent.

Core write kinds:

- `submit_proposal_batch`,
- `submit_assessments`,
- `request_evaluation`,
- `apply_proposal_batch`,
- `emit_run_event`,
- `extension_write`.

A write commits only through `RunContext`. The plan evaluator must validate capability grants, idempotency, preconditions, graph revision, schema fingerprints, trust policy, hidden partition policy, and role-specific authority before invoking `RunContext`.

### 3.2 Commit model

External effects are not rolled back. If a plan spends money on an LM call and a later proposal write fails, the LM charge remains real and receipted.

Graph writes may be atomic or sequential as requested by `commit` and allowed by capability. Default is:

```json
{ "kind": "graph_writes_atomic", "on_stale": "reject" }
```

### 3.3 Determinism and replay

Pure reads are deterministic when these are fixed:

- plan/op hash,
- resolved graph revision,
- dataset/case-set version,
- artifact adapter fingerprint,
- projection schema fingerprint,
- read policy fingerprint,
- capability fingerprint.

Calls are deterministic only under replay by effect receipt. Fresh calls are not guaranteed to reproduce.

Writes are idempotent by idempotency key and commit preconditions.

## 4. Query IR

The query language is an algebra, not a bag of RPC methods.

### 4.1 Graph sources

Core graph sources include:

- `candidate`, `candidates`,
- `proposal`, `proposal_batch`, `proposal_batches`,
- `assessment`, `assessments`,
- `population_events`,
- `recent_failures`,
- `costs`,
- `events`,
- `candidate_tree`,
- `extension_source`.

### 4.2 Graph traversal steps

Core edges include:

- parents,
- children,
- lineage,
- siblings,
- informed_by,
- informed,
- proposal_that_created,
- assessments,
- pairwise_assessments.

### 4.3 Case queries

Case queries are first-class and policy-sensitive. The engine must resolve syntactic case sets to concrete case IDs and partitions before authorizing. This includes `Cases(_)`, `Tagged(_)`, `Recent`, and `Unscoped`; none may bypass hidden-partition checks because the syntax does not mention validation/test explicitly.

Core case ops:

- `resolve_set`,
- `case.load`,
- `case.input`,
- `case.target`,
- `case.metadata`.

Default role policy:

- runner: case input, no target,
- reflector/proposer: target-safe examples, usually no direct target query,
- scorer/evaluator: request-scoped target access,
- operator: explicit.

### 4.4 Workspace queries and calls

Workspace read operations are pure relative to an immutable workspace snapshot:

- snapshot,
- read_file,
- list,
- stat,
- digest,
- git_log,
- git_diff,
- capture_artifacts.

Workspace materialization and command execution are calls:

- `workspace_materialize`,
- `sandbox_exec`,
- `agent_run`.

Workspaces may contain secrets. Workspace read policy includes path allow/deny rules, byte limits, data-class labels, and snapshot fingerprints.

### 4.5 Projections

Projection is part of authorization, not display formatting. Queries must state how much they materialize.

Core projections:

- ids,
- summary,
- candidate,
- artifact,
- assessment,
- evidence,
- diff,
- cost_summary,
- event_summary,
- extension_projection.

Artifact projections/diffs are adapter-owned. Core only enforces surface fingerprint, schema fingerprint, byte limits, and capability grants.

## 5. Capability model

A bearer token resolves to a `leaven.capability.v1` grant document. The grant document is fingerprinted and auditable.

### 5.1 Grant shape

Each grant is:

```text
action + resource selector + typed constraints
```

Examples:

- `graph.read`,
- `case.read`,
- `workspace.read`,
- `workspace.materialize`,
- `sandbox.exec`,
- `lm.complete`,
- `agent.run`,
- `human.review`,
- `proposal.submit`,
- `proposal.apply`,
- `assessment.submit`,
- `evaluation.request`,
- `watch.start`,
- `extension.read`, `extension.call`, `extension.write`.

### 5.2 Execution policy is separate from capability

Capabilities say what Leaven will do for a token. Execution policy says what the process can do outside Leaven.

A Python process with network access can call OpenAI directly regardless of Leaven grants. Therefore remote/multi-tenant execution must restrict OS/container capabilities as well as Leaven grants.

Named profiles:

- `trusted_local_operator`,
- `managed_sandbox`,
- `package_scorer`,
- `remote_untrusted`,
- `custom`.

### 5.3 Delegation

Delegation is attenuation-only. A child token may only reduce authority: fewer actions, narrower resources, tighter limits, shorter expiration, stricter data-class constraints, stricter execution policy. It may never expand the parent.

### 5.4 Authorization algorithm

For `leaven.ir.eval`:

1. Parse plan and schema versions.
2. Verify/resolve bearer token.
3. Normalize grants and execution policy.
4. Resolve `latest_at_start` to a concrete graph revision.
5. Compute static required capabilities.
6. Perform static authorization.
7. Evaluate pure reads, applying projection/redaction/quotas.
8. Perform dynamic authorization for resolved call/write values.
9. Reserve/charge budget for costful calls.
10. Stage graph writes.
11. Commit through `RunContext` if preconditions hold.
12. Emit result with receipts, redactions, charges, revisions, and errors.

## 6. Information-flow labels

Values crossing managed boundaries may carry data-class labels:

- `public`,
- `case.input`,
- `case.target`,
- `case.metadata`,
- `candidate.output`,
- `candidate.artifact`,
- `workspace.file`,
- `workspace.secret`,
- `scorer.private`,
- `evaluator.private`,
- `optimizer.visible`,
- `prompt.raw`,
- `completion.raw`,
- `transcript.raw`,
- `human.review`,
- `external.secret`,
- `x.<namespace>`.

Every LM/agent/sandbox/human call declares `input_classes`. Capability grants declare allowed and forbidden input classes.

**Target access is not target egress.** An evaluator may be allowed to read `case.target` but forbidden to send `case.target` to a remote LM or agent runtime.

## 7. Evaluator privilege model

Evaluators are intentionally privileged. They may run candidates, read hidden targets, inspect workspaces, invoke verifiers, call judge LMs, spawn judge agents, and submit assessments.

That privilege is scoped to an evaluation job:

- one run,
- one evaluation request,
- one resolved case set,
- specific candidates/pairs/groups,
- specific purpose and granularity,
- specific case IDs,
- specific output assessment shape.

Evaluator workers receive `leaven.evaluation_job.v1` plus a scoped capability token. The job gives handles and refs; target material is retrieved through receipted `case.read` calls.

### 7.1 Managed-boundary evaluator model

Evaluator code is arbitrary host-language code, but privileged boundaries use Leaven portals:

- `cx.case.load`,
- `cx.graph.query`,
- `cx.workspace.*`,
- `cx.sandbox.exec`,
- `cx.lm.complete`,
- `cx.agent.run`,
- `cx.human.review`,
- `cx.assessments.submit`.

DSPy, Inspect-style scorers, theorem provers, pytest, Codex/Claude agents, human review queues, and custom Python logic all fit this model.

### 7.2 DSPy as evaluator

DSPy programs remain Python. Leaven provides an LM adapter and optional callbacks:

```python
with leaven.dspy_context(cx, model_role="grader", allow_input_classes=[...]):
    result = judge(...)
```

The adapter maps underlying DSPy LM calls to `Call { kind: "lm_complete" }`. Module control flow, fallback, branching, ensembles, and parsing remain inside Python. Optional callbacks record module-level spans and trace refs.

### 7.3 Agent evaluators

Agent judge sessions route through `agent_run` by default. This records transcript refs, commands, output files, raw provider events if allowed, cost, runtime fingerprint, and trace refs. BYO agent spawning is allowed only in trusted-local profiles and degrades replayability.

### 7.4 Evidence envelope

Evaluator/scorer evidence must be visibility-labeled. The public channel is optimizer-visible by policy. The private channel is evaluator/operator-only.

Evidence records:

- whether it is target-derived,
- public feedback/metrics/summary,
- private payload or payload ref,
- trace refs,
- source receipts,
- redaction policy.

Reflectors receive only the policy-projected evidence channel.

## 8. Proposal writes

A proposal write contains:

- effect: create, change, or change from workspace diff,
- causal provenance,
- `informed_by` semantic refs,
- annotations,
- metadata,
- read receipts at the batch level.

`informed_by` and `read_receipts` are deliberately separate. `informed_by` says what graph objects the stage relied on. `read_receipts` says what exact projected/redacted values the stage saw.

## 9. Assessment writes

Assessment writes are request-scoped.

An evaluator/scorer token may submit assessments only for the evaluation request and resolved case/candidate set it was invoked for. The engine validates:

- shape: independent/pairwise/listwise,
- granularity: per-case/aggregate/mixed,
- candidates,
- cases,
- purpose,
- target visibility,
- evidence visibility,
- receipt/cost attribution.

## 10. Cost, budget, and replayability

Leaven budgets real money and provider-metered effects. Graph traversal and internal reads are not budgeted in dollars, but they are quota-limited.

Managed effects return receipts and costs. Assessment/proposal writes cite effect receipts for cost attribution.

Replayability classes:

- `pure_read`,
- `fully_managed`,
- `boundary_managed`,
- `has_declared_external_effects`,
- `has_untracked_external_effects`.

Trusted-local BYO effects may be declared, but they are not equivalent to Leaven-managed calls.

## 11. Watch protocol

Subscriptions are not infinite plans. `leaven.watch.v1` is a sibling protocol using the same capability spine.

A watch request specifies:

- run,
- source: run events or graph changes,
- starting revision or cursor,
- filter,
- projection,
- backlog/heartbeat limits.

Cross-run watches are denied by default unless explicitly granted.

## 12. Extensions

Core stays closed and typed. Extensions are schema-registered.

An extension op declares:

- namespace,
- op,
- input schema fingerprint,
- output schema fingerprint,
- required capability action,
- source-ref extraction behavior,
- redaction behavior,
- data-class behavior.

Examples:

- `leaven.dspy.trace.select`,
- `leaven.skillbank.skill.project`,
- `leaven.inspect.solver.trace`,
- `leaven.theorem.proof_state`,
- `leaven.browser.har_summary`.

No untyped `signals: dict[str, Any]` may become a cross-stage dependency.

## 13. Schema files

The JSON Schema files in `schemas/` are normative for this draft:

- `common.schema.json`
- `leaven.plan.v1.schema.json`
- `leaven.plan_result.v1.schema.json`
- `leaven.capability.v1.schema.json`
- `leaven.evaluation_job.v1.schema.json`
- `leaven.evidence_envelope.v1.schema.json`
- `leaven.watch.v1.schema.json`
- `leaven.worker_protocol.v1.schema.json`

The schemas are intentionally strict for core fields. Unknown core fields are rejected unless placed in `metadata` or an `extension` payload.

## 14. Implementation notes

### 14.1 Engine

The engine should expose:

- graph revision stamps,
- idempotent stage-call IDs,
- owned snapshots across async waits,
- budget reservation and charge reconciliation,
- commit-time validation,
- checkpointable queue state for async optimizers,
- population staleness compatibility validation.

Async optimizer scheduling remains in optimizer/orchestrator crates, not engine.

### 14.2 Stage workers

Foreign-language stage workers use JSON-RPC envelopes from `leaven.worker_protocol.v1`. The worker receives role-specific payloads plus a scoped token. The token grants use of `leaven.ir.eval`; the worker does not receive raw `RunContext`.

### 14.3 SDKs

SDKs should make Leaven portals the path of least resistance:

- `cx.graph...` compiles to `Let` nodes,
- `cx.lm.complete(...)` compiles to `Call lm_complete`,
- `cx.agent.run(...)` compiles to `Call agent_run`,
- `cx.assessments.submit(...)` compiles to `Write submit_assessments`.

SDKs may batch independent operations into one plan DAG. Host-language `asyncio.gather` / `Promise.all` is also valid and sends multiple smaller plans under the same stage/evaluation attempt id.

### 14.4 Security invariants

- Never persist bearer token secrets.
- Do not let syntax-only case sets bypass partition checks.
- Do not make target access imply target egress.
- Do not send hidden targets to external models unless explicitly granted.
- Do not expose raw prompt/completion/transcript by default.
- Redaction reasons may be coarse in high-security modes to avoid leaking existence/counts.
- Counts can leak. Capabilities must control exact/bucketed/no counts.
- BYO effects degrade replayability and audit quality.
- Workspace agents should default to read-only, target-safe, short-lived, no-delegation tokens.

## 15. Minimal evaluator example

See `examples/evaluator_dspy_codex.py` for a full evaluator sketch that:

- reads case targets through `case.read`,
- materializes a candidate workspace,
- reads git diff/log,
- runs pytest through `sandbox_exec`,
- spawns a Codex app-server judge through `agent_run`,
- routes DSPy LM calls through `lm_complete`,
- submits request-scoped assessments with evidence envelopes.

## 16. Open refinement points

These are intentionally not settled by this draft:

- exact Rust type names and crate placement,
- exact StageWorkerRuntime trait signature,
- exact provider-specific LM/agent runtime adapters,
- exact policy backing store for remote multi-tenant deployments,
- exact DSPy callback span format,
- exact artifact adapter schemas for DSPy, SkillBank, Inspect, theorem provers, browser tasks, multimodal tasks,
- exact OpenTelemetry span mapping.

The architectural shape above should remain stable while those details refine.
