# Make-It-Real: Propagation Plan for the Redesigned Leaven Python SDK

Status: synthesis note (working memory). Date: 2026-06-01.
Subordinate to `docs/specs/leaven_python.md` (governing Python product spec) and
the locked `docs/specs/public-seam-v1/` (governing wire). Where this note and a
spec disagree, the spec wins. This is a path-to-real, not a contract; durable
decisions land in specs/schemas/tests as the slices below complete.

**Decisions locked 2026-06-01 (user):** `leaven/stage.run` is ONE generic method
(not per-role). Example-03's LM is a deterministic MOCK host `Lm` (live is a later
opt-in) — and the first real run MUST actually execute with it, not merely
compile. The `Score.rewards[]` reward-vector schema is deferred until AFTER the
first real run (example 03 uses one scalar exact-match reward). `objective ∈
{hybrid, cartesian}` is validate-only in V1. Implementation proceeds as a
verification-gated pipeline: slice 1 (bidirectional `leaven-acp`) → slice 2
(`leaven/stage.run`) → slice 3 (example 03 real).

## 0. Frame: what p9 proves, and the exact gap

`examples/p9_python_acp_gepa_codex` is a **live mechanics smoke**, not the SDK.
Verified against ground truth (`src/main.rs`, `worker/p9_worker.py`):

PROVEN by p9 + the locked profile (reuse as-is, do not rebuild):

- **Live Codex** through `CodexCliRuntime` / `leaven-agent-codex-cli`
  (`main.rs:9, 93`).
- **The `leaven-acp` stdio transport seam.** `AcpStdioProcessSession::spawn`
  launches a `python3` worker and exchanges ACP JSON-RPC with a capability
  token + fingerprint injected at launch
  (`stdio.rs:108-166`; env `LEAVEN_CAPABILITY_TOKEN` / `LEAVEN_ENDPOINT` /
  `LEAVEN_CAPABILITY_FINGERPRINT`, never persisted, `session.rs:230-260`).
- **Public-seam validation of the locked profile** — the 25 `leaven/*`
  extension methods in `locked_profile_methods()` (`main.rs:409-437`), each
  `params=leaven.plan.v1` / `result=leaven.plan_result.v1` / `produces_receipt`
  / a `required_action` capability path. The full method vocabulary the SDK
  needs already validates in both directions (the validators are
  direction-agnostic, `package.rs:391-418`).
- **A durable tiny GEPA-shaped accept** (seed + one child, `result_summary`-shaped
  state, `main.rs:103-121`).

NOT proven by p9 (this is the entire build):

- The worker (`p9_worker.py`, ~50 lines) **computes nothing** — it replays a
  host-supplied `LEAVEN_P9_RESPONSE_MAP` (`p9_worker.py:8, 36-45`) and only
  emits `session/update` lifecycle notifications (`p9_worker.py:21-34`).
- The ACP exchange is **one-directional**: the host calls extension methods
  *on* the replay worker; `call_extension` has no inbound-request branch
  (`stdio.rs:208-223` loops on `handle_session_update`, else validates the line
  as the response). A worker that calls `leaven/lm.complete` *back* is
  structurally unrepresentable today.
- The GEPA is a hardcoded seed+child with a tiny local scorer, not a loop.
- p9's `AGENTS.md` disclaims "a real Python SDK implementation" and "full GEPA
  optimizer policy."

**The keystone correction (resolves the goal's "open gap" framing):** the
profile spec already *mandates* the inverse of what p9 ran. "The engine is the
ACP client. The worker is the ACP agent… Leaven extension methods cover the full
worker callback surface" (`profiles/leaven_acp_profile_v1_v0.3.md:7-9, 43, 61`;
`00_architecture_judgment_v0.3.md:35`). So the 25 `leaven/*` methods are meant
to be **worker→host callbacks** (the worker runs the stage and calls
`leaven/lm.complete` BACK into the engine). p9 drove them backwards as a
transport smoke. The bidirectional callback is therefore not a new vocabulary —
it is the **locked profile's primary direction, never exercised round-trip**.

What is genuinely missing decomposes into four things, in dependency order:

1. **A bidirectional ACP session loop** in `leaven-acp` (worker→host effect
   callbacks; the inverse-request branch p9 lacks). A proven local template
   exists: `leaven-agent-codex-app-server` `client.rs:184-217` /
   `transport.rs:111-195`.
2. **A stage-dispatch leg** (host→worker "run this stage now"): the 25 methods
   are all effect/query/mutation callbacks — none is "execute stage N." This is
   a genuine **wire gap**, not just transport.
3. **The real Python SDK** riding both legs: `serve_stage`, the role-scoped `cx`
   effect builders, `optimize().run()` driving a real (tiny) GEPA loop.
4. **Wire extensions for the reward vector** — `Rubric` is a weighted reward
   VECTOR and `RewardValue(value, feedback, output)` is per-reward, but the
   locked wire `Score` is a scalar (`value` + flat `metrics:{str→number}` +
   one `output`, **no `reward`/`feedback`/`objective` vocabulary**,
   `common.schema.json:742-768`).

---

## 1. PROPAGATION MAP

Each new-surface noun → concrete change at the wire-schema layer
(`leaven-public-seam` / `docs/specs/public-seam-v1/schemas`), the transport
layer (`leaven-acp`), and the engine/optimizer layer (`leaven-engine` /
`leaven-run` / `leaven-gepa` / evidence). The last column marks what p9 + the
locked profile **already prove** so the build does not redo it.

| New-surface noun | Wire schema change | `leaven-acp` (transport) change | Engine / optimizer change | Already proven by p9 + locked profile |
|---|---|---|---|---|
| **`Environment(task, rollout, rubric)`** | None — correctly host-side sugar; decomposes onto existing legs (`case.*` reads, `RunnerRequest`, assessment submission). Verify the host lowering decomposes rather than inventing an `Environment` envelope. | None. | Reuse `ScoringEvaluator` (`leaven-run/src/evaluator.rs:81-291`) which already lowers `(cases, runner, scorer)`→per-case `Assessment::Independent`. `task`→case inputs, `rollout`→runner, `rubric`→scorer. The only new work is the rubric *fan-out* (see Rubric row) and the role-scoped `cx`. | Case-set + runner/scorer lowering exists in `leaven-run`; `evaluation.request` + assessment machinery validates on the wire. |
| **`Rubric([@reward…])` = weighted reward VECTOR** | **NEW.** Add typed `rewards[]` to `common.schema.json#/$defs/Score` (id / value / weight / is_objective / feedback / output). Keep scalar `value` as the weighted-aggregate roll-up. Add `objective` + `objective_dims` to `RequestEvaluationWrite.request` (`plan.v1:2772`) so the frontier axis travels with the evaluation request. | Payload-agnostic; carries the extended `Score`. The validators it calls will reject any reward-vector not in the locked schema, so the schema change gates the transport. | **New evidence shape** (`ObjectiveVectorEvidence` or extend `CaseAssessmentEvidence`) holding `BTreeMap<ObjectiveId, RewardValue>` in `leaven-evidence`. Scorer closure fans out over N reward fns and assembles the vector. The wire projection (`leaven-run/.../projection/output.rs:82`) emits `rewards[]` instead of one `value`. | `evaluation.request` method + flat `metrics:{str→number}` + one `output` per `Score` validate today (`common.schema.json:748-756`). The *scalar* path is proven; the vector is new. |
| **`objective=` ∈ {instance, objective, hybrid, cartesian}** | Covered by `objective`+`objective_dims` on `RequestEvaluationWrite` above. | None. | `instance` = existing per-case `ParetoFrontier` (`leaven-population/src/pareto_frontier.rs:53-240`), reuse verbatim. `objective`/`hybrid`/`cartesian` = generalize `ParetoFrontier<Axis>` over the axis key type (the dominance algorithm at `:204-228` is axis-agnostic; only the key changes). GEPA's strict-improvement gate stays scalar via the weighted-sum roll-up (`assessment.rs:160-168`, `step.rs:263-265`). | The case-axis Pareto frontier exists and runs (`pareto_frontier.rs`). It IS `objective="instance"`. The other three axes are new. |
| **Role contexts** (`RolloutContext` / `RubricContext` / `ReflectContext` / `ProposeContext` / `JudgeContext` / `EvaluatorContext`) | Map to `Subject.role` enum (`capability.v1:146-157`) + stage payloads (`stage_payloads.v1`). One open: `JudgeContext` target visibility is unpinned (`stage_payloads.v1:447-456` has open `rubric:{}`, no target-access flag). **NEW (small):** add required `target_visibility: {enum:[target_free, target_under_policy]}` to `JudgeContext`. | The transport gates inbound methods through `profile.method(...)` so role scope = which methods the capability token grants (mirror the private/MCP rejection at `tests:693-736` in the reverse direction). | Role scoping is structural: runner `cx` has no `case.target` path, reflector/proposer get typed requests not `cx.case`. The engine already carries `Subject.role`; the projection just respects it. | `Subject.role` enum + the five stage payload roles (Reflector/Proposer/Runner/Scorer/Judge) validate (`stage_payload.rs:350-368`). Role-scoped capability tokens are minted today (p9). |
| **cx effect builders** (`cx.lm` / `cx.agent` / `cx.sandbox` / `cx.workspace` / `cx.case` / `cx.proposals` / `cx.assessments` / `cx.batch`) | None for `lm`/`agent`/`workspace.read*`/`case.*`/`proposal.submit`/`assessment.submit` — methods exist in `locked_profile_methods()`. **NEW (small):** `leaven/workspace.write_file` if proposers write directly (table has materialize/read/release, no first-class write). `sandbox.exec` is wire-locked but **engine lowering is unbuilt**. | **THE CORE TRANSPORT CHANGE.** Each `cx.*` call is a worker→host inbound JSON-RPC request the engine must service mid-stage. Add the inbound-request branch + a host effect-handler trait (`AcpEffectHost`) that validates inbound `params` (`validate_acp_jsonrpc_request_document`, exists), dispatches to the host, validates the result (`validate_acp_extension_result_document`, exists), writes it back with the worker's `id`. | Host effect handler bridges each method into `RunContext` (the existing `PlanExecutionHost` trait `plan_execution.rs:126-231` has exactly this vocabulary: `lm_complete`, `agent_run`, `sandbox_exec`, `workspace_materialize`, `case_query_load`, `graph_query`, `emit_run_event`). Must route writes through the finalizers `RunContext::propose`/`evaluate`, not the raw contexts. | The method vocabulary + receipt/cost/capability-fingerprint envelope machinery (`leaven-public-seam/.../receipts`, `result`) is proven; p9's host pre-bakes these envelopes. The transport carrying them is proven. **Only the inbound direction is new.** |
| **`RewardValue(value, feedback, output)`** | Subsumed by the `Score.rewards[]` change: each array item carries `value` + `feedback` + `output`. `OutputRecord` is already a real wire type (`common.schema.json:700-741`, kinds text/json/blob_ref/structured/agent_session/workspace_diff). No NEW type beyond the array. | None (carried in the extended `Score`). | **Mostly already works for the scalar.** `CaseAssessmentEvidence` (`leaven-evidence/src/feedback.rs:72-161`) is structurally `RewardValue(value, feedback, output)` and already flows `output→produced`, `feedback→feedback`, `score→score` into the reflective dataset (`reflection/dataset.rs:224-237`). For the vector, per-objective feedback/output populate `ReflectiveRun.side_info` (`types.rs:81`, channel exists) — localized new `ReflectionProjection` impl, no new plumbing. | `Score.output` (required) + single feedback-via-evidence flow validate. The reflective dataset rich-output channel exists. Per-reward cardinality is the only gap. |
| **splits** (`Case.split` / `CaseSplits` train/val/test) | None. Split tag rides `Case.metadata`→`MetadataBag` (`common.schema.json:307-311`); resolved membership → `evaluation_job.resolved_set` with `partition_summary` + `purpose:{train\|validation\|test\|diagnostic\|custom}` (`evaluation_job.v1:138-175`). Doc note only (split string → metadata key convention). | None. | Reuse: `leaven-run` already lowers train/val/test inputs → `DatasetSplits` → `CaseSet{TRAIN,VALIDATION,TEST}`. Hidden-split defaults governed by `case_visibility_and_target_isolation.md`. | Split/purpose enum + `partition_summary` validate; GEPA already resolves TRAIN/VALIDATION partitions through `RunContext` (`step.rs:103-121`). |
| **stage dispatch** (host→worker "run stage N") — *implied by every role context* | **NEW.** No locked method dispatches a stage. Add a `leaven/stage.run` extension method (params: stage kind + role-scoped payload `RunnerRequest`/`ScoreContext`/`ReflectRequest`/`ProposeRequest`/`JudgeContext`; result: the stage's typed output incl. the reward vector). New schema + profile row + matrix/conformance in `leaven-public-seam`. | Drive host→worker through the *existing* `call_extension` direction; the bidirectional loop services the worker's effect callbacks during the call. | A Rust `Proposer`/`Evaluator`-shaped adapter (parallel to `leaven-gepa-agentic-git`'s in-process reflector) projects the stage request to `stage_payloads.v1` (projection exists `public_seam_stage.rs:73-219` for reflect/propose; generalize to runner/scorer/judge), sends it over ACP, services callbacks against `RunContext`, parses the worker's result. Lives in a **new `leaven-acp`-depending bridge crate** (workspace currently has zero), not in `leaven-engine` (cold of ACP) or `leaven-acp` (must not own graph mutation). | The stage-payload schemas + the reflect→propose wire projection exist and validate. The *dispatch method* binding them host→worker, and the round-trip, are new. |

---

## 2. BIG DECISIONS

### Decision A — Multi-objective reward vector: first-class on the wire, scalarized for V1 selection

**This is already half-decided and must not be relitigated.** The resolved-verdicts
ledger D5 (`2026-06-01-public-api-coherence-and-open-decisions.md:117-122`) locks:
*"`Rubric` carries the reward vector… the optimizer declares the reduction via
`objective=`; V1 ships `weighted` (scalarize for selection), `pareto` reserved
for true reward-dimension multi-objective later. The contract (vector in,
reduction declared) is locked now so it can't be designed away."*

The trap the scouts independently flagged: the **wire `Score` IS the early scalar
collapse the Python `Rubric` docstring says must never happen**
(`rubric.py:5-7` vs `common.schema.json:742-768`). If V1 ships only the scalar
wire, `objective ∈ {objective, hybrid, cartesian}` becomes *silently
inexpressible* and the locked contract is violated at the seam regardless of the
Python type.

**Recommendation (carries vector, reduces for selection):**

- **Carry the full vector on the wire now** even though V1 selection scalarizes.
  Add typed `Score.rewards[]` (id/value/weight/is_objective/feedback/output) +
  `objective`/`objective_dims` on `RequestEvaluationWrite`. This is cheap, it
  honors the locked D5 contract, and it means turning on real reward-dimension
  Pareto later is an *engine* change, not a *wire* change (no second schema
  migration, no resume-fingerprint break).
- **Scalarize at the gate, not at the seam.** GEPA's strict-improvement gate
  consumes the weighted-sum roll-up (`Score.value`) — that keeps
  `assessment.rs:160-168` / `step.rs:263-265` working unchanged. `objective=`
  selects the *frontier axis*: `instance` reuses the existing case-axis
  `ParetoFrontier`; the other three generalize `ParetoFrontier<Axis>` over the
  key type (same dominance code). V1 can implement `instance` (proven) +
  `weighted` reduction, and *validate* `objective`/`hybrid`/`cartesian` payloads
  end-to-end while deferring their frontier impl — the contract is real, the
  advanced impl lands behind it.

**Rejected alternative (scalarize-and-smuggle for V1):** carry only
`Score.value` + the existing flat `metrics:{str→number}`, push the named-reward
vector through the untyped `candidate_summary.scores:{}` / `assessment_summary.evidence:{}`
escape hatches (`plan_result.v1:130,171`). Rejected: untyped means no
validation, no negative proof, no resume-fingerprint stability — it re-creates
the exact "designed-away" failure D5 forbids, and `matrix_rules` says a row is
not proven unless it says `shape_only`.

### Decision B — Bidirectional ACP: an asynchronous demultiplexing session loop

p9 ran the locked profile *backwards*; the spec's primary direction (worker is
the agent, engine is the client, `profile:7-9`) is exactly what the SDK needs.
The transport must become a bidirectional JSON-RPC peer with **two interleaved
legs on one session**:

- **Leg 1 (host→worker, stage dispatch):** the engine sends `leaven/stage.run`
  with a role-scoped payload via the existing `call_extension` direction.
- **Leg 2 (worker→host, effect callbacks):** while leg 1 is in flight, the
  worker calls `leaven/lm.complete` etc. *back*; the host services each as an
  inbound request and replies before leg 1's response arrives.

**Design (copy the proven in-repo template):** replace the single-threaded
`call_extension` read loop with a demultiplexer modeled on
`leaven-agent-codex-app-server/src/client.rs:184-217` (`wait_for_response`) +
`transport.rs:111-195` (async stdio). The loop classifies every inbound line:
(i) **response-by-id** to an outstanding host→worker request; (ii)
**`session/update` notification** (existing lifecycle handling); (iii)
**inbound request** (id + method + params, no result) → validate params as Plan
IR, dispatch to the `AcpEffectHost`, write the extension-result back with the
worker's id. The codex client does exactly (i)+(ii)+(iii) at
`client.rs:208-214`. The engine is already `#[tokio::main]` (p9 `main.rs:84`),
so the async demux (a dedicated reader task fanning into per-request oneshot
channels) is the clean target. `stdin` is already shareable behind
`Arc<Mutex<>>` (`stdio.rs:99-104`); the structural blocker is that `stdout` is
read only under `&mut self` (`stdio.rs:95, 263-288`) — that read must move into
the owned loop.

**Invariants that must hold in the new (inbound) direction:**

- ID namespaces must not collide: host-originated ids are `leaven-acp-{n}`
  (`stdio.rs:195`); worker-originated requests carry their own ids and must be
  answered, never confused with pending host responses.
- Inbound methods gate through `profile.method(...)` — private/MCP inbound is
  rejected, mirroring `tests:693-736`. The no-MCP, no-private guarantees hold
  *both* directions.
- The inbound request's capability fingerprint must match the launched session
  (the env injected at spawn, `session.rs:230-260`). Today nothing checks this
  because the worker never initiates.
- The effect handler bridges into `RunContext`, which is `&mut` and
  single-threaded (`LocalBoxFuture` per the proposer trait) — dispatch must run
  on the same task that owns `&mut RunContext`. Writes route through the
  finalizers `RunContext::propose`/`evaluate`, never the raw
  `proposal_context`/`evaluation_context` holes (`leaven-engine/AGENTS.md`).

### Wire changes the user must bless (the seam is not immutable, but each is a locked-schema edit)

1. **`Score.rewards[]`** (typed per-reward vector: id/value/weight/is_objective/
   feedback/output) + keep scalar `value` as the weighted roll-up.
   `common.schema.json#/$defs/Score`.
2. **`objective` + `objective_dims`** on `RequestEvaluationWrite.request`
   (`leaven.plan.v1` ~ line 2772) so the frontier axis travels with the
   evaluation request.
3. **`leaven/stage.run`** — a new locked extension method + `leaven.stage_run.*`
   schema + profile row + conformance/matrix rows, for host→worker stage
   dispatch carrying role-scoped payloads.
4. **`JudgeContext.target_visibility`** required enum (`target_free` /
   `target_under_policy`) to close the SDK's own open flag
   (`stage_payloads.v1:447-456`).
5. **`leaven/workspace.write_file`** (smaller; only if Python proposers write
   directly rather than via materialize/agent.run).
6. **An `integrated_surface` conformance row** proving a worker actually calls
   back (the matrix has the proof level but no callback row reaches it;
   `ps1.no_mcp.acp_only` is only a Rust-side route check).

Changes 1–4 are load-bearing for the contract; 5–6 close honesty gaps. None
routes through `leaven.worker_protocol.v1` — that schema is a deprecation
tombstone ("Use leaven.acp_profile.v1").

---

## 3. THINNEST REAL SLICE — example 03 running for real

Target: `docs/specs/leaven_py/examples/03_prompt_optimize.py` /
`sketch-03-prompt-optimize.py` running end-to-end for REAL — optimize a prompt
for arithmetic QA, one LM call per rollout, exact-match reward, a real (tiny)
GEPA loop. **Mock-or-live LM, no agent, no sandbox.** This is the smallest thing
that exercises the bidirectional callback p9 lacks.

The flow that must actually happen:

1. `optimize(...).run()` (host) lowers the `Environment` and spawns ONE Python
   worker over the p9-proven stdio transport.
2. The engine runs a real (tiny) GEPA loop. Per candidate-case it **dispatches a
   runner stage to the worker** (`leaven/stage.run`, host→worker — the leg p9
   never had).
3. The worker's `run(prompt, case, cx)` body calls **`cx.lm.complete(...)` which
   is `leaven/lm.complete` BACK to the engine** (worker→host — the bidirectional
   bit p9 lacks). The host executes the LM (mock or live), returns
   `LmResponse`. Worker returns the output string.
4. The engine dispatches the **reward stage** (`@lv.reward exact`) — exact-match
   `output == case.target["answer"]` → `float`. (For the thinnest slice the
   reward can run host-side as one degenerate reward; running it in the worker
   is the same machinery and is the natural next increment.)
5. GEPA screens parent vs child on a minibatch, accepts strict improvements,
   produces `Optimized[PromptArtifact]`.

What each layer needs for THIS slice (and explicitly what it does NOT):

**Wire schema (`leaven-public-seam`):**
- ADD `leaven/stage.run` method + minimal `leaven.stage_run` schema carrying a
  `RunnerRequest` (target-free, `stage_payloads.v1:341`) and returning an
  output string (an `OutputRecord` of kind `text`). Profile row + one
  conformance row.
- DO **not** yet add `Score.rewards[]` — exact-match is one scalar reward, so
  the existing scalar `Score.value` + required `Score.output` suffices. (The
  vector schema is slice 2, not slice 1.) This keeps slice 1 a pure
  bidirectional+dispatch proof, decoupled from the reward-vector schema churn.

**Transport (`leaven-acp`):**
- The bidirectional demux loop (Decision B): inbound-request branch +
  `AcpEffectHost` with **just `lm_complete`** wired (others can `unimplemented`/
  reject for this slice). Capability-fingerprint check on inbound. ID
  namespacing.
- Keep `call_extension` host→worker green for `leaven/stage.run`.
- Proof: a Python worker that *initiates* `leaven/lm.complete` and the host
  responds — the literal inverse of
  `stdio_session_runs_python_external_worker_program_end_to_end`
  (`tests:46-128`).

**Engine / bridge:**
- New `leaven-acp`-depending bridge crate exposing a Rust `Runner` adapter that:
  projects a `RunnerRequest` → `leaven/stage.run`, sends it, services the
  worker's `leaven/lm.complete` callbacks against a host `Lm` (mock or
  `leaven-lm-*`), parses the returned output string.
- Reuse `ScoringEvaluator` for the exact-match scorer (one scalar reward
  host-side) and the existing GEPA reference loop unchanged — it already does
  parent selection, minibatch screen, accept, validate (`step.rs:79-145`). No
  reward-vector, no `objective=` axis work in this slice (default `instance` /
  scalar).

**Python SDK (`docs/specs/leaven_py`):**
- Implement `serve_stage` (the real ACP server loop: read the capability env,
  accept a `leaven/stage.run` dispatch, invoke the decorated `run` with a
  `RolloutContext`, let `cx.lm.complete` call back, return output).
- Implement `cx.lm.complete` → `leaven/lm.complete` and `Rollout.fn`,
  `Environment`, `Task`, `Rubric([exact])`, `optimize().run()`,
  `runtime.local()`, `cases.from_jsonl`. Everything else stays
  `NotImplementedError`.
- Drop the scaffold-drift items so the slice is honest: remove
  `cx.proposals.apply` / `submit_and_apply` from `ProposeContext`, move
  `assessments.submit` to `EvaluatorContext` only (resolved verdicts; not
  exercised by slice 1 but cheap to fix while touching the package).

**LM, mock-or-live:** default the slice to a **mock host `Lm`** (deterministic,
no spend) so example 03 is a cheap reproducible product-proof of the
bidirectional+dispatch+tiny-GEPA path; add a `--live` flag wiring a real
`leaven-lm-*` provider as an explicit opt-in (mirrors p8's deterministic-default
+ live-opt-in discipline). No agent, no sandbox, no workspace materialize.

**Honest label:** this slice is the first **product-proof of the SDK
bidirectional seam** — but only of the prompt/LM/exact-match path. It is not yet
proof of the reward vector, agent rollout, sandbox, or `objective != instance`.

---

## 4. ORDERED PATH-TO-REAL (slice 03 → P5 agentic gate)

Each slice ends with a focused proof; `just check` is the completion gate when
behavior (not docs) changed. Cross-crate seam slices need their own
public-seam contract tests before any maturity claim.

1. **Slice 1 — bidirectional spike (no new wire).** Refactor
   `AcpStdioProcessSession` to the tokio demux loop, keeping current host→worker
   `call_extension` semantics green. Add the inbound-request branch +
   `AcpEffectHost` with `lm_complete`, capability-fingerprint check, ID
   namespacing. *Proof:* existing `stdio_session_contract.rs` unchanged + a new
   inverse test (Python worker initiates `leaven/lm.complete`, host responds).
   `cargo test -p leaven-acp`.

2. **Slice 2 — `leaven/stage.run` dispatch (wire + seam).** Add the method,
   minimal `stage_run` schema, profile row, conformance rows in
   `leaven-public-seam`. *Proof:* seam contract tests + topology contract.

3. **Slice 3 — example 03 real (the thinnest slice above).** New bridge crate's
   `Runner` adapter; Python `serve_stage` + `cx.lm` + `optimize().run()` over a
   mock host `Lm`; tiny GEPA loop. *Proof:* example 03 runs deterministically
   end-to-end; `--live` opt-in wires a real provider. This is the first SDK
   product-proof.

4. **Slice 4 — reward vector first-class (Decision A).** Add `Score.rewards[]` +
   `objective`/`objective_dims` to the wire; `ObjectiveVectorEvidence` in
   `leaven-evidence`; scorer fan-out over N reward fns; per-objective
   feedback/output → `ReflectiveRun.side_info`; weighted-sum roll-up feeds the
   GEPA gate. Implement `objective="weighted"` reduction + `objective="instance"`
   frontier (reuse), *validate* `objective`/`hybrid`/`cartesian` payloads while
   deferring their frontier impl. *Proof:* a multi-reward example (e.g.
   correctness+brevity) scoring a vector and reflecting per-objective feedback;
   seam contract for the extended `Score`.

5. **Slice 5 — full cx effect surface.** Wire the remaining `AcpEffectHost`
   methods into `RunContext` through the finalizers: `cx.workspace.read*` /
   `case.*` / `proposal.submit` / `assessment.submit` (evaluator-only) /
   `event.emit`. Add `leaven/workspace.write_file` if needed. Implement
   `cx.batch` (the least-proven ergonomic: accumulate into one plan doc, resolve
   on exit). *Proof:* example 06 (custom reflect/propose) + example 05
   (evaluator with judge) over the wire; `JudgeContext.target_visibility` closed.

6. **Slice 6 — agent + sandbox rollout.** Wire `cx.agent.run` → `leaven/agent.run`
   (host executes via `leaven-agent-codex-cli`, the live-Codex path p9 already
   proves) and build the `sandbox.exec` engine lowering (wire-locked,
   impl-missing). `Rollout.kind` selects runner-stage vs agent.run vs
   sandbox.exec. *Proof:* an agent-rollout example over the bidirectional seam,
   live Codex behind an explicit gate.

7. **Slice 7 — P5 agentic acceptance gate.** Compose slices 4–6 into the
   EvoSkill/agentic path: GEPA reflection over a materialized workspace
   (reuse `leaven-gepa-agentic-git` / `-skill` bridges), proposer writes skills,
   reward vector scores, real GEPA accept. This is where the SDK meets the
   existing P5 live-Codex milestone. *Proof:* the P5 milestone running through
   the SDK surface (not the hand-written example main), with the
   honest-classification discipline of `examples/AGENTS.md`. `just check`.

The dependency spine: 1 (transport) → 2 (dispatch) → 3 (first product-proof) are
strictly ordered; 4 (reward vector) and 5 (effect surface) can proceed in
parallel after 3; 6 builds on 5; 7 composes 4+5+6.

---

## 5. OPEN QUESTIONS FOR THE USER

1. **`leaven/stage.run` shape — one method or per-role methods?** One generic
   `leaven/stage.run` (params = stage kind + role-scoped payload) is cleaner and
   fewer profile rows; per-role methods (`leaven/stage.run_runner`, `…_scorer`,
   …) make the capability `required_action` more precise. The transport scout
   leans generic. Your call on the wire vocabulary granularity.

2. **Reward vector on the wire in V1, or defer to slice 4?** Decision A
   recommends carrying `Score.rewards[]` now (cheap, honors locked D5, avoids a
   second resume-fingerprint-breaking migration). But example 03 (slice 3) does
   NOT need it — exact-match is one scalar. Do you want the vector schema landed
   *before* slice 3 (so the seam is contract-complete from the first
   product-proof), or *after* (so slice 3 is the minimal bidirectional proof
   with zero reward churn)? The plan assumes after; confirm.

3. **Mock host `Lm` for the example-03 default — acceptable as product-proof?**
   p8 set the precedent (deterministic local model output, live opt-in). A mock
   `Lm` makes example 03 cheap/reproducible but is a fixture for the LM behavior.
   The bidirectional seam + dispatch + GEPA loop are still *real*. Is "real SDK
   seam, mock LM" the right product-proof bar for the first slice, or must the
   default be live?

4. **Bridge crate name + placement.** A new `leaven-acp`-depending crate is
   needed (the engine is cold of ACP; `leaven-acp` must not own graph mutation).
   Parallel to `leaven-gepa-agentic-git`. Proposed name e.g.
   `leaven-acp-stage-bridge` or `leaven-stage-acp`. Confirm the crate and its
   `AGENTS.md` boundary (it composes ACP transport + `RunContext` finalizers +
   stage-payload projection; it does not own GEPA policy, provider runtime, or
   the wire contract).

5. **`objective ∈ {hybrid, cartesian}` — validate-only in V1, or full impl?**
   Decision A ships `instance` (proven reuse) + `weighted` reduction, and
   *validates* the other axes' payloads while deferring their frontier impl. Is
   payload-validation-without-frontier-impl an acceptable V1 honesty posture (the
   contract is real, the advanced selection lands behind it), or do you want
   `objective="objective"` (reward-dimension Pareto) fully built in V1?

6. **`cx.sandbox.exec` engine lowering — in scope, or stub?** The method is
   wire-locked but has no engine path (cx-design §7.3). Slice 6 builds it. Is
   sandbox rollout in the V1 path-to-real, or deferred behind agent rollout?
