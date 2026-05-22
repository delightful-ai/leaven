# Leaven Public Seam V1 Lock Draft

Status: **candidate normative lock draft**.

Protocol set: `leaven.plan.v1`, `leaven.plan_result.v1`, `leaven.capability.v1`, `leaven.stage_payloads.v1`, `leaven.evaluation_job.v1`, `leaven.evidence_envelope.v1`, `leaven.watch.v1`, `leaven.worker_protocol.v1`.

Audience: engine implementers, SDK authors, stage-worker authors, artifact-adapter authors, evaluator authors, security reviewers, optimizer authors.

Purpose: define the public value language, authorization spine, evaluator privilege model, and schema package that make Leaven usable from Python/TS/CLI/agents without giving up graph truth, target safety, budget accounting, replay, or audit.

Non-goal: define Leaven’s internal Rust layout, optimizer algorithms, provider-specific APIs, persistence tables, or artifact-specific schemas.

Terminology: **MUST** is a contract; **SHOULD** is a default that may be overridden with an explicit reason; **MAY** is allowed but not assumed.

---

## 0. The judgment this spec carries

### 0.1 One public IR family, three semantic node classes

`leaven.plan.v1` is one finite value language.

A `Let` node is pure relative to the resolved graph/dataset/workspace snapshot.

A `Call` node invokes a metered external effect and is replayable by receipt, not by wishful determinism.

A `Write` node submits a graph mutation intent; it does not mutate storage until `RunContext` commits it.

The public wire is unified because users compose reads, calls, and writes in one thought.

The semantics remain split because reads, effects, and graph writes have different failure, cost, replay, and atomicity laws.

### 0.2 Capability token is the authorization spine

Every transport enters the same authorization kernel.

CLI, Python, TS, workspace agents, local Unix sockets, remote HTTPS, and future MCP adapters all carry the same opaque bearer token class.

The bearer token is secret and ephemeral.

The grant document is structured, fingerprinted, auditable, attenuable, and never equivalent to the bearer secret.

A grant is `action + resource + constraints`.

Delegation is attenuation-only.

### 0.3 Managed boundaries, arbitrary host-language interior

Leaven is not a workflow DSL.

Python evaluators remain Python.

DSPy modules remain DSPy modules.

Inspect-style scorers remain async scorer functions.

Shell glue remains shell glue only in trusted profiles.

Costful or privileged boundaries cross Leaven portals: graph read, case read, workspace read, workspace materialize, sandbox exec, LM complete, agent run, human review, proposal submit, assessment submit.

Local branching, aggregation, parsing, retries, fallbacks, weighted scoring, and helper functions do not need IR nodes.

### 0.4 Evaluator privilege is request-scoped and egress-scoped

Evaluators are privileged by design.

Evaluators may read hidden targets when their evaluation job grants it.

Evaluators may execute candidates when their execution profile grants it.

Evaluators may send target-derived material only to destinations whose grants allow those data classes.

Target access does not imply target egress.

Assessment write authority is scoped to the evaluation request, resolved case set, candidate set, shape, purpose, and granularity.

### 0.5 Visibility lives in results

Visibility is not only an ingress check.

Every managed read returns a receipt describing revision, projection, redaction, source refs, materialized bytes, and visibility.

Every managed effect returns a receipt describing runtime/model/tool policy, cost, transcript/prompt visibility, and replay class.

Evaluator evidence is split into public and private channels.

Scorer output is the ordinary bridge from hidden target data to optimizer-visible evidence; this bridge MUST be labeled.

### 0.6 `RunContext` remains the only graph mutation authority

The public IR never inserts graph records directly.

Proposal submission, assessment submission, evaluation requests, proposal application, population events, and run events all commit through `RunContext`.

The engine MUST validate capabilities, preconditions, graph revision, schema fingerprints, hidden-partition policy, idempotency, and stage authority before graph mutation.

### 0.7 Blessed LM and agent dispatch are first-class calls

The default way to spend money is through Leaven.

`lm_complete` and `agent_run` are core `Call` variants.

Managed calls get cache, budget, replay, telemetry, provider policy, receipts, and provenance.

BYO network/subprocess is an explicit trust-profile escape hatch and degrades replayability.

### 0.8 Query IR supersedes the old `StageQuery` forever wire

The old closed enum was the right scaffold and the wrong public ceiling.

The public query language is an algebra over sources, traversal steps, predicates, projections, limits, and extensions.

Time-travel and diff queries are v1.

Subscriptions are not infinite plans; they are `leaven.watch.v1`.

### 0.9 Artifact semantics live in adapters

Core Leaven understands refs, revisions, receipts, surface fingerprints, schema fingerprints, projections, changes, diffs, and source refs.

Core Leaven does not understand DSPy modules, skill banks, Inspect solvers, theorem prover states, browser traces, git trees, or multimodal artifact internals.

Adapter namespaces own artifact-specific projection, diff, change, materialization, and parse semantics.

### 0.10 Determinism is exact

Pure reads are deterministic at fixed graph revision, case-set version, projection fingerprint, adapter fingerprint, policy fingerprint, and capability fingerprint.

Fresh LM, agent, sandbox, and human calls are not deterministic.

Replay of managed effects is by effect receipt.

Writes are replay-safe only through idempotency keys and commit receipts.

---

## 1. Protocol inventory

### 1.1 `leaven.plan.v1`

`leaven.plan.v1` is the finite IR evaluated by the engine.

A plan is a DAG of named operations.

A plan is not a conversation, process, or subscription.

A plan MAY contain independent operations that the engine schedules concurrently.

A plan MUST resolve `latest_at_start` to one concrete base graph revision before evaluating pure graph reads.

### 1.2 `leaven.plan_result.v1`

`leaven.plan_result.v1` is the only normal result envelope for plan evaluation.

It contains base revision, final revision, values, receipts, redactions, charges, errors, capability fingerprint, and policy fingerprint.

It MUST NOT contain bearer token secrets.

### 1.3 `leaven.capability.v1`

`leaven.capability.v1` is the structured grant document behind an opaque token.

It names subject, issuer, audience, validity, grants, delegation limits, execution profile, policy fingerprint, and parent fingerprint.

It is persisted by fingerprint and safe summary, not by bearer secret.

### 1.4 `leaven.stage_payloads.v1`

`leaven.stage_payloads.v1` defines public worker invocation payloads.

It includes target-safe `ReflectRequest`, target-aware `ScoreContext`, target-free `RunCase`, evaluator job refs, preference contexts, callback events, artifact adapter requests, and dataset adapter requests.

It is invocation shape, not graph storage shape.

### 1.5 `leaven.evaluation_job.v1`

`leaven.evaluation_job.v1` defines the evaluator’s request-scoped authority context.

The job gives refs and handles.

The job SHOULD NOT inline hidden targets unless the policy explicitly allows non-receipted target materialization.

### 1.6 `leaven.evidence_envelope.v1`

`leaven.evidence_envelope.v1` splits evaluator/scorer evidence into public and private channels.

It records target-derived status, trace refs, source receipts, and redaction policy.

### 1.7 `leaven.watch.v1`

`leaven.watch.v1` is a finite subscription request shape.

It uses the same capability spine.

It is not embedded inside `leaven.plan.v1`.

### 1.8 `leaven.worker_protocol.v1`

`leaven.worker_protocol.v1` defines JSON-RPC envelopes for stage workers.

The worker gets role-specific payloads and an out-of-band token.

The worker never receives raw `RunContext`.

---

## 2. Transport model

The canonical method is `leaven.ir.eval`.

`leaven.ir.eval` accepts one `LeavenPlanV1`.

`leaven.ir.eval` returns one `LeavenPlanResultV1`.

Authorization is out-of-band: `Authorization: Bearer $LEAVEN_TOKEN`, stdio credential envelope, Unix socket credential, or mTLS-bound token.

The token MUST be scoped to the caller, role, run, stage call, time window, and authority set.

The token MUST NOT be embedded in plan JSON.

The same service method supports CLI, SDKs, workspace agents, local workers, remote workers, and adapters.

CLI commands are syntax sugar over plans.

SDK calls are builders for plans.

Workspace agents using `leaven query` are untrusted clients unless the token grants more.

Remote deployment does not add a second policy model.

---

## 3. Schema conventions

Schemas use JSON Schema 2020-12.

Every public object has `schema_version` when it is a top-level protocol object.

Every tagged union uses a string `kind` discriminator.

Core objects reject unknown fields unless the schema explicitly allows `metadata` or an `extension` payload.

Extension payloads MUST carry namespace, op, input schema fingerprint, and output schema fingerprint when they cross the core boundary.

Every schema fingerprint MUST be computed over canonical schema bytes chosen by the implementation.

Every artifact adapter MUST publish projection, diff, change, materialization, and parse schemas before they appear on public wire.

IDs are opaque.

Clients MUST NOT infer ordering, shard, role, or timestamp from opaque IDs.

Graph revisions are ordered within a run.

Graph revisions are not globally comparable across runs.

---

## 4. Plan evaluation semantics

### 4.1 Plan identity

`plan_id` is an idempotency key for the caller’s intent.

The engine MAY reject duplicate `plan_id` with inconsistent content.

The engine SHOULD return the prior result for duplicate `plan_id` with identical content and compatible replay policy.

### 4.2 Consistency

`latest_at_start` resolves once.

`at_revision` pins all pure graph reads to the specified revision.

`since_revision` is for finite diff/event queries.

A plan result MUST report `base_revision`.

A plan result MUST report `final_revision` even when no graph write occurs.

### 4.3 Dependency DAG

Each operation has a name.

Variable references create implicit dependencies.

`deps` may add explicit dependencies when the operation does not reference the prior value directly.

Independent operations MAY run concurrently.

Concurrency MUST respect token constraints, budget reservations, runtime limits, and execution policy.

### 4.4 `Let` semantics

A `Let` binds an expression result.

A `Let` MUST NOT invoke external providers.

A `Let` MUST NOT mutate graph state.

A graph/case/workspace read `Let` MUST produce a read receipt unless the result is fully literal/local.

A `Let` MAY be denied, redacted, truncated, bucketed, or paginated by policy.

A `Let` MUST carry graph revision or snapshot identity where applicable.

### 4.5 `Call` semantics

A `Call` invokes an external or costful capability.

Core calls are `lm_complete`, `agent_run`, `sandbox_exec`, `workspace_materialize`, `human_review`, and `extension_call`.

A `Call` MUST perform dynamic authorization after input values and data-class labels are resolved.

A `Call` SHOULD reserve budget before dispatch.

A `Call` MUST charge actual cost after dispatch when cost is known.

A `Call` MUST return an effect receipt.

A `Call` MAY be served from cache when capability and call policy allow it.

A fresh call is not deterministic.

A replayed call is identified by effect receipt.

### 4.6 `Write` semantics

A `Write` submits an append-only graph mutation intent.

Core writes are `submit_proposal_batch`, `submit_assessments`, `request_evaluation`, `apply_proposal_batch`, `emit_run_event`, and `extension_write`.

A write MUST be staged before commit.

A write MUST perform static and dynamic authorization.

A write MUST validate schema fingerprints before commit.

A write MUST validate hidden partition policy before commit.

A write MUST validate role authority before commit.

A write MUST validate idempotency before commit.

A write MUST commit through `RunContext`.

### 4.7 Commit model

External effects are not rolled back.

Graph writes MAY be atomic across write nodes when `commit.kind = graph_writes_atomic`.

Default stale behavior is `reject`.

Async optimizer crates MAY choose guarded or repair semantics, but engine primitives MUST expose base revision, final revision, and precondition failures.

### 4.8 Failure model

A plan may produce partial values and errors.

A failed `Call` that spent money MUST still produce a charge receipt when the provider reports cost.

A failed `Write` MUST NOT silently mutate graph state.

A redacted result is not a failed result.

A denied result is an authorization error.

A truncated result is a quota result and MUST declare truncation.

---

## 5. Query IR

### 5.1 Query is a value

A query is a serializable value, not an RPC method name.

The SDK MAY build queries with chained host-language calls.

The engine evaluates the query value.

The same query value can travel over CLI, JSON-RPC, local embedding, or remote HTTP.

### 5.2 Graph sources

Core graph sources are candidate, candidates, proposal, proposal batch, proposal batches, assessment, assessments, population events, recent failures, costs, events, candidate tree, and extension source.

A candidate source addresses one candidate ref.

A candidates source searches candidates under policy and projection limits.

An events source supports finite time-travel/diff queries between revisions.

A costs source is a graph read with cost visibility policy; reading cost does not itself charge budget.

### 5.3 Graph edges

Core graph traversal edges are parents, children, lineage, siblings, informed_by, informed, proposal_that_created, assessments, and pairwise_assessments.

`lineage.depth` MUST be bounded by policy.

`siblings` MUST respect population/read scope.

`informed_by` and `informed` expose informational provenance, not causal ancestry.

### 5.4 Case queries

Case queries are first-class.

A case query MUST resolve syntactic sets to concrete cases before authorization.

`Cases(_)`, `Tagged(_)`, `Recent`, and `Unscoped` MUST NOT bypass hidden-partition checks.

Runner tokens usually grant `case.input` and deny `case.target`.

Reflector tokens usually get build-once target-safe examples and no direct target query.

Scorer and evaluator tokens may get request-scoped `case.target`.

Operator tokens may get broader access only explicitly.

### 5.5 Workspace queries

Workspace read operations are pure relative to a workspace snapshot.

Core workspace reads are snapshot, list, stat, read_file, digest, git_log, git_diff, and capture_artifacts.

Workspace reads MUST enforce path policy.

Workspace reads MUST label data classes.

Workspace reads MUST return snapshot identity.

Workspace reads MAY redact, truncate, or deny secret paths.

### 5.6 Projections

Projection is part of authorization.

Projection is not presentation sugar.

A query MUST state what it materializes.

Candidate projection may include origin, identity, scores, artifact projection, and summary fields.

Artifact projection is adapter-defined and core-enforced by surface fingerprint, schema fingerprint, byte limit, and grant.

Evidence projection is governed by evidence visibility.

Diff projection is adapter-defined and core-enforced by surface fingerprint and schema fingerprint.

Counts may be exact, bucketed, hidden, or denied by capability.

### 5.7 Predicates

Predicate language is closed and typed.

Raw host-language predicates are not accepted.

Predicates MUST be evaluated over public projected fields or authorized internal fields.

Predicate errors MUST NOT leak hidden field values.

### 5.8 Receipts

Every graph/case/workspace query receipt records operation hash, result hash, revision or snapshot, projection fingerprint, read scope fingerprint, source refs, redactions, materialized bytes, and count policy.

Receipts are graph truth about what a stage saw.

Receipts are not the same as semantic `InfoRef`s.

---

## 6. Capability model

### 6.1 Token separation

The bearer token is a secret credential.

The grant document is an auditable authority description.

Run artifacts persist grant fingerprints and summaries.

Run artifacts MUST NOT persist bearer tokens.

### 6.2 Grant structure

A grant is action, resource, and constraints.

Action names are path strings such as `graph.read`, `case.read`, `lm.complete`, `agent.run`, `proposal.submit`, and `assessment.submit`.

Resource selectors bind grants to runs, candidates, cases, resolved sets, evaluation requests, workspaces, runtime pools, LM pools, sandbox pools, model roles, and extension namespaces.

Constraints bind grants to query kinds, projections, schemas, data classes, byte limits, row limits, timeouts, cost limits, concurrency limits, model sets, tool policies, evidence visibility, and revision windows.

### 6.3 Execution policy

Execution policy is not capability.

Capability controls what Leaven will do.

Execution policy controls what the process can do outside Leaven.

A Python process with network access can bypass Leaven LM budget.

Remote and package-scorer profiles MUST use OS/container restrictions to make Leaven portals the only route to external effects.

### 6.4 Named execution profiles

`trusted_local_operator` permits local repo code with operator trust; BYO effects may be allowed and must be marked.

`managed_sandbox` denies network except Leaven endpoint and denies subprocess except `sandbox_exec` portals.

`package_scorer` is for package-registry evaluator/scorer code and defaults to no BYO network, no raw workspace mount, narrow target access, and narrow assessment writes.

`remote_untrusted` is the default for untrusted remote code and should grant only narrow reads and managed calls.

`custom` requires a full execution policy fingerprint.

### 6.5 Delegation

Delegation is attenuation-only.

A child token may reduce actions, resources, constraints, expiration, concurrency, budget, data classes, visibility, and execution profile.

A child token MUST NOT add an action missing from the parent.

A child token MUST NOT widen resources.

A child token MUST NOT increase budget or expiration.

A child token MUST record parent fingerprint.

### 6.6 Authorization algorithm

Parse plan.

Resolve token.

Normalize grants.

Resolve base revision.

Derive static required capabilities.

Reject obvious unauthorized actions before side effects.

Evaluate pure reads with projection and redaction.

Resolve data-class labels for call inputs.

Perform dynamic authorization for calls.

Reserve and charge budget.

Resolve write values.

Perform dynamic authorization for writes.

Validate preconditions.

Commit graph writes through `RunContext`.

Return values, receipts, redactions, charges, errors, base revision, and final revision.

### 6.7 Data-class labels

Core data classes are public, case.input, case.target, case.metadata, candidate.output, candidate.artifact, workspace.file, workspace.secret, scorer.private, evaluator.private, optimizer.visible, prompt.raw, completion.raw, transcript.raw, human.review, external.secret.

Namespaces may add `x.<namespace>` labels.

Every managed call declares input classes.

Every call grant declares allowed and forbidden input classes.

Policy MUST reject a call if any input class is forbidden or absent from allowed classes.

Data classes propagate through templates, extracts, aggregation, and evidence envelopes.

### 6.8 Target egress

Target access grants reading target data.

Target egress grants sending target-derived data to an external effect.

They are separate.

A deterministic grader may read target and compute local score without target egress.

An LM judge that sees the target requires both target access and LM target egress.

An agent judge that sees the target requires both target access and agent target egress.

---

## 7. Stage payloads

### 7.1 Reflector payload

`ReflectRequestV1` is build-once and target-safe.

It may include runner-visible input, candidate output, score, scorer feedback, trace refs, source refs, side info, and redaction metadata.

It MUST NOT include raw targets, answer keys, hidden reference solutions, scorer-only metadata, hidden split internals, or secrets.

Reflectors MAY issue live graph queries through their token.

Live reflector queries are policy-projected and receipted.

Reflectors MUST NOT reconstruct raw case examples by bypassing target-safe projection.

### 7.2 Runner payload

`RunRequestV1` is target-free.

It contains candidate/artifact view, target-free case input, output contract, and metadata.

A runner may execute candidate behavior but should not see hidden targets.

### 7.3 Scorer payload

`ScoreContextV1` is target-aware only when the grant permits it.

A scorer may receive target inline or fetch it through receipted `case.read`.

A scorer returns assessments or score evidence, not graph mutations directly unless routed through `assessment.submit`.

### 7.4 Evaluator payload

Evaluator workers receive `EvaluationJobV1` or a ref to it.

The evaluator job is request-scoped.

The evaluator token gates case target reads, workspace access, LM calls, agent runs, sandbox exec, and assessment writes.

### 7.5 Preference payload

Pairwise/listwise preference stages may see multiple candidate outputs and case context.

Target visibility follows the same case policy as scorers/evaluators.

### 7.6 Adapter payloads

Artifact adapters own artifact projection/diff/change/materialize/parse semantics.

Dataset adapters own case loading, set resolution, input projection, target projection, and fingerprinting.

Adapter outputs MUST carry schema fingerprints.

---

## 8. Evaluator privilege model

### 8.1 Evaluator as privileged stage

An evaluator runs candidates and produces assessments.

An evaluator may be a Python function, DSPy program, agent-backed judge, sandboxed verifier, theorem prover, test harness, human-review queue, or composition of these.

The evaluator’s local decision logic is opaque to Leaven unless instrumented.

Every privileged boundary SHOULD use Leaven portals.

### 8.2 Request scope

Evaluator authority is scoped to one `evaluation_request_id`.

Authority includes only the resolved set, candidates, pairs/groups, case IDs, purpose, granularity, and assessment shape named by the job.

An evaluator token MUST NOT submit assessments for unrequested candidates or cases.

An evaluator token MUST NOT apply proposals.

An evaluator token MUST NOT mutate population state directly.

### 8.3 Target reads

Target material should be loaded through `case.read` to create a receipt.

Inline targets are allowed only when policy explicitly permits non-receipted materialization.

A target read receipt is private by default.

Reflectors must not see target read receipts unless redacted to source existence only.

### 8.4 DSPy evaluators

Leaven provides a DSPy LM adapter.

The adapter maps DSPy LM calls to `Call { kind: lm_complete }`.

DSPy module control flow remains Python.

DSPy callbacks SHOULD record module spans and trace refs.

A DSPy evaluator that uses BYO LM client is `has_untracked_external_effects` unless wrapped in a declared external effect.

### 8.5 Agent evaluators

Agent judge sessions route through `agent_run` by default.

`agent_run` records runtime fingerprint, tool policy, instructions hash, transcript refs, command refs, output refs, raw provider event visibility, cost, and effect receipt.

Multiple agents may run concurrently subject to `max_concurrent` and budget.

BYO agent spawning is trusted-local only and degrades replayability.

### 8.6 Workspace inspection

Workspace inspection is core.

Read-only workspace operations are `Let` expressions over snapshots.

Workspace materialization and command execution are `Call` effects.

Path policy MUST protect secrets.

Workspace operations MUST record snapshot hashes.

### 8.7 Evidence envelope

Evaluator evidence MUST be visibility-labeled.

Public evidence is optimizer-visible subject to policy.

Private evidence is evaluator/operator-visible only.

Target-derived status MUST be explicit.

Trace refs MUST include visibility labels.

Raw transcripts, prompts, completions, and target-derived private traces MUST NOT become reflector-visible by accident.

### 8.8 Assessment write

`submit_assessments` is request-scoped.

The engine validates shape, granularity, candidates, cases, purpose, resolved set, and evidence visibility.

Assessment cost attribution SHOULD cite managed effect receipts.

A scorer/evaluator may declare external effects only if capability grants allow it.

---

## 9. Proposal model

A proposal write is creation-side.

A proposal effect is create, change, or change from workspace diff.

A create effect supplies a full artifact under an artifact schema.

A change effect supplies an adapter-defined change under a surface fingerprint and change schema.

A workspace-diff effect supplies workspace ref, roots, parser ref, surface fingerprint, and change schema.

Causal provenance determines content lineage.

`informed_by` records semantic graph refs relied on.

`read_receipts` record exact projected/redacted values observed.

`informed_by` MUST NOT be used as a substitute for read receipts.

Read receipts MUST NOT be used as a substitute for semantic source refs.

---

## 10. Assessment model

Assessment shapes are independent, pairwise, and listwise.

Independent assessments target one candidate and one case/aggregate scope.

Pairwise assessments target two candidates and a case/aggregate scope.

Listwise assessments target an ordered or scored set of candidates.

Per-case granularity produces one row per candidate/case or comparison/case.

Aggregate granularity produces one row over a resolved set.

Evidence visibility controls what downstream stages may see.

Hidden target data may influence score and feedback, but raw target data must not leak unless policy explicitly allows it.

---

## 11. LM, agent, sandbox, and human calls

### 11.1 LM complete

`lm_complete` is provider-neutral.

It carries model or model role, purpose, messages, output contract, cache policy, limits, and input data classes.

It returns content, parsed output when applicable, model/runtime fingerprint, cost, cache status, trace refs, and receipt.

Raw prompt and raw completion visibility are policy-controlled.

### 11.2 Agent run

`agent_run` runs a provider-neutral agent session in a workspace.

It carries runtime, workspace, instructions, env policy, tool policy, output contract, limits, and input data classes.

It returns status, transcript refs, commands, output files, raw provider event refs, workspace diff refs, cost, trace refs, and receipt.

The agent runtime does not know optimizer types.

### 11.3 Sandbox exec

`sandbox_exec` runs commands or verifiers under a sandbox policy.

It carries workspace, argv, env policy, timeout, output contract, resource limits, and input data classes.

It returns exit status, stdout/stderr refs or redacted text, output file refs, cost if any, trace refs, and receipt.

Sandbox exec is the managed substitute for uncontrolled subprocess in non-trusted profiles.

### 11.4 Human review

`human_review` represents costful or asynchronous human judgment.

It carries queue, rubric, inputs, data classes, SLA, and output schema.

It returns review result, reviewer visibility class, cost, trace refs, and receipt.

Human review is a call, not a graph write.

---

## 12. Watch protocol

Watches are sibling protocol objects.

A watch may stream run events or graph changes from a cursor/revision.

A watch must have backpressure, heartbeat, cursor, projection, and cancellation.

A watch uses the same token and capability grants.

A watch is denied by default for cross-run sources.

A watch result must use redaction and count policy like normal reads.

---

## 13. Extensions

Core IR is closed.

Extensions are registered.

An extension declares namespace, op, input schema fingerprint, output schema fingerprint, required action, data-class behavior, source-ref extraction, redaction behavior, and replay behavior.

Extension payloads are not arbitrary metadata.

No extension may bypass capability checks.

No extension may bypass hidden target policy.

No extension may become a cross-stage dependency without schema fingerprinting.

---

## 14. Security invariants

Never persist bearer token secrets.

Never let explicit case lists bypass hidden-partition checks.

Never equate target read with target egress.

Never send hidden targets to remote LMs or agents without explicit input-class grant.

Never make raw prompts, completions, transcripts, or provider events visible by default.

Never let redaction reasons leak more than policy allows.

Never expose exact counts when count policy says bucketed or hidden.

Never let workspace path traversal bypass path policy.

Never let BYO effects masquerade as managed effects.

Never make an evaluator token a root token.

Never let a package scorer inherit trusted-local defaults.

Never expose `RunContext` to foreign-language workers.

Never let public schema be accidental Rust serde output.

---

## 15. Replayability classes

`pure_read` means no managed effect or graph write occurred.

`fully_managed` means every external effect crossed Leaven portals and every write committed through `RunContext`.

`boundary_managed` means host-language control flow was opaque but all external effects were managed.

`has_declared_external_effects` means BYO effects were declared but not replayable by Leaven receipt.

`has_untracked_external_effects` means the run is not fully auditable or replayable.

Managed evaluator code should target `boundary_managed` or better.

Trusted local research code may accept degraded replayability explicitly.

Package scorers should not be allowed below `boundary_managed` without operator override.

---

## 16. Cost, quotas, and abuse controls

Budget is real money and provider-metered units.

LM calls, agent dispatch, sandbox pools with paid compute, human review, judge calls, and external verifier services may charge budget.

Graph queries do not charge dollar budget.

Graph queries still need quotas.

Quotas include max query nodes, max rows, max bytes, max depth, max wall time, max concurrent reads, max watch backlog, max exact counts, and max materialized artifacts.

A quota denial is not a budget denial.

A quota truncation must be labeled.

Cost attribution should be by effect receipt.

Double charging is forbidden.

---

## 17. Versioning and compatibility

`v1` schemas are stable once published.

Additive extension fields belong in `metadata` or registered extension payloads.

Core unknown fields are rejected.

New core variants require a new minor schema package or explicit `x.<namespace>` extension.

Removing fields requires a new major protocol version.

Changing semantic meaning requires a new major protocol version.

SDKs should preserve unknown extension payloads they do not understand when forwarding.

SDKs should not manufacture core variants not declared by schema.

---

## 18. Implementation checklist

The engine implements one authorization kernel for all transports.

The engine resolves case sets to partitions before case authorization.

The engine records query receipts separately from semantic source refs.

The engine records effect receipts separately from write receipts.

The engine validates data-class egress before LM/agent/sandbox/human calls.

The engine validates assessment writes against evaluation request scope.

The engine validates proposal writes against surface and change schemas.

The engine does not expose raw graph mutation APIs to workers.

The engine logs capability fingerprint, policy fingerprint, execution policy fingerprint, code/package fingerprint, and runtime fingerprints.

The SDK makes Leaven portals easier than BYO providers.

The SDK provides batching without making concurrency mandatory.

The Python SDK provides `LeavenDSPyLM` and optional DSPy trace callbacks.

The CLI compiles queries to plans and prints receipts by default in machine-readable modes.

The remote service constrains OS/network execution separately from token grants.

---

## 19. Worked architecture: evaluator boundary split

Python-local: loops, conditionals, parser helpers, score formulas, DSPy module definitions, retry choices, ensemble aggregation, object construction.

Leaven-managed: case target reads, workspace materialization, git diff, pytest sandbox exec, Codex/Claude judge session, DSPy LM call, assessment submit.

The evaluator code should feel like normal Python.

The expensive and privileged edges should look like `cx.case.load`, `cx.workspace.git_diff`, `cx.sandbox.exec`, `cx.agent.run`, `cx.lm.complete`, and `cx.assessments.submit`.

The result should cite read receipts and effect receipts.

The assessment should expose only public evidence to optimizers and keep target-derived private trace in private evidence.

---

## 20. Anti-patterns

Do not create a separate Python-only callback protocol.

Do not put RPC method sprawl where a plan value belongs.

Do not force DSPy control flow into Leaven IR.

Do not let evaluators become root operators.

Do not use `metadata` as a hidden cross-stage API.

Do not copy untyped `signals: dict[str, Any]` as a dependency mechanism.

Do not make MCP the ontology.

Do not make Inspect, DSPy, SkillBank, or HF Datasets the core artifact shape.

Do not make OTel logs the source of truth.

Do not treat BYO LM calls as budgeted.

Do not treat declared external effects as replayable.

Do not make redaction a lossy afterthought.

Do not persist raw tokens.

---

## 21. Normative schema package

The `schemas/` directory is part of this lock draft.

`common.schema.json` defines common IDs, refs, fingerprints, score, cost, output contracts, trace refs, redactions, and extension objects.

`leaven.plan.v1.schema.json` defines plan, expressions, graph/case/workspace query IR, managed calls, writes, preconditions, and commit policy.

`leaven.plan_result.v1.schema.json` defines plan results, value variants, receipts, redactions, charges, and errors.

`leaven.capability.v1.schema.json` defines grant documents, action/resource/constraints, execution policy, and delegation.

`leaven.stage_payloads.v1.schema.json` defines worker invocation payloads and role-specific public contexts.

`leaven.evaluation_job.v1.schema.json` defines evaluator jobs.

`leaven.evidence_envelope.v1.schema.json` defines public/private evidence channels.

`leaven.watch.v1.schema.json` defines watch requests.

`leaven.worker_protocol.v1.schema.json` defines JSON-RPC worker envelopes.

The schemas are intentionally more detailed than the Markdown.

The Markdown carries judgment.

The schemas carry wire shape.

Both are normative for this draft.

