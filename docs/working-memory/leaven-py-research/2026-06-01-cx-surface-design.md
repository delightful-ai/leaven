# Leaven Python — The `cx` (Stage Context) Surface Design

Status: design note (pre-spec-revision). Persistent investigator deliverable.
Date: 2026-06-01.

Subordinate to `docs/specs/leaven_python.md` (governing Python product spec) and
the locked `docs/specs/public-seam-v1/` (governing wire). Where this note and
the locked seam disagree about wire behavior, the seam wins
(`leaven_python.md:636-643`). This note designs *what each stage role's `cx`
exposes, what each call returns, and where the capability + target-isolation
boundaries sit*, grounded in the Rust engine and the locked seam.

Input verdicts already accepted (do not relitigate): Rubric is the product
scoring surface; `evaluator` is internal/seam (resolved in the coherence ledger,
`2026-06-01-public-api-coherence-and-open-decisions.md:95-100`); `cx` is the
primary effect surface and `output` is the convenience default
(`…coherence…:50-55`); the projection audit's `cx` role-split and Case-projection
reworks are inputs (`2026-05-24-python-surface-rust-projection-audit.md:37-48`).

---

## 0. The load-bearing distinction (read this first)

There are **two different `RunContext`s** and conflating them is the root of the
scaffold's confusion:

1. **Rust engine `RunContext`** (`crates/leaven-engine/src/context/run_context.rs:28`):
   the lifetime-bound, generic-over-`P` **graph-mutation authority**. Its methods
   are `insert_seed`, `record_proposal_batch`, `propose`, `apply_batch`,
   `apply_proposal`, `charge`, `emit`, `graph()`, `budget()`, `case()`, plus the
   role-scoped sub-context constructors `proposal_context`, `evaluation_context`,
   `render_context`, `materialize_context`
   (`run_context.rs:154-343`). It has **no** `lm`, `agent`, `sandbox`,
   `workspace`, or `case.load` namespace. It is engine-internal and the spec
   explicitly forbids exposing it as a Python concept
   (`leaven_python.md:203-206,573-575`).

2. **Python `cx`** (the scaffold's `RunContext`/`StageContext`/`EvalContext`,
   `docs/specs/leaven_py/src/leaven/context.py`): the **wire-side stage handle**.
   Its namespaces (`cx.lm`, `cx.agent`, `cx.sandbox`, `cx.workspace`, `cx.case`,
   `cx.assessments`, `cx.proposals`, `cx.batch()`) construct **Plan IR ops**
   (`01_plan_ir_spec_v0.3.md`) that the engine validates against the locked seam
   before execution. Python never holds the Rust `RunContext`; it emits typed
   `AssessmentWrite`/`Proposal` envelopes and the engine applies them through the
   *Rust* `RunContext` on its side (`leaven_python.md:203-206`).

**Consequence for naming.** The Python class is currently *also* called
`RunContext` (`context.py:56`), which collides head-on with the spec's "don't
expose `RunContext`" rule (`leaven_python.md:573-575`). Whatever role-scoping
we choose, the Python class names must not be `RunContext` — see §6.

The wire roles are the canonical scoping axis. The capability `Subject.role`
enum is exactly (`schemas/leaven.capability.v1.schema.json`, Subject `oneOf`):

```
runner, scorer, reflector, proposer, judge, callback, artifact_adapter, dataset_adapter
```

plus the `evaluation_stage_call` subject (the evaluator) and `operator`. Every
`cx` capability is scoped by which of these the stage call's capability token
authorizes (`02_capability_spec_v0.3.md:51-58`).

---

## 1. Inventory: the current Python `cx` surface (the scaffold)

From `context.py` and `builders/*`. Every namespace is on the shared
`_ContextBase` (`context.py:30-53`) — **all three context flavors expose all
seven builders**. This is the role-scoping gap the audit flagged
(`…projection-audit…:48`: "All context flavors expose all builders → Rework").

| `cx.<ns>.<method>` | Args (key) | Returns | Source |
| --- | --- | --- | --- |
| `cx.case.load` | `case_id`, `include=("input","metadata")` | `Case` | `builders/case.py:20` |
| `cx.case.load_batch` | `case_ids`, `include=…` | `list[Case]` | `builders/case.py:35` |
| `cx.workspace.materialize_candidate` | `candidate_id`, `surface`, `lifetime` | `WorkspaceHandle` | `builders/workspace.py:66` |
| `cx.workspace.release` | `handle` | `None` | `builders/workspace.py:85` |
| `cx.workspace.read_file` | `handle`, `path`, `max_bytes` | `WorkspaceFile` (`content` + `QueryReceipt`) | `builders/workspace.py:89` |
| `cx.workspace.write_file` | `handle`, `path`, `content` | `CallReceipt` | `builders/workspace.py:99` |
| `cx.workspace.list` | `handle`, `path`, `recursive` | `WorkspaceListing` | `builders/workspace.py:108` |
| `cx.workspace.snapshot` | `handle`, `algorithm` | `WorkspaceSnapshot` | `builders/workspace.py:119` |
| `cx.workspace.git_diff` | `handle`, `against`, `expected_data_classes` | `WorkspaceDiff` (`text` + receipt) | `builders/workspace.py:128` |
| `cx.workspace.git_status` | `handle` | `WorkspaceStatus` | `builders/workspace.py:138` |
| `cx.workspace.git_log` | `handle` | `WorkspaceDiff` | `builders/workspace.py:142` |
| `cx.workspace.write_skills` | `handle`, `bank` | `CallReceipt` | `builders/workspace.py:146` |
| `cx.lm.complete` | `prompt`/`messages`, `model`, `model_role`, `response_format`, `tools`, `input_classes`, `forbidden_input_classes`, … | `LmResponse` (`text`, `parsed`, `usage`, `cost_usd`, `model`, `CallReceipt`) | `builders/lm.py:45` |
| `cx.agent.run` | `workspace`, `instructions`, `runtime`, `output`, `timeout_s`, `allowed_commands`, `input_classes`, … | `AgentSession` (`transcript_ref`, `parsed`, `final_message`, `files`, `commands`, `CallReceipt`) | `builders/agent.py:43` |
| `cx.sandbox.exec` | `workspace`, `argv`, `env`, `cwd`, `timeout_s`, `output`, `stream_policy`, `input_classes`, … | `SandboxExec` (`exit_code`, `stdout_ref`, `stderr_ref`, `files`, `CallReceipt`) | `builders/sandbox.py:36` |
| `cx.assessments.submit` | `evaluation_request_id`, `assessments: [AssessmentWrite]` | `AssessmentSubmission` (`WriteReceipt`, `submitted`) | `builders/assessments.py:26` |
| `cx.proposals.submit` | `batch: ProposalBatch` | `ProposalSubmission` (`WriteReceipt`, `proposal_ids`) | `builders/proposals.py:23` |
| `cx.proposals.apply` | `submission` | `WriteReceipt` | `builders/proposals.py:27` |
| `cx.proposals.submit_and_apply` | `batch` | `WriteReceipt` | `builders/proposals.py:31` |
| `cx.batch()` | — | `BatchBuilder` (async CM; `b.workspace/lm/agent/sandbox`) | `builders/batch.py:35` |
| `cx.stage_id` (prop) | — | `str` | `context.py:46` |
| `cx.capability_fingerprint` (prop) | — | `str` | `context.py:51` |
| `cx.candidate_id` / `cx.case_id` (RunCtx) | — | `str` | `context.py:60,65` |
| `cx.rollout_workspace` (RunCtx) | — | `WorkspaceHandle` | `context.py:69` |
| `cx.parent_candidate_id` (StageCtx) | — | `str \| None` | `context.py:84` |
| `cx.evaluation_request_id` (EvalCtx) | — | `str` | `context.py:99` |

The scaffold note at `context.py:11-14` is explicit that this is a typing
convenience and "the engine still enforces role-specific capabilities." That is
the gap: **the seam enforces, but the Python types do not declare.** The seam's
own design says syntax is not authorization (`01_plan_ir_spec:42`), but the
projection audit's verdict and the spec's "what the user writes" readability rule
(`leaven_python.md:567-572`) both want the *types* to say which method is legal.

---

## 2. Rust engine reality: what is actually enforced, and where

### 2a. Role read-scoping that EXISTS in the engine

`crates/leaven-engine/src/trust.rs` is the live scoping mechanism. It is
**partition-hiding**, not field-hiding:

- `TrustPolicy` hides case-set partitions per actor:
  `hide_from_proposers`, `hide_from_optimizers`, `hide_from_callbacks`
  (`trust.rs:52-78`). This is the **train/val/test split isolation** axis
  (`gepa_reflection_evidence_visibility.md:216-229`), *not* the
  input-vs-target axis.
- `Rust RunContext` builds per-role sub-contexts via
  `trust.{proposer,evaluator,renderer,callback}_read_scope()`
  (`run_context.rs:305-343`, `trust.rs:80-117`). Each carries a `ReadScope`
  (`hidden_partitions` + `EvidenceVisibility`).
- `EvidenceVisibility` (`trust.rs:24-34`): `Full / ScoresOnly / SummariesOnly /
  None` — degrades how much evidence detail a renderer/loader may expose without
  changing which graph records are visible.
- `check_evaluation_request` (`trust.rs:120-145`) refuses an `Actor`'s evaluation
  request that references hidden partitions. **Evaluator and Renderer are
  exempt** (`trust.rs:129`) — they may see all partitions, which is exactly why
  the evaluator is the privileged scoring role.

### 2b. Target-field isolation that EXISTS in the engine

This is a **different mechanism** from partition hiding, and it is **structural
by type**, owned in `leaven-eval` and `leaven-agentic`:

- `leaven-eval::Case<I, T>` keeps `input: I` and `target: Option<T>` as separate
  fields (`crates/leaven-eval/src/dataset.rs:9-58`); the spec's canonical shape
  (`case_visibility_and_target_isolation.md:59-68`).
- The runner view is **target-free by construction**:
  `RunCaseView<'a, I>` / `CandidateRunCtx<'a, P, I>` have **no `T` type
  parameter** — "that absence is the enforcement mechanism"
  (`case_visibility_and_target_isolation.md:104-127`).
- The scorer view carries target: `ScoreContext<'a, P, I, T, O>` /
  `ScoreCaseView<'a, I, T>` with `target: Option<&'a T>`
  (`case_visibility_and_target_isolation.md:152-168`).
- In the agentic path the same split is the presenter-vs-scorer trait boundary:
  `AgentCasePresentationInput` (runner-equivalent, materializes from input)
  vs `AgentCaseScoreInput` (scorer, carries the completed `session` + workspace)
  (`crates/leaven-agentic/src/case_evaluator.rs:32-89`). The crate AGENTS.md
  states the law: "keep hidden `CaseTarget` values as scorer-visible only"
  (`crates/leaven-agentic/AGENTS.md`, presenter/scorer decision card).

### 2c. Reflection visibility that EXISTS in the engine

Reflection/proposal never receive a raw `Case`. They consume a **target-safe
projection** built once:

- `ReflectiveCase` / `ReflectiveRun` / `ReflectiveValue`
  (`gepa_reflection_evidence_visibility.md:69-121`) carry `case_id`, an `input`
  projection, candidate `produced` output, `score`, `feedback`, source refs.
- The forbidden flow is explicit: target → reflective dataset is illegal; target
  may influence reflection **only** through scorer feedback
  (`gepa_reflection_evidence_visibility.md:186-208`).
- The Python `ReflectRequest.examples: list[ReflectExample]` already matches:
  `ReflectExample` has only `case_id`, `candidate_id`, `feedback`, `score`,
  `source_refs` (`docs/specs/leaven_py/src/leaven/stage_payloads.py:31-52`) — no
  `Case`, no `target`.

### 2d. What is real vs aspirational in `cx`

| `cx.*` namespace | Real engine/eval/agentic counterpart | Status |
| --- | --- | --- |
| `cx.proposals.*` | `RunContext::record_proposal_batch / propose / apply_batch` (`run_context.rs:167-242`); proposer trait (`stage/proposer.rs`) | **Real** (engine-side; Python emits envelopes) |
| `cx.assessments.*` | `Evaluator::evaluate → Metered<Vec<Assessment>>` (`stage/evaluator.rs:28-32`); evaluation request records | **Real** (engine-side) |
| `cx.case.load` | `RunContext::case(CaseId) -> Option<&P::Case>` (`run_context.rs:131`); `leaven-eval` views | **Real** read; projection/`include` gating is the design work |
| `cx.workspace.materialize_candidate` | `MaterializeContext` (`run_context.rs:340`); `AgentCasePresenter` (`case_evaluator.rs:51`) | **Real** for agentic/evaluator path |
| `cx.agent.run` | `AgentRuntime` / `AgentSession` (consumed by `AgentCaseEvaluator`) | **Real** runtime; `AgentSession` is a real Rust fact |
| `cx.sandbox.exec` | `sandbox_exec` Plan IR op (`01_plan_ir_spec:52-53`) | **Wire-locked; engine lowering not yet implemented** |
| `cx.lm.complete` | `lm_complete` Plan IR op (`01_plan_ir_spec:43`); `leaven-lm` `Lm` trait | **Wire-locked; provider-neutral request lowering exists, full Plan-IR path not yet wired** |
| `cx.batch()` | one Plan IR document with one receipt root (`01_plan_ir_spec` Call nodes) | **Wire-locked; not implemented** |
| `cx.workspace.git_*` / `read_file` | Workspace reads are Plan IR *expressions* (`01_plan_ir_spec:57-59`) | **Wire-locked; not implemented** |

The whole Python `cx` is `NotImplementedError`-scaffold today (every method body
raises). The "reality" is the wire contract + the Rust traits the engine will
lower into, not a running ACP loop. See §5.

---

## 3. The wire ceiling: what a stage can actually request, and how it is gated

`leaven.plan.v1` has three node classes (`01_plan_ir_spec:3-15`):

- **`Let`** (pure): query graph, cases, workspace snapshots.
- **`Call`** (effectful): `lm_complete`, `agent_run`, `sandbox_exec`, human
  review, workspace materialization, workspace release, extensions.
- **`Write`** (graph mutation intent): submit proposals, submit assessments,
  request evaluations, apply proposals, emit events, extension writes.

**This op set is the hard ceiling on what any `cx` namespace can offer.** Every
Python `cx.*` method must lower to one of these or it cannot exist.

Gating is two-layered (`02_capability_spec`, `01_plan_ir_spec:42`):

1. **Capability grants.** A `Grant` = `action` (path string, e.g. `lm_complete`,
   `agent_run`, `case.target`) + `resource` + `constraints` + `limits`
   (`schemas/leaven.capability.v1.schema.json` Grant). The `Subject` pins
   `role` + `run` + `stage_call_id` (`Subject.oneOf`). Mint-time validation
   enforces role/purpose invariants JSON Schema cannot
   (`02_capability_spec:51-58`):
   - **a runner cannot receive target fields**;
   - **a reflector cannot receive target egress grants under normal GEPA
     policy**;
   - **an evaluator cannot submit assessments outside its evaluation request**.
2. **Data classes.** Deny lists override allow lists; data classes propagate
   monotonically; a call whose accumulated input classes intersect a forbidden
   class is denied **before host execution**
   (`leaven_python.md:271-274`, `data_class.py:5-12`). Critically:
   **`case.target` read access does not imply `case.target` egress**
   (`02_capability_spec:27`). A scorer may *read* target but an LM/agent call it
   makes can still be denied target egress by `forbidden_input_classes`.

### Structural target gates at the wire (the proof, not just policy)

From `schemas/leaven.stage_payloads.v1.schema.json`:

- `RunnerRequest`: `case_input` is present; `target_forbidden: {"const": true}`.
  There is **no target field and no target handle** — the runner is provably
  target-free at the wire (`04_stage_payloads_spec:26`).
- `ScoreContext`: has `output: OutputRecord`, `evaluation_request_id`, and an
  **optional** `target_handle: CaseRef`. Target is a *gated handle the scorer
  dereferences*, not an inline field (`04_stage_payloads_spec:27`: "Score
  contexts may be target-aware only under policy").
- `ReflectRequest`: `examples: [ReflectiveExample]` +
  `target_safety: {"const": "target_safe_projection"}`. No raw case, no target
  (`04_stage_payloads_spec:9`). `ReflectiveExample` has
  `input/output/score/feedback/data_classes` — **no `target`/`expected` field at
  the wire** (note: the Rust `ReflectiveCase` *does* have `expected`,
  `gepa_reflection_evidence_visibility.md:71`; the wire projects only what the
  policy makes optimizer-visible — see Open Question Q4).
- `ProposeRequest`: `parent` + `reflection_result` + `allowed_effects` +
  `allowed_change_schemas`. No case, no target.
- `JudgeContext`: `left/right` candidates + `outputs: array` + optional `case` +
  optional `rubric`. No target field (`04_stage_payloads_spec:21-24`).

These `const` assertions are the wire saying target isolation is **structural**,
not advisory. The Python role contexts must mirror this exactly.

---

## 4. Per-role capability + data-access TABLE

The five product/seam roles, what `cx.*` each may use, and the legal `case`
projection. "Forbidden" = mint-time capability refusal or structurally-absent at
the wire, not just convention.

Legend for case projection:
- **input-only**: `case.input` (+ `case_id`); no target, no metadata by default.
- **input+target**: may read `case.target` via gated handle/`include`.
- **target-safe-reflective**: receives only the `ReflectiveExample` projection
  (id/input/output/score/feedback) — never a `Case` object.

| `cx.*` | **rollout-cx** (`runner`) | **rubric-cx** (`scorer`) | **reflect-cx** (`reflector`) | **propose-cx** (`proposer`) | **evaluator-cx** (`evaluation_stage_call`, internal) |
| --- | --- | --- | --- | --- | --- |
| **case projection** | input-only (`RunCaseView`, no `T`) | input+target (gated `target_handle`) | target-safe-reflective (no `Case`) | target-safe-reflective (no `Case`) | input+target+metadata (full `cx.case.load(include=[…,"target"])`) |
| `cx.case.load` | ✅ input/metadata only; **target FORBIDDEN** (`include` rejected by cap) | ✅ incl. `target` under policy | ❌ no raw case load; gets `req.examples` | ❌ no raw case load; gets `req` | ✅ full, incl. target |
| `cx.lm.complete` | ✅ (no target egress; runner has no target) | ✅ (judge-style rewards; target read OK, **egress gated**) | ✅ (reflection LM; **target egress denied**, `02_capability_spec:56`) | ✅ (proposer LM) | ✅ |
| `cx.agent.run` | ✅ (Codex-native rollout substrate) | ✅ (grade captured files/diffs) | rarely; agent-backed reflector allowed (`04_stage_payloads_spec:17`) | ✅ (agentic proposer, sketch-04) | ✅ |
| `cx.sandbox.exec` | ✅ | ✅ | ❌ by default | rarely | ✅ |
| `cx.workspace.read_file/git_*/snapshot` (reads) | ✅ on `cx.rollout_workspace` | ✅ on `cx.rollout_workspace` | ❌ by default | ✅ on proposer workspace | ✅ on materialized candidate |
| `cx.workspace.materialize_candidate` | ❌ (engine materializes the rollout ws; see §5 audit) | ❌ (use `cx.rollout_workspace`) | ❌ | ✅ (build proposal workspace) | ✅ (evaluator materializes) |
| `cx.workspace.write_file/write_skills` (Call writes) | ✅ inside rollout ws only | ❌ | ❌ | ✅ inside proposal ws | ✅ |
| `cx.proposals.submit/apply` (Write) | ❌ | ❌ | ❌ (reflection ≠ mutation, `04_stage_payloads_spec:6`) | ✅ (the proposer's whole job) | ❌ |
| `cx.assessments.submit` (Write) | ❌ | ❌ (scorer returns `Score`/`RewardValue`; engine writes the assessment) | ❌ | ❌ | ✅ (only within own `evaluation_request_id`, `02_capability_spec:58`) |
| `cx.batch()` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `cx.rollout_workspace` (prop) | ✅ (engine-prepared) | ✅ (same ws the rollout used) | ❌ (no rollout in scope) | ❌ | ❌ (materializes its own) |
| `cx.evaluation_request_id` (prop) | ❌ | ✅ (scoped) | ❌ | ❌ | ✅ |
| `cx.parent_candidate_id` (prop) | ❌ | ❌ | ✅ (None if no explicit parent) | ✅ | ❌ |
| `cx.candidate_id` / `cx.case_id` (prop) | ✅ | ✅ | ❌ | ❌ | per-item via `job.independent_cases()` |
| `cx.stage_id` / `cx.capability_fingerprint` (props) | ✅ all roles | ✅ | ✅ | ✅ | ✅ |

Key reductions from the scaffold's "everything everywhere":
- **rollout-cx loses**: `case.target`, `materialize_candidate`,
  `proposals.*`, `assessments.*`. It is the most-restricted product role.
- **rubric-cx loses**: `materialize_candidate`, `proposals.*`, `assessments.*`.
  It gains target read (gated) and `cx.rollout_workspace`.
- **reflect-cx / propose-cx lose**: `cx.case` raw load entirely (they get typed
  `ReflectRequest`/`ProposeRequest`), `cx.rollout_workspace`. propose-cx is the
  only product role with `cx.proposals.*`.
- **evaluator-cx** is the privileged superset (the seam shape, not a product
  front door): the only role with `cx.assessments.submit`, full target load, and
  free candidate materialization. This is the `evaluator_dspy_codex.v0.3.py`
  shape (`docs/specs/public-seam-v1/examples/evaluator_dspy_codex.v0.3.py`).

`judge` (judge-cx) was not in the prompt's five but is a wire role: input is
`JudgeContext` (candidates + outputs + optional case/rubric); same capability
profile as reflect/propose (LM/agent allowed, no `case.target` inline, no
`proposals`/`assessments`). It returns a preference/ranking the engine records.

---

## 5. The rollout-result contract (resolving the ledger's fuzziness)

The ledger flagged "what IS the rollout result" as fuzzy
(`…coherence…:50-55`). Resolution, grounded in verifiers + the wire:

### What verifiers does (prior art)
- `State` (`docs/specs/leaven_py/repos/verifiers/verifiers/types.py:435`) carries
  `input` (prompt/answer/info), `completion`, `trajectory`, `reward`, `metrics`,
  `timing`, plus borrowed `client`/`sandbox` handles
  (`types.py:447-466`).
- `RolloutOutput` (`types.py:370-400`) is the *serialized* result: required
  `completion`, `reward`, `timing`, `metrics`; optional `answer`, `info`,
  `trajectory`, `token_usage`.
- A reward function receives by-name kwargs from `{state, prompt, completion,
  answer, info, task}` + parser objects
  (`verifiers/rubrics/rubric.py:285`, default `func(completion, answer, **kwargs)`
  at `rubric.py:192`). **`completion` is "the output"; `answer` is the target —
  and verifiers gives the rubric both freely.** This is the prior art for: rubric
  reads `output` AND `case.target`.

### The Leaven contract (proposed)
Three distinct things, do not conflate:

1. **`output`** = the projected rollout result, the *first positional* the
   rubric/reward receives. It defaults to:
   - **agent rollout** (`Rollout.agent()`): the agent's **final message**
     (`AgentSession.final_message`, `builders/agent.py:30`), per
     `…coherence…:50-55`.
   - **non-agent / `Rollout.fn`**: the runner's **return value** (sketch-03:
     `run(...) -> str` returns `reply.text.strip()`,
     `sketch-03-prompt-optimize.py:28-30`), i.e. final LM text.
   - **override**: `Rollout(..., output=lv.output.json_schema(M) | files([...]) |
     text())` reshapes it to parsed JSON / captured files / text. This is the
     `OutputContract` (`output.py:16-49`), the same contract `cx.agent.run` and
     `cx.sandbox.exec` take. The projected value is an `OutputRecord` family
     value (`output_record.py:25` kinds: `text/json/blob_ref/structured/
     agent_session/workspace_diff`).
   `output` is the **convenience default**; it is not the whole result.

2. **`cx.rollout_workspace`** = the engine-prepared `WorkspaceHandle`
   (`context.py:69`, `_handles.py:19`) for the same rollout. The rubric uses it
   to grade *beyond* the final message: read produced files
   (`cx.workspace.read_file(cx.rollout_workspace, …)` — exactly the spec scorer
   at `leaven_python.md:101-104`), `git_diff`, run hidden tests. This is the
   `AgentCaseScoreInput.workspace` in Rust (`case_evaluator.rs:84-88`). The audit
   says keep it but only as the *runtime-prepared* workspace, not a
   user-materialized one (`…projection-audit…:40`).

3. **agent session handle** = `AgentSession` (`builders/agent.py:16-38`):
   `transcript_ref` (blob), `parsed`, `final_message`, `files`, `commands`,
   `cost_usd`, `receipt`. This is the *full* rollout fact when the rubric needs
   the trajectory/transcript/commands, not just the final message. For
   `Rollout.agent()`, `output` is the projection *of* this session; the session
   itself is reachable when the rubric needs more (Open Question Q2: is the
   session on `cx` or passed as a second rollout-result arg?).

**Return types of the load-bearing calls** (all carry a receipt; receipts are
audit currency, `_receipts.py`, `leaven_python.md:275-277`):
- `cx.lm.complete(...) -> LmResponse` = `text`, `parsed`, `finish_reason`,
  `usage`, `cost_usd`, `model`, `CallReceipt` (`builders/lm.py:25-39`).
- `cx.agent.run(...) -> AgentSession` (above).
- `cx.sandbox.exec(...) -> SandboxExec` = `exit_code`, `stdout_ref`,
  `stderr_ref`, `files`, `cost_usd`, `CallReceipt` (`builders/sandbox.py:15-27`).
- a `Rollout.fn` body returns **whatever the user returns** (becomes `output`);
  the reward returns `float | RewardValue` (per ledger D7,
  `…coherence…:119`).

---

## 6. RECOMMENDATION: how to express role-scoping in the Python types

### The options
- **(A) One class, capability-gated methods that raise.** Keep a single `cx`
  class; methods like `cx.proposals.submit` raise `CapabilityError` when the
  role isn't authorized. This is what the scaffold does today implicitly
  (`context.py:11-14`).
- **(B) Separate context classes per role**: `RolloutContext`, `RubricContext`,
  `ReflectContext`, `ProposeContext`, `JudgeContext`, `EvaluatorContext` — each
  exposes only its legal namespaces; an illegal method does not exist on the
  type.

### Recommendation: **(B), separate classes — with shared mixins**, and rename off `RunContext`.

Reasons, argued against the spec rule and the audit:

1. **The spec's own rule forces it.** `leaven_python.md:617-625` says: *"If
   `RunContext` carries a field that's present in some context types and absent
   in others … user code that reads the field silently fails … Either every
   context type has the field, or it is not a context field — surface it through
   a builder method that raises explicitly."* Option (A) is precisely the
   "optional across boundaries" anti-pattern: `cx.proposals` would be a real
   attribute on rollout-cx that raises at call time. The spec permits *one*
   escape — "surface it through a builder method that raises explicitly when
   unavailable" — but that escape is for fields *unavailable in some execution
   modes* (in-process vs out-of-process), **not** for capabilities forbidden by
   **role**. Role is known statically at authoring time; the seam roles are a
   closed enum (`Subject.role`). When the boundary is static and closed, the
   type should encode it. So (B) is the spec-aligned reading, and (A) is the
   reading the spec wrote that paragraph to forbid.

2. **The audit demands it.** `…projection-audit…:48`: "All context flavors expose
   all builders → **Rework** → Split or document role-specific capabilities.
   Runners should not get proposal/assessment mutation surfaces." And
   `…projection-audit…:37,39`: "Runner projection must be structurally
   target-free" / "Runner gets input-only case view." **"Structurally" is the
   operative word** — it means by type, mirroring the Rust `RunCaseView` having
   no `T` parameter (`case_visibility_and_target_isolation.md:124-127`) and the
   wire `RunnerRequest.target_forbidden: const true`. A raise-at-runtime gate is
   not structural.

3. **Readability rule.** `leaven_python.md:567-572`: every Python surface must be
   readable as "this is what the user writes" without cross-file inference.
   `def run(prompt, case, cx: RolloutContext)` tells the reader at the signature
   that target and proposals are off-limits. `cx: RunContext` that *might* raise
   tells them nothing until runtime.

4. **It matches the wire 1:1.** Six wire roles → six context classes. The
   capability `Subject.role` enum is the authority; the Python type is the local
   projection of it. No new axis invented.

### Concrete shape
```
class _CxBase:              # stage_id, capability_fingerprint, batch(); all roles
class _EffectCx(_CxBase):   # lm, agent, sandbox  (mixin: roles that run effects)
class _ReadWsCx(_CxBase):   # workspace reads on a given handle

RolloutContext(_EffectCx, _ReadWsCx)   # + rollout_workspace, candidate_id, case_id
                                       #   case: input-only loader
RubricContext(_EffectCx, _ReadWsCx)    # + rollout_workspace, evaluation_request_id
                                       #   case: target-capable loader
ReflectContext(_EffectCx)              # + parent_candidate_id; NO cx.case, NO proposals
ProposeContext(_EffectCx, _ReadWsCx)   # + parent_candidate_id, proposals.*, materialize
JudgeContext(_EffectCx)                # + outputs; no proposals/assessments
EvaluatorContext(_EffectCx, _ReadWsCx) # + case (full), materialize, assessments.* (priv)
```

This keeps the "one canonical way" rule (`leaven_python.md:579`) — there is one
context per role, not two ways to do one thing — while satisfying "don't put
optional-across-boundaries fields." **None of these is named `RunContext`**,
resolving the `leaven_python.md:573-575` collision. `cx.case` differs by role via
the *loader type* (input-only vs target-capable), which is the structural
target-free guarantee, not an `include=` flag the runner could pass.

Counter-argument acknowledged: more classes = more surface. Mitigated by shared
mixins (the namespaces are defined once). The alternative (A) trades type surface
for runtime failure surface in an audit-first system that explicitly values
"capability authorization … traceable by reading the call site"
(`leaven_python.md:600-604`). For a data/model-correctness system, structural
beats raise-at-runtime.

---

## 7. REALITY-VS-INTENT: where Python `cx` outruns the engine / wire

Honest maturity. None of `cx` runs today (all bodies raise
`NotImplementedError`). Beyond that, specific gaps:

1. **No `cx`→wire lowering exists.** `cx.*` is documented to "construct typed
   Plan IR ops" (`builders/__init__.py:5-7`), but there is **no implemented Plan
   IR builder, no `leaven-acp` Python worker loop, no `leaven serve --stdio`
   path** wired to these. The whole surface is intent against the locked schema.
   `serve_stage` raises (`decorators.py:273`); `optimize(...).run()` raises.

2. **`cx.batch()` is unproven.** The "one Plan IR document, one receipt root"
   transaction (`01_plan_ir_spec` Call nodes; `leaven_python.md:354-358`) and the
   placeholder-resolution semantics (`builders/batch.py:11-19`) have no engine
   counterpart yet. It is the load-bearing ergonomic and the least-proven piece.

3. **`cx.sandbox.exec` lowering not implemented.** `sandbox_exec` is locked v1 at
   the wire (`01_plan_ir_spec:52-53`) but there is no engine sandbox-exec lowering
   from a Plan IR Call. (Workspace + agent paths exist in `leaven-agentic`;
   sandbox-as-Plan-IR-op does not.)

4. **`cx.rollout_workspace` for non-agentic rollouts.** It is real in the
   agentic scorer path (`AgentCaseScoreInput.workspace`,
   `case_evaluator.rs:84-88`), but for an `Rollout.fn` LM-only rollout there is
   no workspace — the property must be **absent or raise** for those rollouts.
   This is exactly the "optional across boundaries" hazard
   (`leaven_python.md:617-625`): `rollout_workspace` is present for agent/command
   rollouts and meaningless for `lm.complete`-only rollouts. It must be surfaced
   as a method/property that raises explicitly when the rollout has no workspace,
   or scoped to a `RolloutContext` variant.

5. **Codex-native default is unproven (ledger D10).** `Rollout.agent()` defaults
   to Codex as the materialization/rollout substrate
   (`sketch-04-evoskill.py:47-57`), but no live Codex has run the loop;
   skill-bank reflection is proven only with `FakeAgentRuntime`
   (`…coherence…:125`). `cx.agent.run` returning a real `AgentSession` from Codex
   is intent, not proof.

6. **`cx.proposals.apply` from a Python proposer is suspect.** The engine AGENTS
   warns `proposal_context`/`evaluation_context` are *current public holes*,
   "raw/non-finalizing until sealed" (`leaven-engine/AGENTS.md` decision card).
   Python `cx.proposals.submit_and_apply` (`builders/proposals.py:31`) implies a
   finalizing apply a Python proposer triggers; the canonical path is the engine
   calling `RunContext::propose` (`run_context.rs:202`) which charges/records/
   emits/checkpoints. **Whether a Python proposer may `apply` (vs only `submit`,
   engine applies) is unresolved** and the wire's `Write` op set lists both
   "submit proposals" and "apply proposals" (`01_plan_ir_spec:15`) — so the
   ceiling allows it, but the engine finalizer discipline argues against it as an
   ordinary product path.

7. **`cx.assessments.submit` vs rubric returning a `Score`.** The product rubric
   returns `float|RewardValue` and the engine writes the assessment; only the
   *evaluator* role calls `cx.assessments.submit` directly
   (`evaluator_dspy_codex.v0.3.py:54-67`). The scaffold puts
   `AssessmentsBuilder` on `_ContextBase` (every role), which is wrong:
   assessment submission is evaluator-only at the wire
   (`02_capability_spec:58`).

---

## 8. OPEN QUESTIONS for the main session

- **Q1 — Class-per-role vs gated-raise (the §6 decision).** Confirm separate
  context classes (`RolloutContext`/`RubricContext`/`ReflectContext`/
  `ProposeContext`/`JudgeContext`/`EvaluatorContext`) and the rename off
  `RunContext`. This is the priority decision; the table in §4 is its
  specification.

- **Q2 — rollout-result arity for the rubric.** Does the rubric/reward receive
  *only* `output` (with the `AgentSession` reachable via `cx`), or `(output,
  session)` / `(output, case, cx)` where the full session is a second arg?
  verifiers passes the whole `State` to rewards (`rubric.py:285`); sketches pass
  `(output, case, cx)` (`sketch-03:39`). Recommend: `(output, case, cx)` with the
  session reachable as `cx.rollout_session` on `RubricContext` — but confirm
  whether the session handle is a `cx` property or a positional.

- **Q3 — `case.target` access shape for the rubric.** Sketches read
  `case.target["answer"]` directly off a `Case` object (`sketch-03:40`), but the
  wire gives the scorer a gated `target_handle: CaseRef`
  (`ScoreContext.target_handle`), and the law says scorer target is "controlled"
  (`case_visibility_and_target_isolation.md:91-94`). Decide: does rubric-cx get
  `case.target` as an eagerly-projected field (simple, matches sketches) or must
  it `await cx.case.load(case_id, include=["target"])` / dereference the handle
  (matches wire, costs a round-trip, but keeps the gate explicit and receipted)?

- **Q4 — `expected` in reflective examples.** The Rust `ReflectiveCase` has an
  `expected` field (`gepa_reflection_evidence_visibility.md:71`) but the wire
  `ReflectiveExample` does **not** (only input/output/score/feedback). Confirm
  the Python `ReflectExample` correctly omits `expected` (it does today,
  `stage_payloads.py:31-40`) and that any "expected" signal reaches reflection
  *only* via `feedback` — i.e. the Rust `expected` is a non-target-derived label
  (e.g. format hint) and never the gold answer, or it must be dropped from the
  wire projection.

- **Q5 — May a Python proposer `apply`, or only `submit`?** (§7.6) The wire
  ceiling allows both `Write` ops; the engine finalizer discipline
  (`RunContext::propose`) argues the engine should own apply. Decide whether
  `cx.proposals.apply` / `submit_and_apply` stay on `ProposeContext` as ordinary
  product calls or are demoted to advanced/evaluator-only.
