# Leaven Public Seam V1 — Comprehensive Design Pass Notes

> **Purpose of this document.** This is the transferable judgment behind the Leaven Public Seam V1 lock draft. It captures (a) the architectural commitments we built up over a long conversation, (b) the audit findings from a line-by-line read of the candidate lock draft, (c) the open questions where we want a sharper position, and (d) the things we want preserved against pressure. It is intentionally opinionated and intentionally redundant in places where redundancy makes the spec safer.
>
> **Audience.** Frontier models (Claude, GPT-5.5 Pro, etc.) reviewing or refining the lock draft. Engine implementers and SDK authors as a long-form companion to the normative spec. Future contributors who need to understand *why* the lock draft is shaped this way.
>
> **Tone.** Treat this as a peer-level briefing. Push back on positions we got wrong. Bring solutions we haven't seen. Prior art is welcome, named explicitly, adapted not imitated. Don't hedge on the tensions — pick a side and argue for it.
>
> **Status.** Companion notes to `leaven_public_seam_v1_lock_spec.md` and the `schemas/` package. Not itself normative. When this document and the lock spec disagree, the lock spec wins for wire shape; this document wins for *why*.

---

## 0. What Leaven is, in one paragraph

Leaven is a Rust workspace for LLM agent learning — optimizers (GEPA, MIPRO, TextGrad, FlashEvolve-style async, future research) operating over typed artifacts (prompts, skill banks, harnesses, code, DSPy programs, anything) with a typed `RunGraph` recording lineage, proposals, assessments, evidence, costs, and policy. The optimizer rhythm is owned by optimizer crates. The engine owns durable state — `RunGraph`, `RunContext` (the only mutation authority), evaluation cache, budget ledger, trust policy, persistence, callbacks. Stages (proposer, evaluator, scorer, reflector, judge, runner, callback) own side-effectful work. Today users write paper repros as 8000-line Rust programs (EvoSkill being the proof case). The goal of the public seam is: make the next paper repro 200 lines of glue in whatever language the user wants — Python first, then TS, then ship a CLI any language can drive — without losing graph truth, target safety, budget accounting, replay, audit, or remote/multi-tenant deployment.

## 1. The forcing functions

Two forcing functions justify everything else.

**Near term.** A team member writing a new paper repro should write Python (or TS, or shell, or whatever they want) at the boundary, not Rust. The interior of their evaluator/scorer/reflector should look like ordinary host-language code — loops, conditionals, helper functions, DSPy modules, Codex/Claude agent sessions, pytest, parsers, retries. The expensive and privileged edges should be Leaven portals. The ergonomic target is "200-line Python evaluator that calls cx.case.load / cx.workspace.git_diff / cx.sandbox.exec / cx.agent.run / cx.lm.complete / cx.assessments.submit." Anything more ceremonious than that is failure.

**Medium term.** External users — researchers running their own optimizers, organizations deploying Leaven over the network, IDE integrations, replay tooling, multi-tenant cloud — need the same wire to work. CLI as canonical client, SDKs as thin wrappers, agents-in-workspaces using `leaven` like `git log`, remote deployment over HTTPS with capability tokens. One protocol; many transports; many languages.

## 2. The conversation arc — how we landed here

This is the timeline of *why* the lock draft looks the way it does. Read this so the architectural commitments don't feel arbitrary.

**Phase 1 — How do we ship the binary?** Started with the question: how do users glue together Leaven from outside Rust? We considered PyO3 bindings (rejected — manylinux wheel matrix, Python ABI versioning, GIL + Tokio integration hell, locks Leaven to Python forever), CLI-only with declarative-only callbacks (rejected — user explicitly pushed back: "no declarative only would be terrible"), DSPy/DSRs as the integration surface (rejected as primary — works as an *adapter*, not the wire), MCP for everything (rejected as canonical — wrong-shaped primitives for stages, wrong cognitive model). Landed on CLI-as-bridge: same `leaven` binary works at the user's terminal, inside a Codex/Claude agent's workspace (the agent uses `leaven query lineage` like `git log`), inside a Python worker process (subprocess to CLI or speak JSON-RPC directly), inside remote containers. One seam; many consumers.

**Phase 2 — How is privilege controlled?** Worked through the capability-token model. Tokens are bearer credentials passed out-of-band (env var on subprocess spawn, header on HTTPS). The grant document behind a token is structured (Cedar-shaped: action + resource + constraints), fingerprinted, attenuable (macaroon-style child tokens can only narrow, never widen), persistable by fingerprint and summary (never as bearer secret). Every transport enters the same authorization kernel. Tokens are scoped to a stage call lifetime by default. Delegation is attenuation-only.

**Phase 3 — Evaluator privilege.** Evaluators are intentionally more privileged than other stages — they see hidden targets, execute candidates, inspect workspaces, run multiple agents/programs concurrently. User explicitly pushed back on speculative deferrals: "sandbox.exec has a concrete user; agent evals need arbitrary, heterogeneous environments — that compromises the core quality of the library." Settled on: evaluator interior is arbitrary host-language code (Python, DSPy programs, Codex sessions, all running freely), only the *boundaries* cross typed IR Call nodes. Capability tokens decide which boundaries are mandatory (LM through Leaven, agent through Leaven, workspace reads through Leaven) versus escapable (BYO subprocess/network in trusted profiles). Trust profiles bundle execution policy + capability defaults (`trusted_local_operator | managed_sandbox | package_scorer | remote_untrusted`).

**Phase 4 — Data-class taint labels.** Oracle introduced this as the load-bearing security primitive we hadn't named: target *access* and target *egress* are separate concepts. An evaluator with `case.target` read access AND network access AND a permissive LM grant can still leak hidden targets to an external LM provider unless the grant explicitly excludes `case.target` from `allowed_input_classes`. Closed enum of 16 data classes (`public | case.input | case.target | case.metadata | candidate.output | candidate.artifact | workspace.file | workspace.secret | scorer.private | evaluator.private | optimizer.visible | prompt.raw | completion.raw | transcript.raw | human.review | external.secret`) plus `x.*` extension namespace. Every managed call declares input classes; every grant declares allowed and forbidden input classes; policy rejects calls that violate either direction.

**Phase 5 — The IR is a value.** This is the deepest commitment. The IR is not RPC dispatch; it is a typed value language. Workers build typed values describing computations over the graph; the engine evaluates the value. Composition lives in the IR. Multiple steps in one round trip. The engine sees the whole shape and can optimize, fingerprint, cache, replay. Same value serialized travels over CLI flags or JSON-RPC over Unix socket or HTTPS or in-process embedding. This commitment is what makes everything else possible — lazy execution, optimization, fingerprinting, determinism at a revision, cross-transport equivalence, provenance threading via `refs_from_result`, inspectability.

**Phase 6 — Reflection vs proposal as separate stages.** User flagged this as a load-bearing principle that almost got missed: reflection (understanding what happened) and proposal (deciding what to do about it) are different cognitive operations and LMs do *one thing* well. DSPy's `ChainOfThought` → `Predict` split, GEPA's two-phase prompt evolution, ACE's diagnostic-then-patch, FlashEvolve's reflective async repair — every research-grade pattern enforces this separation. Collapsing them into one stage makes the LM do two things at once, which is the exact failure mode language models are worst at. The lock draft *names* reflector and proposer as separate roles but doesn't ship distinct typed payloads for both. This is the most important must-fix.

**Phase 7 — The ACP pivot.** User asked: "for worker, is that supposed to be like an agent? If so, we might want to use ACP and add those functionalities." Dispatched a research agent on Agent Client Protocol. ACP confirmed as a credible base protocol: programmatic permission answering supported by spec ("Clients MAY automatically allow or reject permission requests according to the user settings"), multi-session per process, streaming via JSON-RPC notifications, JSON-RPC 2.0 over stdio canonically. Gaps: no flow control primitive (reference Rust SDK uses unbounded mpsc — OOM hazard for high-volume notifications), no sampling primitive (the LM-call-from-server-to-client pattern that MCP has). The right shape is **ACP + MCP-over-ACP side by side on one socket**: ACP carries stage-call lifecycle + permission gating; MCP-over-ACP carries the LM/sandbox/tool/graph callback channel; engine enforces capability-token grants on every call. This replaces `leaven.worker_protocol.v1` entirely.

**Phase 8 — Schema audit.** Read the 9 schemas, 3 examples, and 1068-line lock spec line by line. Surfaced 100+ issues across all severity levels. Most are mechanical fixes; a handful are architectural gaps (missing `ProposeRequestV1`, missing `Score.output` on the assessment-write side, unspecified schema fingerprint algorithm, unspecified JSONPath/template/field-path dialects, the worker_protocol that goes away with the ACP pivot).

## 3. The architectural commitments — what to defend

These are the load-bearing positions. We arrived at each one deliberately. They should not be argued away in subsequent passes without a *concrete and demonstrably better* alternative. Each is stated as a *commitment* with the reason it matters.

### 3.1 The IR is a value, not a call.

`leaven.plan.v1` is a typed value language with three semantic node classes: `Let` (pure read expressions, deterministic at resolved revision), `Call` (metered external effects — LM, agent, sandbox, human, workspace materialize, extension — replayable by receipt), `Write` (append-only graph mutation intents committed by `RunContext`).

Why this matters: every property we care about — composability with host languages, fingerprinting, replay, caching, optimization, cross-transport equivalence, provenance threading, inspectability — depends on the IR being data, not RPC. If the IR is a flat dispatch table (`{ "method": "lineage", "args": {...} }`), composition lives in client code and the engine sees one operation at a time; we lose every optimization opportunity, every cache lookup, every replay guarantee. Polars expressions, LINQ, SQL queries, Cypher patterns all win because the query is *data*. Leaven's IR follows the same shape.

The IR also commits us to a discipline: **the engine evaluates the value**, the client builds it. This means SDKs are thin builders that produce typed values, not RPC stubs.

### 3.2 The capability token is the architectural spine.

Every transport — CLI command, Python SDK call, TS SDK call, agent-in-workspace shell-out, local Unix socket, remote HTTPS, future MCP adapter — enters the same authorization kernel. The bearer token is an opaque secret credential passed out-of-band. The grant document behind the token is a structured Cedar-shaped value: principal (subject), action (path-string), resource (selector), constraints (typed). Delegation is macaroon-style attenuation only — a child token can only restrict, never widen.

Why this matters: collapses every load-bearing question onto "what does the token allow." Can the worker query lineage? Token decides. Can the worker propose? Token decides. Can the worker spawn an agent? Token decides. Can it make BYO LM calls? Capability + OS execution policy together decide. Cross-run access? Token capability gate. Local-dev ergonomics? Trust profile defaults bundle the capabilities. Audit? Persisted token fingerprints. Replay? Capability fingerprint pinned in plan results. We don't invent — capability/bearer-token patterns are well-trodden (Kubernetes RBAC, OAuth scopes, SPIFFE workload identity, AWS STS session tokens, macaroons).

### 3.3 Managed boundaries; arbitrary host-language interior.

The interior of a worker (reflector, scorer, evaluator, proposer, judge, runner) is arbitrary code in the worker's language. The boundaries crossing into Leaven are typed IR portals. Specifically:

**Crosses IR:** graph queries (read graph state), case reads (read case input/target/metadata under policy), workspace reads (snapshot/list/read/stat/digest/git_log/git_diff/capture_artifacts), workspace materialize (open a mutable handle), sandbox exec (run command), LM complete (make LM call with cache+budget+replay), agent run (spawn Codex/Claude session), human review (queue for human judgment), proposal submit (create proposal batch), assessment submit (record evaluation result).

**Stays in host language:** loops, conditionals, parser helpers, score formulas, DSPy module definitions, retry choices, ensemble aggregation, branching logic, object construction, control flow generally.

Why this matters: Leaven is not a workflow DSL. Forcing all evaluator logic through IR Call nodes would (a) explode the IR surface area, (b) make DSPy-as-evaluator awkward (you'd need a DSPy-to-IR compiler nobody asked for), (c) provide no real benefit because evaluator-interior logic is invisible-to-Leaven anyway. The user owns their interior; Leaven owns the boundary.

### 3.4 Evaluator privilege is request-scoped and egress-scoped.

Evaluators are intentionally more privileged than other stages. They see hidden targets, execute candidates, inspect workspace state, call grader LMs, spawn judge agents, run sandbox commands, accumulate assessments. **But** that privilege is scoped to one `evaluation_request_id`, one resolved candidate/case set, one shape, one purpose, one granularity. An evaluator token MUST NOT submit assessments for unrequested candidates/cases. An evaluator token MUST NOT apply proposals or mutate population state directly.

And: **target access does not imply target egress**. Reading the target locally for deterministic grading is different from sending target-derived data to an external LM provider or agent runtime. The data-class taint labels (§3.6) make this enforceable.

### 3.5 Visibility lives in results, not just in policy.

Every managed read returns a receipt with revision, projection fingerprint, redaction list (typed reasons), source refs, materialized bytes, and visibility. Every managed effect returns a receipt with runtime/model/tool policy fingerprint, cost, transcript/prompt visibility, and replay class. Evaluator evidence is split into `public` (optimizer-visible after policy projection) and `private` (evaluator/operator-only) channels with per-consumer `redaction_policy` (optimizer/reflector/operator gets different projections). Scorer output is the *ordinary* bridge from hidden target data into optimizer-visible evidence, so that bridge must be labeled and auditable.

### 3.6 Data-class taint labels are the v1 security primitive.

Closed enum of 16 classes (`public | case.input | case.target | case.metadata | candidate.output | candidate.artifact | workspace.file | workspace.secret | scorer.private | evaluator.private | optimizer.visible | prompt.raw | completion.raw | transcript.raw | human.review | external.secret`) plus `x.*` extension namespace. Every managed call declares input data classes. Every call grant declares `allowed_input_classes` and `forbidden_input_classes`. The engine MUST reject a call if any input class is forbidden or absent from allowed classes. Data classes propagate through templates, extracts, aggregation, and evidence envelopes.

This is what catches target leaks structurally. Without it, an evaluator with `case.target` read and a permissive LM grant can silently send the answer key to OpenAI's API. With it, the LM grant declares `allowed_input_classes: [public, case.input, candidate.output]` and `forbidden_input_classes: [case.target]`; the engine rejects the call at ingress if the value flowing into it carries the `case.target` label.

### 3.7 RunContext remains the only graph mutation authority.

The public IR never inserts graph records directly. Proposal submission, assessment submission, evaluation requests, proposal application, population events, and run events all commit through `RunContext`. The engine MUST validate capabilities, preconditions, graph revision, schema fingerprints, hidden-partition policy, idempotency, and stage authority before graph mutation. External effects are not rolled back (LM money already spent is real); graph writes may be atomic across write nodes when commit policy says so.

### 3.8 Blessed LM and agent dispatch are first-class Call nodes.

The default path for expensive operations is *through Leaven*. `lm_complete` and `agent_run` are core `Call` variants. Managed calls get cache, budget, replay, telemetry, provider policy, receipts, and provenance — for free, by default, with the lowest-friction code path. BYO network/subprocess is an explicit trust-profile escape hatch that degrades replayability. The capability token controls whether BYO is permitted; the execution policy (OS/container level) controls whether BYO is *possible* outside Leaven.

### 3.9 Query IR supersedes the closed `StageQuery` enum.

The old `StageQuery` enum (Help/ListCandidates/Candidate/Assessment/Evidence/Lineage/Diff) was right as scaffold but wrong as a public forever-wire. The public query language is a closed-but-rich algebra over sources (candidate/candidates/proposal/proposal_batch/proposal_batches/assessment/assessments/population_events/recent_failures/costs/events/candidate_tree/extension_source), traversal steps (parents/children/lineage/siblings/informed_by/informed/proposal_that_created/assessments/pairwise_assessments), predicates (eq/ne/gt/gte/lt/lte/in/and/or/not), projections (ids/summary/candidate/artifact/assessment/evidence/diff/cost_summary/event_summary/extension), and limits.

Time-travel and diff queries are v1 (`consistency.at_revision`, `consistency.since_revision`). Subscriptions are a sibling protocol (`leaven.watch.v1`), not infinite plans.

### 3.10 Artifact semantics live in adapters, not core.

Core Leaven knows: refs (CandidateRef, ProposalRef, AssessmentRef, CaseRef, WorkspaceRef, ExternalRef), revisions (GraphRevision), receipts (ReceiptRef), surface fingerprints (SurfaceFingerprint), schema fingerprints (SchemaFingerprint), projections (mode-based on common shapes), changes (effect kinds), diffs, source refs.

Core Leaven does NOT know: DSPy module internals, skill bank structure, Inspect solver shape, theorem prover proof state, git tree internals, browser trace structure, multimodal artifact internals. These are adapter namespaces (`x.dspy.*`, `x.skill_bank.*`, `x.inspect.*`, etc.) with their own typed payloads, schema fingerprints, and capability constraints.

### 3.11 Determinism is exact and honest.

Pure reads are deterministic at fixed graph revision, case-set version, projection fingerprint, adapter fingerprint, read policy fingerprint, capability fingerprint. Fresh LM/agent/sandbox/human calls are *not* deterministic. Replay of managed effects is by effect receipt. Writes are replay-safe only through idempotency keys and commit preconditions.

This corrects an earlier framing ("same IR + revision = same result forever") that Oracle pushed back on. The honest rule: **same IR + revision + same recorded effect receipts = same replayed result**. Replay-by-receipt, not replay-by-wishful-determinism.

### 3.12 Reflection vs proposal are structurally separate stages.

Reflection (understanding what happened — building a structured diagnostic from examples and graph state) and proposal (deciding what to do about it — emitting a typed proposal batch) are different cognitive operations. LMs do *one thing* well. Mixing them is the failure mode language models are worst at. Every research-grade pattern enforces this separation (DSPy's ChainOfThought→Predict, GEPA's two-phase, ACE's diagnostic-then-patch, FlashEvolve's reflective async repair).

The optimizer's loop is `reflect → propose → apply → evaluate`, not `reflect-and-propose-in-one-shot → apply → evaluate`. The schemas should have distinct `ReflectRequestV1` and `ProposeRequestV1` payloads with `ReflectionResultV1` as the bridge. The reflector LM call produces insight; the proposer LM call (or non-LM proposer) takes the insight and emits a proposal batch.

The current lock draft has *roles* `reflector` and `proposer` listed but only ships `ReflectRequestV1`. This is the most important architectural must-fix.

### 3.13 ACP profile + MCP-over-ACP, not invented worker protocol.

Worker processes are agent-shaped. ACP (Agent Client Protocol) is purpose-built for this — JSON-RPC 2.0 over stdio, session lifecycle, streaming updates, programmatic permission negotiation (spec explicitly endorses non-UI answers: "Clients MAY automatically allow or reject permission requests"), filesystem and terminal methods, multi-session per process. We adopt ACP as the base transport and ship a "Leaven ACP profile" defining Leaven-namespaced extension methods (`leaven/graph.query`, `leaven/case.load`, `leaven/proposal.submit_batch`, etc.).

ACP lacks a sampling primitive (the LM-call-from-server-to-client pattern), so we run **MCP-over-ACP** side by side on the same socket: ACP for stage-call lifecycle and permission gating; MCP for LM/sandbox/tool/graph callbacks. The engine exposes itself as an MCP server reachable over the ACP socket; the worker calls `lm.complete`, `sandbox.exec`, `graph.read`, `agent.run` as MCP tool calls; the capability token gates every call.

ACP gaps to address at the integration layer: bounded channels (reference Rust SDK uses unbounded mpsc — OOM hazard at high notification volume), credit-based flow control if needed, pinned to ACP v1 (v2 just landed today; lots of RFDs in flight; track but don't ship against). Naming friction: ACP's mental model is "IDE-client + coding-agent" while we use "engine-client + worker-agent" — JSON-RPC roles map cleanly but documentation needs to call out the inversion.

This replaces `leaven.worker_protocol.v1.schema.json` entirely.

### 3.14 Trust profiles bundle execution + capability defaults.

Named profiles: `trusted_local_operator | managed_sandbox | package_scorer | remote_untrusted | custom`. Each profile bundles execution policy (network, subprocess, filesystem, BYO effects, secrets handling) with capability defaults. Project configuration declares the profile once per evaluator/scorer/reflector entry point; runtime applies it without prompting. This solves the local-dev-painful problem — team-written evaluators get the permissive `trusted_local_operator` profile by default because they live in the repo; package-registry scorers get the restrictive `package_scorer` profile by default.

Capabilities refine profiles; execution policy is enforced by OS/container in tandem (capability tokens cannot stop a Python process from opening a socket on their own — sandboxing must happen at the process level).

### 3.15 Replayability classes are honest about what's reproducible.

Five classes: `pure_read | fully_managed | boundary_managed | has_declared_external_effects | has_untracked_external_effects`. Most evaluators will be `boundary_managed` — host-language interior is opaque but every expensive/external effect is receipted. Trusted local research code may accept `has_untracked_external_effects` explicitly. Package scorers should not be allowed below `boundary_managed` without operator override.

This is per-assessment ideally, not per-plan (an audit finding — currently the lock draft has it at plan level only; should be at the AssessmentWrite level too).

### 3.16 Visibility-labeled evidence envelope is required, not optional.

Every evaluator/scorer assessment carries an `EvidenceEnvelopeV1` with structurally required `public` channel (optimizer-visible after redaction), optional `private` channel (evaluator/operator-only), required `target_derived: boolean`, optional `data_classes` set (should be required when `target_derived: true`), required `redaction_policy { optimizer, reflector, operator, raw_prompt_logging?, raw_transcript_logging? }`, optional `trace_refs`, optional `source_receipts`, optional `metadata`.

Without this structure, an evaluator that writes `evidence.feedback = "expected answer was 42, got 41"` silently leaks the answer key to reflectors. The envelope's structure makes leaks structurally hard.

## 4. The conversation's unresolved tensions (open questions for Oracle)

These are real questions where we have a current best guess but want a sharper position. Push back if you see a better answer.

### 4.1 How should ProposeRequestV1 be shaped?

A reflector produces a structured *reflection result* — diagnostic, suggested edit direction, identified failure modes, target-safe summaries of what went wrong. A proposer receives that reflection result plus parent candidate plus live query authority and produces a `ProposalBatchWriteV1`. What should `ReflectionResultV1` carry? Some candidates:

- Free-text rationale + structured "what to change" hints (closer to GEPA's reflection rendering)
- Typed edit suggestions per surface part (closer to ACE's diagnostic structure)
- Just text + source refs, let the proposer figure it out (lowest commitment, lowest leverage)
- A small structured DSL describing "the parent's part X failed on cases Y because Z; consider editing to Z'"

The choice affects how separately swappable reflectors and proposers are. The richer the reflection result, the more proposer LM calls become "translate this typed insight into a proposal" rather than "redo the whole reasoning."

Oracle pick: what shape best serves the principle of LMs doing one thing well?

### 4.2 Where exactly does Score.output live on the wire?

Recent Leaven Rust change (`leaven-run/AGENTS.md`) requires `Score::with_output(...)` for every successful score, with `MissingReportableOutput` failure mode if absent. The scorer-supplied `OutputRecord` is what reports, evidence stores, and GEPA reflection consume.

The wire schema has `output: {}` on `ScoreContextV1` (the scorer's *input* — line 228 of `stage_payloads.v1`). But on the *output* side: `Score` in `common.schema.json` (line 384-422) has no `output` field; `AssessmentWriteV1::Independent` references `Score` plus `evidence: EvidenceEnvelopeV1`; `EvidenceEnvelopeV1.public` has `score, feedback, metrics, summary, trace_refs` but no `output`.

Options:
- Add `output: OutputRecord` to `Score` directly. Simpler for consumers; bigger Score type.
- Add `output: OutputRecord` to `EvidenceEnvelopeV1.public` as a required field. Keeps Score scalar; evidence carries the structured output.
- Introduce a new top-level field on `AssessmentWriteV1::Independent` (and listwise/pairwise) for output. Most explicit; most schema churn.
- Define `OutputRecord` as a closed shape (e.g., `{ kind: "text" | "json" | "agent_session" | "workspace_diff" | ..., value: typed }`) and put it in `common.schema.json`.

Oracle pick: which placement aligns best with the new Rust contract and minimizes the chance scorers forget to attach output?

### 4.3 Schema fingerprint algorithm.

§3 of the lock spec says "Every schema fingerprint MUST be computed over canonical schema bytes chosen by the implementation." This is loose enough that two implementations will diverge. Need to pin:

- Canonical JSON form (RFC 8785 JSON Canonicalization Scheme is the obvious candidate)
- Hash algorithm (SHA-256 by default; Blake3 for performance if we want it; both supported with prefix in `fp_sha256_...` vs `fp_blake3_...`?)
- What's included in the bytes (just the schema fields? schema + `$id` + `title`? what about `$ref` resolution — do we hash the referenced schemas recursively?)

This affects cache keys, replay determinism, and audit trail integrity. Must be pinned at v1.

### 4.4 Three dialects in plan.v1 need to be closed.

`template.template: string` — Jinja syntax? Mustache? Handlebars? Custom subset? Used in `LmCompleteCallV1.messages[].content_template` and in `ExprV1::template`. Two implementations using different template engines silently diverge.

`extract.path: JsonPath` — referenced via `common.JsonPath` which is just `minLength: 1`. RFC 9535 JSONPath? jq-style? Closed subset? Affects every IR that extracts substructure from a let-binding.

`FieldRefV1.path: string` — used in every predicate and sort key. Dot-nested keys (`assessment.score.value`)? JSON Pointer (`/assessment/score/value`)? Closed grammar?

All three need closed dialect choices at v1, each with a small, testable grammar.

### 4.5 Aggregate USD budget shape.

Each capability grant has its own `max_usd_micro` in `limits` (lm: $250k, agent: $250k, sandbox: cheap, human: variable). No token-level `max_total_usd_micro`. A reflector with the example capability can spend the sum of all per-action caps before any limit triggers.

Options:
- Add token-level `max_total_usd_micro` that's a hard ceiling on the sum across all grants.
- Per-action caps with engine-side composition (default sum-bounded by the smallest cap).
- Both, with token-level overriding per-action.

This is non-controversial in principle but the schema needs the field.

### 4.6 Token expiry mid-call.

What happens when a token's `expires_at` passes while a long-running LM call or agent session is in flight? Options:

- Engine continues the call but blocks any *new* calls (graceful drain)
- Engine cancels the in-flight call immediately
- Token auto-renewal protocol (refresh tokens, but adds complexity)
- Capability says: "calls that started before expiry may complete"

The lock draft doesn't specify. For 20-minute capability windows (per the evaluator_capability example) and 5-minute agent sessions, this matters in real workloads.

### 4.7 DataClass propagation rules.

Lock spec §6.7: "Data classes propagate through templates, extracts, aggregation, and evidence envelopes." Good rule. But the IR doesn't define *how*. Specifically:

- A `template { lineage: $lineage, siblings: $siblings }` produces a string. Does the output carry the union of the input vars' data classes? The intersection? Only the labels that are public-safe?
- An `extract { input: $patch, path: "$.parsed.change" }` extracts a substructure. Does the extracted value inherit the input's labels? Or are some labels stripped (e.g., extracted summary fields lose `prompt.raw`)?
- An `aggregate` operation over a graph_set produces a single value. Same questions.
- `refs_from_result` produces InfoRefs from a query result. Do the resulting refs carry data classes derived from the projection?

Two implementations using different propagation rules silently diverge on capability checks. Must be pinned with explicit join/union rules.

### 4.8 Cross-run queries.

Sealed by default at v0. But "we want that to be a simple extension." The schema already has optional `run` field on every Ref, so cross-run *addressing* exists. Cross-run *capability* doesn't (capability grants have `resource.run` but not `resource.runs` consistently).

Options for v1.1 path:
- Add `RunResource.runs: [RunId]` and `RunResource.run_set: string` as alternate ways to scope a grant.
- Cross-run capability requires explicit operator approval (not delegatable from a normal subject).
- Cross-run reads are append-only (you can read other runs but not propose/assess into them).

For v1 we deny by default. The schema should reserve enough surface that v1.1 is a clean extension.

### 4.9 Watch.v1's place in v1.

The `watch.v1` schema is the most underdeveloped in the bundle (75 lines, missing delivery semantics, backpressure strategy, lifetime, method protocol). FlashEvolve-style async repair *does* need watches eventually. Three options:

- Ship watch.v1 as-is and refine in v1.x updates (risks locking in a thin design).
- Defer watch.v1 to v1.1 entirely; v1 has only pull-based diff queries via `consistency.since_revision`.
- Beef up watch.v1 significantly before lock — delivery semantics (`at_least_once | exactly_once`), backpressure strategy (`drop_oldest | pause | disconnect`), explicit lifetime (`until_cancel | until_stage_call_end | until_token_expiry`), method protocol (`watch.start | watch.next | watch.ack | watch.cancel`).

Recommendation: defer to v1.1. The current shape is insufficient to lock; the use cases (live dashboards, FlashEvolve repair) can be served by pull-based diffs in the interim.

### 4.10 LM API multimodal/streaming/tools surface.

`LmCompleteCallV1` bakes in OpenAI chat-completions shape (`messages: [{role: system|user|assistant|tool|developer, content}]`). What about:
- Responses-API-style providers (single message + tools as a separate primitive)?
- Single-message providers (Anthropic Claude has a different shape, though normalizable)?
- Multimodal content (image, audio, file inputs)? Currently only `content: string` and `content_template`. No image_url, no file_id, nothing structured.
- Tool/function calling at the LM call level? Currently no `tools` field. Tool results go in `role: "tool"` messages but `tool_call_id` field is missing on `LmMessageV1`.
- Streaming completions? Currently the call returns one response; streaming would need to be a separate variant or come through the watch protocol.

For v1, the minimum viable shape is text + tools + JSON-schema output. Multimodal and streaming can be deferred to extensions or v1.1.

### 4.11 The visibility enum naming inconsistency.

Three enums in the bundle for similar concepts but different names:
- `EvidenceProjectionV1.visibility` (plan.v1 line 1473-1481): `none | score_only | score_and_feedback | summary_only | public_only | full`
- `ReflectiveExampleV1.evidence_visibility` (stage_payloads.v1 line 358-364): `score_only | feedback | summary | full_public | redacted`
- `redaction_policy.optimizer` / `redaction_policy.reflector` (evidence_envelope.v1 line 92-109): `none | score_only | score_and_feedback | public_only | summary_only | full`

Three different vocabulary sets for "how much evidence does this consumer see." Pick one canonical enum in common.schema.json and reference everywhere.

### 4.12 The `judge` vs `preference` naming inconsistency.

`StageRoleV1` in `stage_payloads.v1` has `preference` (line 28). Earlier conversation and the lock spec reference `judge` in several places. Pick one canonical name. (Probably `judge` for the typed role; `preference` is the *output shape* of a judge stage. They're different concepts.)

### 4.13 The closed-vs-extensible Query enum.

The query language is closed at v1 (21 query kinds enumerated in `GraphReadGrant.constraints.queries`, 14 source kinds in `GraphSourceV1`, 9 step/edge kinds, 5 step types). New optimizer research will inevitably want new query kinds. Options:

- Closed forever; new queries require v2.
- Closed + namespace-extension (`x.<namespace>.<op>`) for adapter-registered queries.
- Closed core + "well-known extensions" registry that's separate from v2 schema bumps.

The bundle uses the closed + `x.*` extension namespace model. This is the right shape but needs explicit policy on what makes a query "well-known" enough to graduate into core.

## 5. The complete issue catalog

This is the line-by-line audit output. Grouped by schema/file, with severity inferred. Each issue is named so Oracle can pattern-match what we're asking about.

### 5.1 `common.schema.json` (the foundation)

**Confirmed strengths:**

- All IDs prefixed (`run_`, `cand_`, `prop_`, `pb_`, `assess_`, `evid_`, `case_`, `rset_`, `evalreq_`, `evalatt_`, `ws_`, `sc_`, `plan_`, `cap_`/`ltok_`) — visually scannable, ungrep-confusable.
- `EvaluationAttemptId` first-class (line 55-58). Promoted out of metadata as Oracle recommended.
- `ReceiptId` tagged-prefix union (`qrec | lmrec | agentrec | execrec | humanrec | wrec | caseread | wsread | effect | chargerec`) — receipt kind introspectable from the ID alone.
- `UsdMicro` as integer (10⁻⁶ USD) — no floating-point money.
- `MetadataBag.description`: "Core must not attach authorization meaning to unregistered keys." The exact rule we need, stated in-schema.
- `Redaction.reason` enum covers 10 leak vectors including `count_policy` and `path_denied`.
- All Ref types have optional `run` field — cross-run extensibility built in, default-current.
- `WorkspaceRef.snapshot_fingerprint` — refs point to specific snapshots, not "current state."
- `ExtensionObject` requires `namespace + op + schema_fingerprint + payload` — typed extensions with version pinning.
- `Cost` has `human_review_usd_micro` distinct from `usd_micro` — human labor money distinct from compute money.
- 16-class DataClass enum with `x.*` extension namespace is comprehensive.

**Issues:**

1. **`Score` lacks `output` field** (lines 384-422). New Rust contract requires `Score::with_output(...)`. See §4.2 for placement options.
2. **Fingerprint type aliases are documentation-only** (lines 91-105). `SchemaFingerprint`, `SurfaceFingerprint`, `PolicyFingerprint`, `CapabilityFingerprint`, `RuntimeFingerprint` all `$ref` to base `Fingerprint`. JSON Schema can't distinguish them at validation. Either structural disambiguation (different ID prefixes like `fp_pol_...` vs `fp_runtime_...`) or accept that fingerprint typing is convention-only.
3. **`JsonPath` field unspecified** (lines 114-117). `minLength: 1` is the only constraint. See §4.4.
4. **No `EvaluationAttemptRef`** despite `EvaluationAttemptId` being typed. Need a Ref type for citing attempts.
5. **`MetadataBag.additionalProperties: true`** without max bytes/depth cap (line 138-141). Metadata-bomb attack surface; should have ingress limits.

### 5.2 `leaven.capability.v1.schema.json` (the spine)

**Confirmed strengths:**

- Required top-level fields capture identity + audit trail (issuer, subject, audience, issued_at, expires_at, policy_fingerprint, capability_fingerprint, grants, delegation).
- `subject` is one of `stage_call | evaluation_stage_call | operator | service` — request-scoped by design (line 119-225).
- `execution_policy` separate from `grants` (line 226-293) — Leaven-side capability vs OS-side execution split is explicit.
- 14 grant kinds covering all action verbs (GraphRead, CaseRead, WorkspaceRead, WorkspaceMaterialize, SandboxExec, LmComplete, AgentRun, HumanReview, ProposalSubmit, ProposalApply, AssessmentSubmit, EvaluationRequest, Watch, Extension).
- `propose.submit` and `proposal.apply` are *separate* grants (line 952, line 1013) — submit does NOT imply apply.
- `target_egress` enum on `CaseReadGrant` (line 591-598) — target access and egress decoupled.
- `allowed_input_classes` + `forbidden_input_classes` on most call grants — defense-in-depth via both allow-list and deny-list.
- `max_concurrent` and `max_usd_micro` on every costful action (lm.complete, agent.run, sandbox.exec).
- `assessment.submit` request-scoped (line 1059-1070): bound to `evaluation_request_id`, `allowed_candidates`, `allowed_cases`, `evidence_visibility_allowed`.
- `delegation` with `may_delegate, max_depth, allowed_actions, must_attenuate, expires_with_parent` — macaroon-style.
- 5 execution profiles named (`trusted_local_operator | managed_sandbox | package_scorer | remote_untrusted | custom`).
- `revocation` field structurally present.

**Issues:**

6. **Aggregate USD budget at token level missing.** Each grant has `max_usd_micro`; no top-level `max_total_usd_micro`. See §4.5.
7. **`purpose` AND `purposes` on `LmCompleteGrant`** (line 801 + line 804) — singular and plural both present, ambiguous.
8. **`revocation` semantically empty** (line 79-91). `revocation_list: string` (URL? list-id?) and `revocation_epoch: integer` (logical clock?) — both unspecified beyond type. Need defined check protocol.
9. **`audience` is closed 5-element enum** (line 38-44) — `leaven.ir.eval | leaven.watch | leaven.worker | leaven.cli | leaven.admin`. Adding future audience targets requires schema bump. Consider open registered-string with documented set.
10. **`partitions` enum in `CaseReadGrant` mixes partition names and visibility labels** (line 574-581): `[train, validation, test, hidden, public]`. `hidden` and `public` aren't partition names; they're meta-categories. Clarify or remove.
11. **`stage_call_id` inconsistently required across subject variants** (line 126 vs line 170). Required for plain `stage_call`, optional for `evaluation_stage_call`. Should be consistent.
12. **`ExtensionGrant` lacks `forbidden_input_classes`** (line 1265-1267) — has `allowed_input_classes` only. Asymmetric with other grants.
13. **No `nonce`/`jti` field.** `grant_id` exists but isn't required unique-per-issuance. Replay-attack story not pinned.
14. **`subject` not independently fingerprinted.** Same logical principal across multiple tokens requires re-hashing to check.
15. **No `purpose` cross-validation rule.** A grant with `purpose: "runner"` + `fields: ["target"]` is a misconfiguration but JSON Schema can't catch it. Must be application-level validation at mint time.

### 5.3 `leaven.plan.v1.schema.json` (the IR)

**Confirmed strengths:**

- IR is genuinely value-shaped. Let/Call/Write triple is the architectural commitment.
- `consistency` has all three modes (`latest_at_start | at_revision | since_revision`).
- `mode` includes `replay` with explicit receipt references — honest replay-by-receipt.
- `commit` has `no_graph_writes | graph_writes_atomic | graph_writes_sequential` with `on_stale: reject | allow_if_policy_allows | rebase_extension` (the rebase extension hook for FlashEvolve-style staleness repair is present).
- Workspace materialize is a first-class Call variant (line 1944) returning a handle.
- `refs_from_result { from }` expression for provenance threading (line 498-507) — query results flow into proposal `informed_by` declaratively.
- Comprehensive: 14 ExprV1 kinds, 14 GraphSourceV1 kinds, 5 GraphStepV1, 4 GraphEdgeV1, 8 WorkspaceQueryV1, 6 CapabilityCallV1, 6 GraphWriteV1, 5 PreconditionV1.
- `preconditions` on writes (`candidate_exists | candidate_identity | graph_revision_equals | graph_revision_at_least | workspace_digest | extension_precondition`) — commit-time validation.
- `ProposalEffectWriteV1` has 3 variants including `change_from_workspace_diff` — agent-edits-workspace-then-submits-revset scenario.
- `informed_by` accepts both `InfoRef[]` (literal) and `ExprV1` (extracted from results) — flexible provenance.

**Issues (mostly mechanical):**

16. **Template syntax unspecified** (line 442, line 1803). See §4.4.
17. **JSONPath dialect unspecified** (line 467 references unspecified `JsonPath`). See §4.4.
18. **`FieldRefV1.path` syntax unspecified** (line 1586). See §4.4.
19. **`RequestEvaluationWriteV1.request: object additionalProperties: true`** (line 2540) — major write op with no schema. Reference `evaluation_job.v1` or pin inline.
20. **`tool_policy` on `AgentRunCallV1` open** (line 1869). Tool policies are a security boundary; should be typed.
21. **`provider_hints` and `sampling` on `LmCompleteCallV1` open** (lines 1750, 1732). Boundary between them undocumented.
22. **`selector` in `ArtifactProjectionV1` open** (line 1455). Adapter-defined; no engine validation hook.
23. **`payload` in `EmitRunEventWriteV1` open** (line 2582).
24. **`messages` bakes OpenAI chat shape** (line 1723). See §4.10.
25. **`LmMessageV1` missing `tool_call_id`** (line 1757-1795). Role-tool messages need this.
26. **No first-class `tools` field on `LmCompleteCallV1`** for function calling.
27. **`output: OutputContract`** (line 1730) accepts invalid combos for `lm_complete` (files, workspace_diff don't semantically fit).
28. **Double-wrapping in expression refs.** `expr: { kind: "graph_query", query: { source, steps, projection } }` — the intermediate `query` adds nothing. Same pattern in case_query (line 314) and workspace_query (line 330). Should flatten.
29. **Asymmetric `kind` disambiguation.** `kind: "candidate"` reused as CandidateRef ref-kind, GraphSourceV1 source-kind (line 559), ProjectionV1 projection-kind (line 1344). Three contexts, same string. Disambiguate (e.g., `kind: "by_candidate"` for source).
30. **No field-list projection.** Projections are mode-based; GraphQL-style field selection (`fields: ["id", "score", "feedback"]`) absent.
31. **No cursor/pagination.** `candidates { filter }` has no pagination story.
32. **No `descendants` edge.** Transitive children require compositional traversal.
33. **Predicate operators missing** `contains`, `matches`, `is_null`, `exists`.
34. **`PredicateV1.value` is `{}`** (line 1516). Type-mismatched comparisons aren't catchable at schema time.
35. **EvaluationSetExpr is strict subset of `leaven-core::EvaluationSet`** — missing Union, Intersect, Difference, Stratified, Sample.
36. **`read_file` content's data class not expressible at call site.** Only at grant level; can't narrow per-read for mixed-class workspaces.
37. **No `git_status` workspace query.** Just log + diff.
38. **`digest` hash algorithm unspecified** (line 1219). SHA-256? Blake3?
39. **No streaming for sandbox stdout/stderr.**
40. **Three ways to read case data** (`case.load { include: [target] }` vs `case.target { case }` vs `case.metadata { case }`). Document the visibility differences or unify.
41. **`SubmitProposalBatchWriteV1.semantics: sequence`** new semantic not documented (line 2062).
42. **Workspace materialize has no release call.** Lifetime implicit at stage call end.
43. **No `assessment_exists` precondition.**
44. **`ProposalEffectWriteV1` missing `ChangeFromAgentSession` variant.** Workspace-diff variant exists; agent-session-output variant doesn't.

### 5.4 `leaven.plan_result.v1.schema.json` (receipts)

**Confirmed strengths:**

- `OperationReceiptV1` has three shapes (`query | call | write`) with appropriate fingerprints, revisions, costs, trace_refs.
- `replayability` enum with 5 classes.
- Required: `capability_fingerprint, policy_fingerprint, base_revision, mode, values, receipts, redactions, charges, errors`.
- `Redaction` has typed reasons (10 categories including `count_policy`, `path_denied`, `secret`).
- Query receipt has both `op_hash` and `result_hash` — replay/cache primitives.

**Issues:**

45. **`replayability` at plan level, not assessment level** (line 71). Should be per-assessment with plan-level as roll-up.
46. **`graph_set.items` untyped `array`** (lines 100-101). Row shape unspecified.
47. **Five `kind` values share one schema variant** (lines 120-151): `case_record | workspace_snapshot | workspace_file | workspace_diff | workspace_listing` with `value: {}`. Should be typed variants.
48. **Same for write-receipt group** (lines 277-316): 5 receipt kinds share shape with `ids: object additionalProperties: true`.
49. **No `workspace_handle` value kind.** Materialize call result should be typed.
50. **`lm_response.message` and `parsed` are `{}`** (lines 179-180).
51. **`agent_session.parsed` is `{}`** (line 217).
52. **`sandbox_exec.stdout_ref`/`stderr_ref` bare strings** (lines 258-263). What kind of ref?
53. **`sandbox_exec.files: object additionalProperties: true`** (line 265).
54. **`call_kind` typed enum but `write.write` free string** (line 405 vs 457). Asymmetric.
55. **Call receipt has `request_hash` but no `result_hash`** (line 425). Asymmetric with query receipt.
56. **Write receipt's `preconditions: array items: {}`** (line 477-480). Should link to `PreconditionV1`.
57. **No timestamps on receipts.** `started_at`/`completed_at` missing.
58. **No error detail on failed call receipts.** Write has `reason: string`; call has nothing equivalent.
59. **`PlanErrorV1.code` is free string** (line 518). Should be closed v1 enum.
60. **`PlanErrorV1` doesn't reference `OperationReceiptV1`.** No receipt link for cross-correlation.
61. **`final_revision` optional.** Should default to base_revision when no writes; always be present.

### 5.5 `leaven.evidence_envelope.v1.schema.json` (leak prevention)

**Confirmed strengths:**

- Required: `schema_version, target_derived, public, redaction_policy`. Visibility-labeled by construction.
- `public` channel has typed common fields (`score, feedback, metrics, summary, trace_refs`).
- `private` channel exists with `visibility` enum and `payload`/`payload_ref` for inline vs by-ref.
- `redaction_policy` per-consumer: `optimizer | reflector | operator` with appropriate enums.

**Issues:**

62. **No `output` field.** Recent Rust contract requires `Score::with_output(...)`. See §4.2.
63. **`public.additionalProperties: true`** (line 53). Opens optimizer-visible channel to anything.
64. **`raw_prompt_logging` and `raw_transcript_logging` in `redaction_policy`** (lines 118-133). Belong to call results, not evidence.
65. **`source_receipts` doesn't distinguish read vs effect.** Should mirror `AssessmentWriteV1`'s split.
66. **`data_classes` is optional** (line 19). Should be required when `target_derived: true`.
67. **`target_derived: boolean` is binary.** Finer-grained taint tracking (use data-class set) might be more accurate.
68. **No `evaluator_id` / `evaluator_fingerprint`.** Producer identification missing.
69. **No `replay_class` on envelope.** If per-assessment replay class, natural place to attach.
70. **`redaction_policy.operator` has 3 values, others have 6.** Asymmetric (sensible — operators are most privileged — but should document).
71. **`private.payload: {}`** (line 67). Right for opaque, but `private.payload_schema_fingerprint` would enable downstream validation.

### 5.6 `leaven.evaluation_job.v1.schema.json` (evaluator entry)

**Confirmed strengths:**

- Required identity captures `run, evaluation_request_id, resolved_set, shape, granularity, purpose, base_revision, capability_fingerprint`.
- `evaluation_attempt_id` optional but typed.
- Three shapes (`independent | pairwise | listwise`) with appropriate candidate-list structures.

**Issues:**

72. **`resolved_set.case_ids` flat array.** Bloats for large case sets; pagination missing.
73. **`shape.pairwise.pairs` is array of 2-tuples.** Same bloat for large pairwise tournaments.
74. **No `evaluator_id` / `evaluator_fingerprint` at job level.**
75. **No `stage_call_id` direct field.**
76. **`granularity: "mixed"` unexplained.**
77. **No `deadline_at`.** Tied to capability `expires_at` implicitly.
78. **No `parent_job_id` for nested evaluations.**
79. **No `target_egress_policy` reference at job level.**

### 5.7 `leaven.stage_payloads.v1.schema.json` (role-specific payloads)

**Confirmed strengths:**

- `StageInvocationV1` outer envelope with required identity (`schema_version, role, run, stage_call_id, base_revision, payload, capability_fingerprint`).
- `RunCaseV1` (target-free) vs `TargetCaseV1` (target-aware) — explicit line-level separation.
- `TargetCaseV1.target_ref: InfoRef` for lazy fetch of large targets.
- `ScoreContextV1.case: oneOf [TargetCase, RunCase]` — scorer can be either depending on policy.
- `ReflectRequestV1` carries `examples, source_refs, attempt_index` matching existing Rust `ReflectRequest`.
- `ScoreContextV1.output: {}` (line 228) — output flows in on scorer input side.

**Issues (most important is #80):**

80. **No `ProposeRequestV1` distinct from `ReflectRequestV1`.** See §3.12 and §4.1. THIS IS THE MOST IMPORTANT ARCHITECTURAL GAP.
81. **`StagePayloadV1.oneOf` has 8 variants for 10 roles.** Proposer and operator unmapped.
82. **`ReflectiveExampleV1.evidence_visibility` enum differs from `EvidenceProjectionV1.visibility` enum.** See §4.11.
83. **`StageInvocationV1.run` is `RunRef`** (line 55), other schemas use `RunId` directly. Inconsistent.
84. **`evaluation_attempt_id` always optional.** Should be conditionally required for `role: "evaluator"`.
85. **Adapter payloads have `payload: {}`** (artifact, dataset, callback). Should require `payload_schema: SchemaFingerprint`.
86. **No `JudgeContextV1` distinct from `PreferenceContextV1`.** See §4.12.
87. **`PreferenceContextV1.outputs: array` without item type** (line 422-424).
88. **`TargetCaseV1.target_ref` lazy-fetch but `input`/`metadata` don't have `_ref` variants.** Asymmetric.
89. **`ReflectRequestV1.attempt_index` unbounded integer** (line 319-322). No maximum.
90. **No explicit target_safety declaration on `ReflectRequestV1`.** Author-intent comment, not schema validation.

### 5.8 `leaven.watch.v1.schema.json` (subscription protocol)

**Confirmed strengths:**

- Required: `schema_version, watch_id, source, projection`.
- `source` has `since_revision`/`cursor` for resumability.
- `filter: PredicateV1` reuses predicate language from plan.v1.

**Issues:**

91. **Only 2 source kinds** (`run_events | graph_changes`). Population/cost/failure/frontier source kinds absent.
92. **No delivery semantics declared.** `at_least_once | at_most_once | exactly_once` unspecified.
93. **No backpressure strategy.** Especially given ACP unbounded-mpsc issue, must be specified.
94. **`heartbeat_ms` in `limits`** (line 66). Misplaced — should be in flow/keepalive block.
95. **`since_revision` and `cursor` interaction undefined.**
96. **`projection` 4-option enum** (line 49-54). Conflicts naming with other projection enums.
97. **`extension_projection` at top level but `projection: "extension"` is trigger** — inconsistent shape.
98. **No `subscription_lifetime`.**
99. **No top-level `kind` discriminator.**
100. **No method-protocol coverage.** start/next/cancel/ack methods undefined.

### 5.9 `leaven.worker_protocol.v1.schema.json` (going away with ACP pivot)

This file is replaced by the ACP profile + MCP-over-ACP profile. Issues here are mostly noted for completeness; the file goes away.

101. **No cancellation primitive.** Only `status: "cancelled"` as a result.
102. **No streaming/progress messages.**
103. **No heartbeat/keepalive.**
104. **`payload: {}` unspecified** — no link to `stage_payloads.v1` based on `role`.
105. **`error: {}` unspecified** — no typed error model.
106. **No retry semantics.**
107. **No `query` message for in-stage graph queries.** Bidirectional flow missing.
108. **No `watch` messages.**
109. **`protocols` enum missing several schemas** (`plan_result`, `evidence_envelope`, `stage_payloads`, `watch`).
110. **Token-passing mechanism unspecified.**
111. **No protocol version negotiation.**
112. **No transport framing rules.**
113. **No graceful shutdown handshake.**

### 5.10 Example files

`evaluator_dspy_codex.py` — strong demonstration. Right shape; ~106 lines (under the 200-line target). Issues:

114. **`allow_input_classes` blanket grant on DSPy context** (line 37). Permissive; per-call narrowing safer.
115. **Workspace ops have no `input_classes` declaration.** Resolved by grant-level declaration on `WorkspaceReadGrant.data_classes`, but not visible at call site.
116. **`asyncio.gather` of 4 calls** (line 65). Works but engine doesn't see DAG. Should document `cx.batch()` form for engine-visible concurrency.
117. **Workspace cleanup implicit.** Lifetime scoped to stage call.
118. **`pytest` variable shadows package** (line 65).
119. **`JudgeResult(**dspy_pred.to_dict())`** fragile (line 76). Should use `model_validate`.

`reflector_plan.example.json` — confirms IR-as-value shape. Issues:

120. **Verbose nesting**: `expr.graph_query.query.source.candidate.candidate.id` (lines 15-23). Intermediate `query` adds nothing.
121. **`{ "kind": "candidate", "id": "cand_123" }` repetition.** JSON shorthand allowing `"candidate": "cand_123"` would shrink real plans.
122. **`content_template` syntax unspecified** (line 107-119).
123. **`change: { kind: "extract", input: { kind: "var" }, path: "$.parsed.change" }`** JSONPath dialect unspecified (line 157-164).
124. **Engine must verify input-class flow** from var sources to call accept-classes.

`evaluator_capability.example.json` — sharp demonstration. Issues:

125. **No aggregate USD budget across actions.** Each grant has its own; no token-level total cap.
126. **Token signing/verification mechanism not specified** (belongs in worker protocol or ACP profile).
127. **Token expiry-while-running has no rotation protocol.** See §4.6.

### 5.11 `leaven_public_seam_v1_lock_spec.md` (prose)

**Confirmed strengths:**

- §0 "The judgment this spec carries" is exactly the transferable-judgment artifact requested. 10 commitments stated as one-liners.
- §14 Security invariants: 12 "Never" rules.
- §20 Anti-patterns: 13 do-nots.
- §15 Replayability classes: 5 classes documented.
- §6.7 enumerates all 16 data classes.
- §11 covers LM/agent/sandbox/human calls with shared receipt semantics.
- §17 Versioning sets v1-is-stable contract; "Core unknown fields are rejected"; "Additive extension fields belong in metadata or registered extension payloads."
- Line-by-line MUST/SHOULD/MAY discipline.

**Issues:**

128. **§7.1 Reflector payload section doesn't establish reflection-vs-proposal as structural rule.** No parallel §7.X for Proposer. See §3.12.
129. **§3 schema fingerprint computation undefined** (line 245). See §4.3.
130. **§7.5 conflates judge and preference roles.** See §4.12.
131. **§14 Security invariants missing some "Never" rules:**
    - "Never let `kind` discriminators collide across semantic contexts."
    - "Never let token expiration mid-call result in silent denial."
    - "Never let public schema `additionalProperties: true` open optimizer-visible channels by accident."
132. **§16 Cost mentions per-action budget; aggregate USD cap missing.**
133. **§2 Transport model is 21 lines.** Doesn't define framing, cancellation, streaming, heartbeat. With ACP pivot, this section needs full rewrite.
134. **§13 Extensions says "No extension may bypass capability checks"** but `ExtensionObject` doesn't have a field declaring which capabilities it requires.
135. **§6.7 says data classes propagate through templates/extracts/aggregation/evidence envelopes**, but no explicit join/union rules. See §4.7.
136. **§4.2 Consistency doesn't address what happens when `since_revision` crosses a graph compaction event** (if Leaven ever compacts old revisions). Probably v2 concern.
137. **§18 Implementation checklist doesn't mention ACP pivot or MCP-over-ACP for LM callbacks.**
138. **§5.7 Predicates sparse.** Doesn't enumerate operators in prose; should mirror plan.v1.
139. **§15 Replayability says "Managed evaluator code should target boundary_managed or better"** — should be MUST for `managed_sandbox` profile.
140. **No section on watch protocol beyond §12** (5 sentences). Given watch.v1 is the most underdeveloped schema, prose needs either to pin it better or defer to v1.1 explicitly.

## 6. The ACP pivot — what it changes

This is significant enough to call out explicitly. The current `leaven.worker_protocol.v1.schema.json` is replaced by **two profile documents** (not new schemas; profiles over existing protocols).

### 6.1 Leaven ACP profile

Document title: "Leaven Agent Client Protocol Profile v1." Pins:

- ACP version: v1 (v2 just landed; track but don't ship against).
- Transport bindings: stdio (canonical), Unix socket, HTTPS.
- Authentication: capability-token via ACP `authenticate` method; engine verifies token against persisted grant document fingerprint.
- Session lifecycle: `session/new` per stage call, `session/cancel` for cancellation, `session/load` for resume.
- Streaming: `session/update` for progress notifications; bounded mpsc on the Leaven side (NOT the reference SDK's unbounded mpsc — that's an integration responsibility).
- Permission flow: `session/request_permission` answered programmatically against capability token grants; no UI prompts; deny carries typed `Redaction` reason.
- Leaven extension methods: `leaven/graph.query`, `leaven/case.load`, `leaven/case.input`, `leaven/case.target`, `leaven/case.metadata`, `leaven/workspace.snapshot`, `leaven/workspace.list`, `leaven/workspace.read_file`, `leaven/workspace.stat`, `leaven/workspace.digest`, `leaven/workspace.git_log`, `leaven/workspace.git_diff`, `leaven/workspace.capture_artifacts`, `leaven/workspace.materialize`, `leaven/proposal.submit_batch`, `leaven/proposal.apply`, `leaven/assessment.submit`, `leaven/evaluation.request`, `leaven/event.emit`, `leaven/watch.start`, `leaven/watch.cancel`.
- Data-class labels travel in `_meta` field on JSON-RPC params (ACP's extensibility mechanism); engine validates labels against capability grants on every method.
- Receipts return in JSON-RPC results, alongside the operation's primary result.

### 6.2 Leaven MCP-over-ACP profile

Document title: "Leaven Model Context Protocol over ACP Profile v1." Pins:

- MCP version (TBD; track current).
- Transport: MCP messages tunneled over the ACP socket established by the worker; engine acts as MCP server, worker as MCP client.
- Tools advertised: `lm.complete`, `sandbox.exec`, `agent.run`, `human.review`.
- Tool calls authorized by the same capability token that authenticated the ACP session.
- Sampling primitive: MCP's `sampling/create_message` is the wire for `lm.complete` callbacks. Engine handles cache, budget, replay, provider policy, telemetry behind the sampling call.
- Tool-call results carry effect receipts in `_meta`.

### 6.3 What stays unchanged

The capability schema, plan schema, plan_result schema, evidence_envelope schema, evaluation_job schema, stage_payloads schema all layer cleanly on top of the ACP profile + MCP-over-ACP profile. No schema modifications needed for the pivot. Only:

- Delete `leaven.worker_protocol.v1.schema.json`.
- Write the two profile documents.
- Update lock spec §2 Transport model to reference the profiles.
- Update lock spec §18 Implementation checklist with ACP integration tasks (bounded channels, capability-token-to-permission-flow mapping, MCP-over-ACP wiring).

### 6.4 Concerns with the pivot

- **ACP is moving fast.** v2 just landed; multiple RFDs in flight (HTTP/WebSocket transport, session-fork, MCP-over-ACP, custom-LLM-endpoint, elicitation). Pegging to v1 + named capabilities is right but means tracking the spec.
- **Backpressure is our problem.** Reference Rust SDK uses unbounded mpsc; high-volume `session/update` traffic can OOM the worker. Leaven worker shims need bounded mpsc and/or credit-based flow control.
- **`request_permission` is semantically tool-call-shaped.** We use it for general capability checks; ride `_meta` to encode non-tool actions. Watch the elicitation RFD as possibly cleaner.
- **MCP-over-ACP is v2-unstable.** v1 doesn't have it natively; we'd be early adopters. Alternative: run a separate MCP server process spawned by the engine, worker connects to both ACP and MCP independently. More complex but works on v1.
- **Naming friction.** ACP's mental model is "IDE-client + coding-agent"; we invert to "engine-client + worker-agent." JSON-RPC roles map cleanly but documentation must call out the inversion.

## 7. What we are NOT changing

These are positions we explicitly defend against pressure. Don't argue them away without a concrete demonstrably better alternative.

7.1. **The IR is a value, not a call.** Non-negotiable. RPC dispatch as the IR breaks every property downstream.

7.2. **Closed data-class enum at v1.** 16 classes + `x.*` extension namespace. Open free-string would be policy hell.

7.3. **Closed query language at v1.** 21 query kinds, 14 sources, 9 edges, 5 steps. Open string queries lose type safety and policy enforcement.

7.4. **Closed predicate language at v1.** 6 comparison ops + in/and/or/not. No Mongo-style operator dicts.

7.5. **Capability token + grant document, separated.** Bearer is opaque; grant is structured. Persisted by fingerprint, never bearer secret.

7.6. **Request-scoped evaluator privilege.** Evaluator tokens never root tokens.

7.7. **Target access ≠ target egress.** Two separate grants on two separate axes (read scope vs allowed_input_classes per call).

7.8. **`RunContext` as sole graph-mutation authority.** No exceptions, no public direct-mutation APIs.

7.9. **Visibility-labeled evidence envelope structurally required.** Public/private channels are not optional convenience — they're the leak-prevention primitive.

7.10. **Replay-by-receipt, not by determinism wishful thinking.** LM/agent/sandbox calls are non-deterministic by nature; replay reproduces by receipt only.

7.11. **One IR family, three semantic node classes.** Read/Call/Write have different semantics (determinism, cost, atomicity) but live in one value language.

7.12. **Adapter namespaces own artifact semantics.** Core knows refs/revisions/fingerprints/projections; doesn't know DSPy/SkillBank/Inspect/git internals.

7.13. **ACP profile + MCP-over-ACP, not invented worker protocol.** Worker = agent-shaped. Use the standard.

7.14. **Reflection ≠ proposal.** Structural separation, not collapsed.

7.15. **Trust profiles bundle execution + capability defaults.** Project-level profile declaration; no runtime UI prompts.

## 8. What we want from Oracle this pass

Pick a position on each open architectural question (§4.1 through §4.13). Don't hedge. Don't list trade-offs without choosing. We want your call with the reasoning.

For each must-fix in §5 (the issue catalog), confirm whether you agree it's a fix, suggest the concrete patch, and flag if you'd handle it differently than the categorization suggests.

For the ACP pivot (§6), confirm or push back. If you see a better transport story than ACP + MCP-over-ACP, name it concretely. If you agree, suggest what the two profile documents should look like (structure, key sections, what to pin vs leave to integration).

For anything we missed: what's not in this document that would matter? Where are we wrong in a way we couldn't see because we built up our position incrementally? Where would Leaven 6 months from now wish we'd thought harder?

For prior art: if there's a well-designed IR, capability model, evidence envelope, watch protocol, or trust profile system somewhere we should look at and adapt — name it explicitly. We'd rather adopt something good that exists than invent something worse.

## 9. What we want for the next steps

After this pass:

9.1. **Write the Leaven ACP profile document.** ~3-5 pages, normative. Pins ACP v1, names all Leaven extension methods, defines auth flow, permission-token translation, data-class labeling on `_meta`, receipt return shape. Replaces `leaven.worker_protocol.v1.schema.json`.

9.2. **Write the Leaven MCP-over-ACP profile document.** ~2-3 pages, normative. Pins MCP version, defines the tool surface engine exposes, sampling-as-LM-dispatch wiring, capability-token-gates-each-tool-call rule. Either coexists with ACP on one socket (v2) or runs as separate MCP server connection (v1 fallback).

9.3. **Apply the must-fixes.** ProposeRequestV1 added, Score.output placed, hidden-partition leak guarded at engine ingress (cite from §5 issue numbers).

9.4. **Pin the three dialects** (JSONPath, template, field-path) and the schema fingerprint algorithm.

9.5. **Close `EvidenceEnvelopeV1.public.additionalProperties`** and tighten other `{}` opens.

9.6. **Resolve the visibility enum naming inconsistencies.** One canonical visibility enum in common.schema.json.

9.7. **Update lock spec prose** to incorporate Oracle's positions on the open architectural questions.

9.8. **Then lock v1.** Additive extension fields only, no field removals without major bump, schema fingerprints stable.

## 10. The arc of judgment we want preserved

If a future model or contributor reads only one section, this is it. The architectural judgments compressed to their irreducible form.

10.1. **Value > Call.** The IR is a typed value. Build it, hash it, hold it, modify it, serialize it through any transport, execute it lazily. Composition lives in the IR. The engine sees the whole shape. The same value works over CLI flags or JSON-RPC over stdio or HTTPS or in-process.

10.2. **Tokens are the spine.** Every operation across every transport checks the capability token at one place. Tokens are bearer secrets; grants are structured documents; persistence is by fingerprint. Macaroon-style attenuation for delegation. Cedar-shaped action/resource/constraints for grant content.

10.3. **Boundaries managed; interior free.** Worker code in any language; LM/agent/workspace/proposal/assessment go through Leaven portals; loops/conditionals/parsers/helpers stay host-language.

10.4. **Privilege is request-scoped; access ≠ egress.** Evaluators are powerful but bound to one evaluation request. Reading the target is one grant; sending target-derived data to an external LM is another. Data-class taint labels structurally enforce.

10.5. **Visibility is in results, not just policy.** Every read produces a receipt with revision/projection/redactions. Every effect produces a receipt with cost/runtime/replay class. Evidence has public/private channels.

10.6. **Determinism is honest.** Pure reads at a revision are deterministic. Fresh LM/agent calls are not. Replay is by receipt, not by wishful determinism.

10.7. **ACP for transport; we don't invent.** Worker = agent-shaped. ACP + MCP-over-ACP for LM dispatch. Capability tokens translate into programmatic permission answers; visibility labels ride `_meta`.

10.8. **Reflection and proposal are different stages.** LMs do one thing well. The schemas must enforce the boundary structurally.

10.9. **Replay class per assessment, not per run.** Most evaluators are `boundary_managed`. Trusted research code can opt down explicitly. Package scorers can't go below `boundary_managed`.

10.10. **Adapters own artifact semantics.** Core knows surface fingerprints and schema fingerprints; doesn't know DSPy/SkillBank/git internals.

10.11. **Lock the wire; evolve the SDK.** The schemas are forever once published. SDKs, CLIs, prose can evolve under them. v1 schemas frozen; additive extensions in `metadata` or registered extension payloads; new core variants require new minor schema package or `x.<namespace>` extension; field removals or semantic changes require new major version.

10.12. **Errors are typed, replayable, and audited.** Free-string error codes don't compose. Receipts and redactions are the audit trail. Failed calls that spent money still produce charge receipts.

## 11. Risks and watchitems

Things to monitor over the v1.x lifetime that could indicate we got something wrong.

11.1. **ACP version drift.** ACP v2 just landed; RFDs moving. Track v1 stability; defer to v2 only after multiple downstream implementations have adopted it stably.

11.2. **Capability grant complexity.** 14 grant kinds plus constraints plus delegation plus execution policy. Watch for "evaluators write more capability JSON than evaluator code" — if grants become bigger than evaluators, the abstraction is wrong.

11.3. **DSPy adapter friction.** If `LeavenDSPyLM` requires DSPy users to change their code significantly, adoption stalls. The adapter must be drop-in via `dspy.configure(lm=LeavenDSPyLM(cx))`.

11.4. **Workspace materialization performance.** Materializing full repos for every candidate evaluation may not scale; need lazy/copy-on-write workspaces, snapshot reuse, materialization caching.

11.5. **Data-class propagation correctness.** This is the most security-critical engine subsystem. Bugs here = silent target leaks. Need extensive test coverage of every join/extract/aggregate scenario.

11.6. **Receipt volume.** Every operation produces a receipt. For evaluators doing thousands of operations across hundreds of cases, receipt accumulation can dominate plan result size. Watch for "receipts are 99% of bytes."

11.7. **Extension namespace fragmentation.** `x.dspy`, `x.skill_bank`, `x.inspect`, `x.git`, `x.theorem_prover`, `x.browser`, `x.multimodal`, `x.fluence`, `x.evopro` — if every paper adds an `x.*` namespace, core becomes a kernel and adapters become the actual product. Probably fine but watch the shape.

11.8. **Trust profile sprawl.** Five profiles at v1. If we add `trusted_remote`, `package_evaluator`, `hosted_scorer`, `local_research`, `replay_only`, etc. — the profile system loses its "named bundles" advantage and becomes a custom-policy system. Cap at ~7 profiles; everything else is `custom`.

11.9. **Schema fingerprint drift.** If the canonical-JSON algorithm or hash function changes between v1 implementations, all replay breaks. Pin hard at v1.

11.10. **Watch protocol underspecification.** If we ship watch.v1 as-is and someone builds against it, the gaps (delivery, backpressure, lifetime) become forever-compatible. Defer or beef up.

## 12. Closing — the discipline this document encodes

We built this design over many turns. We pushed back on positions that seemed easy but compromised the principles. We brought in research (ACP, MCP, FlashEvolve, prior IRs from Polars/LINQ/Cypher/Substrait/Cedar/macaroons/Zanzibar) when it helped. We trusted that Oracle and other strong models could bring solutions we hadn't seen — and they did (data-class taint labels, replay-by-receipt correction, trust profiles as bundles, the macaroon-style attenuation framing, the read_receipts-vs-informed_by separation).

The lock draft is *close*. The issues catalogued here are real but mostly mechanical or scoped. The architectural shape is right. The remaining work is: pick positions on the open questions, apply the must-fixes, write the two ACP profile docs, lock v1.

This is the public contract that makes Leaven usable as a general-purpose optimizer from any language. Get it right and every future integration — Python SDK, TS SDK, MCP adapters, remote deployment, multi-tenant cloud, IDE integrations, replay tooling — falls out cleanly. Get it wrong and we rewrite it in a year with users on the public wire and breaking changes everywhere.

Design it like it's forever. Because it is.
