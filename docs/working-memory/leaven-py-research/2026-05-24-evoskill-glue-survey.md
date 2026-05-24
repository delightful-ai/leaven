# EvoSkill Glue-Code Reality Check for the Python SDK

Status: pre-spec research.
Updated: 2026-05-24.

## Authority

This note is subordinate to:

- `docs/working-memory/evoskill-replication.md` (active EvoSkill dossier).
- `docs/working-memory/skill-paper-replication.md` (parent paper-replication
  ledger).
- `docs/working-memory/leaven-py-and-acp-transport.md` (the Python SDK + ACP
  transport pre-spec note that drives this question).
- `docs/specs/agentic_skill_optimization_primitives.md` (governing skill spec).
- `docs/specs/public-seam-v1/examples/evaluator_dspy_codex.v0.3.py`
  (aspirational Python target shape).
- `examples/p5_evoskill_iteration/` (current live one-iteration EvoSkill
  proof).
- `examples/p5_skill_paper_reproductions/` (current paper-close no-spend
  manifest/report harness).
- `crates/leaven-agentic-skill/`, `crates/leaven-gepa-agentic-skill/` (the
  reusable skill-bridge stack).

It records the survey done on 2026-05-24 to test whether the proposed Python
SDK actually unblocks EvoSkill-shaped paper repros. It is not proof of a SDK
design and does not promote anything to product law.

## 1. Current EvoSkill Glue Inventory

The active EvoSkill replication code sits in two example crates plus the
reusable skill-bridge stack. Line counts come from `wc -l`; descriptions are
function-grouped from grepping module/symbol structure.

### 1a. `examples/p5_evoskill_iteration` — live one-iteration loop (~2,978 LOC)

| Module | LOC | Role |
| --- | --- | --- |
| `src/main.rs` | 2,417 | Orchestrates the EvoSkill iteration: CLI parsing, run-store/checkpoint setup, preflight, baseline/failure/proposal/build/evaluate phases, frontier admission, summary writing. |
| `src/data.rs` | 211 | EvoSkill case struct + `Split` enum + OfficeQA/SealQA JSON sample loaders + `case_set` partition builder. |
| `src/roles.rs` | 85 | String constants for executor/proposer/builder developer instructions + meta-skill content. |
| `src/checkpoint.rs` | 72 | Five-variant `EvoSkillCheckpoint` enum carrying every piece of resume state (frontier, sampler, selector, cases, bank, proposal, evidence ref, change). |
| `src/scorer.rs` | 52 | EvoSkill multi-tolerance numeric/text scorer (`{0.0, 0.01, 0.025, 0.05, 0.10}` weighted) — matches paper appendix. |
| `src/error.rs` | 47 | Thin local error wrapper / `msg()` helper. |
| `src/evidence.rs` | 40 | Local evidence enum: `AgentRoleSession{role,instr,session}` and `Evaluation{candidate,split,avg,cases}`. |
| `src/codex.rs` | 30 | Live Codex CLI runtime config (model, approval bypass, bin path env). |
| `src/proposal.rs` | 24 | `SkillProposal` + `EvoSkillProposalAnnotations` types. |

Inside `main.rs` the load-bearing glue surfaces are:

- **Problem-type wiring** (`main.rs:70-77`): `impl OptimizationProblem for EvoSkillProblem` declaring `Artifact = SkillBank`, `Case = EvoSkillCase`, `Evidence = EvoSkillEvidence`, `ProposalAnnotations = EvoSkillProposalAnnotations`. Couples the local types into the generic `RunContext<P>` machinery.
- **Run-store/checkpoint plumbing** (`main.rs:104-173`, `847-852`, `1166-1306`): `RunStores::open`, `load_private_checkpoint`, `print_complete_resume`, `ResumeState::from_checkpoint`, `checkpoint_phase`. Five-variant private checkpoint enum that must be threaded through every phase.
- **Preflight report writer** (`main.rs:193-261`): `write_preflight_report` — synthetic session, dry-run presenter, dry-run scorer, store writes, write JSON report, fail-fast.
- **Five-phase iteration** (`main.rs:271-444`): `run_iteration` orchestrates `ensure_baseline`, `ensure_failures`, `ensure_proposal`, `ensure_child`, `complete_iteration`, each with its own resume short-circuit, its own request struct, and its own checkpoint emission. The five `*Request` structs and four `ensure_*` functions are essentially boilerplate that re-projects checkpoint state into call arguments.
- **`AgentCaseEvaluator` stack assembly** (`main.rs:1087-1148`): `ExecutorStack`, `new_executor_stack` — case partition builder, workspace factory, runtime, presenter, scorer wired into one `AgentCaseEvaluator`. Includes hand-built `CasePartitions` over `TRAIN`/`VALIDATION` constants and per-case `AgentCase` construction.
- **`AgentCasePresenter` impl** (`main.rs:1466-1514`): `EvoSkillPresenter` calls `SkillBankMaterializer::materialize_into` to project the candidate bank into `.agents/skills`, writes `task/case.json`, builds `AgentInstructions` with system prompt + 240s timeout.
- **`AgentCaseScorer` impl** (`main.rs:1516-1559`): `EvoSkillScorer` parses the final assistant JSON, checks whether the bank has any skill (skill-gating), calls `multi_tolerance_score`, emits per-case `EvoSkillEvidence::Evaluation`.
- **Skill-proposer phase** (`main.rs:1620-1684`): `run_skill_proposer` — manually allocates a workspace, writes meta-skill files + failures JSON + existing-skills markdown, calls `runtime.run_session` directly, parses JSON, stores evidence by hand. Bypasses the agentic-proposer trait stack.
- **Skill-builder phase** (`main.rs:1692-1768`): `run_skill_builder` builds a `RepairingAgenticProposer` from scratch — needs custom `Materializer`, `Renderer`, `ProposalRepairPromptBuilder`, `ProposalParser`, plus a wrapping `RecordingBuilderRuntime` to capture session evidence. The traits are `SkillBuilderMaterializer` (1799-1813), `SkillBuilderRenderer` (1815-1834), `SkillBuilderRepairPrompt` (1837-1858), `RecordingBuilderRuntime` (1860-1901), `SkillBuilderParser` (1903-1934), totaling ~140 lines of trait impls.
- **Evaluation/frontier helpers** (`main.rs:1313-1464`): `evaluate_one`/`evaluate_cases`/`evaluate_set` calling `ctx.evaluate_with(..., EvaluationRequest::Independent)` then unpacking `assessment_evidence` per case; `observe_frontier` (writes `TopKFrontier` events through `ctx.emit`); `evoskill_frontier`/`evoskill_parent_selector`/`evoskill_train_sampler` constructors.
- **Resume-path `record_skill_change`** (`main.rs:2068-2117`): manual `Proposal::mutate` + `ProposalBatch` + `record_proposal_batch` + `apply_batch` to re-admit a recovered child without re-running the builder agent. Mirror of what `ctx.propose` would do on the normal path.
- **JSON-parsing helpers** (`main.rs:2022-2066`): `final_json`, `final_assistant_message`, `extract_json_object` — strips fences, finds first `{` to last `}`. Each role's parse goes through this.
- **Workspace I/O helpers** (`main.rs:2125-2183`): `materialize_skill_bank_direct`, `write_json`, `write_file`, `finish_workspace` (with custom error reconciliation between stage failure and cleanup failure).

### 1b. `examples/p5_skill_paper_reproductions` — no-spend paper-close harness (~9,479 LOC)

| Module | LOC | Role |
| --- | --- | --- |
| `src/evoskill.rs` | 9,351 | All no-spend manifest, source pinning, split policy, scorer replay, judge request materialization, runner-input/output bridges, final report, audit, ablations. 280 `fn`s, 20 `pub fn`s. |
| `src/main.rs` | 125 | clap CLI dispatching to ~10 manifest/sidecar writers + final report build + audit. |
| `src/lib.rs` | 3 | Map only. |

Major glue groupings inside `evoskill.rs` (line numbers from `grep` of `fn`/`struct`/`enum`):

- **Schema types** (lines 80-940): ~80 serializable structs/enums (`EvoSkillReplicaManifest`, `SourceRevision`, `SourceUniverseEntry`, `SplitMaterializationReport`, `ScorerManifest`, `JudgeTemplateManifest`, `PaperResultTarget`, `EvoSkillFinalReport`, `FinalScoreSlot`, `LiveRunGateReport`, `ProxyRejectionGate`, `ExactnessGapReport`, ...) plus internal helpers. Each report is bespoke JSON shape with refusal-capable ingestion semantics.
- **Public writers** (the 20 `pub fn`s, lines 1161-1551): `build_evoskill_replica_manifest`, `write_evoskill_local_source_pin_manifest`, `write_evoskill_paper_close_split_policy_manifest`, `write_evoskill_browsecomp_public_transfer_sample`, `write_evoskill_officeqa_score_result_manifest`, `write_evoskill_sealqa_judge_score_result_manifest`, `write_evoskill_sealqa_judge_request_batch`, `write_evoskill_runner_input_batch`, `write_evoskill_runner_output_batch`, `write_evoskill_live_run_request_manifest`, `build_evoskill_final_report`, `audit_evoskill_paper_close_report`. Each is a strict refusal-capable sidecar I/O path with hash/fingerprint matching, role/source-id coverage validation, scorer replay, and merge-not-overwrite semantics. The `_officeqa_score_result_manifest` writer alone is ~50 lines of orchestration; its peer `_sealqa_judge_score_result_manifest` is the same shape (lines 1267-1373).
- **Internal materializers** (the remaining ~260 `fn`s): CSV parsers, JSONL row builders, source-id fingerprinters, BrowseComp public-CSV decryptor (canary-keyed XOR per `browsecomp_eval.py`), file-byte SHA-256 hashers, fingerprint binders, role coverage validators, judge template loaders, runner-input answer-stripping.

### 1c. Reusable skill-bridge stack (paper-neutral; ~2,903 LOC)

| Crate | Module | LOC | Role |
| --- | --- | --- | --- |
| `leaven-agentic-skill` | `patch_plan.rs` | 663 | `SkillPatchPlan` validation: existing-file/create/overwrite guards, support counts, line-range conflict, atomic `references/*.md` pairing. |
| `leaven-agentic-skill` | `merge_tree.rs` | 477 | `SkillPatchMergeTree` provenance: levels, accepted/discarded inputs, output plans, final plan id. |
| `leaven-agentic-skill` | `patch_apply.rs` | 341 | `SkillPatchApplication` atomic apply + rollback. |
| `leaven-agentic-skill` | `report.rs` | 251 | `SkillBankChangeReport` (description/file kinds, rename). |
| `leaven-agentic-skill` | `parsed_patch.rs` | 181 | `SkillParsedPatchDocument` operations → plan + change. |
| `leaven-agentic-skill` | `parser.rs` | 126 | `SkillBankWorkspaceProposalParser` workspace → `SkillBankChange`. |
| `leaven-agentic-skill` | `diff.rs` | 111 | `SkillBankDiff` parent-vs-child. |
| `leaven-agentic-skill` | `materializer.rs` | 82 | `SkillBankMaterializer` writes bank into workspace at layout. |
| `leaven-gepa-agentic-skill` | `skill_reflector.rs` | 309 | `SkillBankReflector: ArtifactReflector` — `project`/`read_back` + `SkillPartScope`. |
| `leaven-gepa-agentic-skill` | `reflector.rs` | 210 | `GepaSkillBankAgenticReflector` glues GEPA reflect requests to skill-bank reflection through `ReflectionWorkspace` + `RunContext::propose`/`apply_batch`. |

This stack is reusable; the per-paper EvoSkill example does not have to rewrite it.

## 2. Pain Points

These are the surfaces that hurt today, with concrete pointers.

### 2a. Five-phase orchestration boilerplate is overwhelmingly hand-rolled

The `run_iteration` / `ensure_*` / `*Request` / `EvoSkillCheckpoint` chain is the biggest single chunk of EvoSkill-specific code in `p5_evoskill_iteration`.

- `ResumeState::from_checkpoint` is a 100-line match (`main.rs:1200-1306`) that just rewires every checkpoint variant back into the same set of `Option<T>` fields the next phase will read.
- Every `ensure_*` function has the same shape: short-circuit on resume → run the phase → checkpoint with every field of state → return one new piece of data. The checkpoint variant enum (`checkpoint.rs:10-72`) has five variants and each one re-lists 7-12 carried fields.
- Why painful: this is mechanical state-machine code that the engine could express as a sequenced stage graph with auto-checkpointing. The example author has to hand-write the resume short-circuits and field-by-field state pickling in Rust and keep them in sync.

### 2b. The agentic-proposer trait stack is heavy

The skill-builder phase requires implementing five traits to drive one agent call (`main.rs:1692-1934`): `Materializer`, `Renderer`, `ProposalRepairPromptBuilder`, `ProposalParser`, plus a wrapping `AgentRuntime` (`RecordingBuilderRuntime`) just to capture session evidence. That's ~140 lines of trait impls + ~80 lines of repair-prompt rendering. The proposer phase (`main.rs:1620-1684`) actually bypasses the trait stack and calls `runtime.run_session` directly because going through it for a single role costs more LOC than open-coding it.

- Why painful: the trait stack is shaped for the long-tail "engine-driven proposer with repair" case. For paper authors writing one Codex call per role, every trait is a syntactic tax. The decision to bypass it in `run_skill_proposer` is a tell: when the cost of the trait stack exceeds the value of trust/repair, paper authors will route around it and lose the benefits.

### 2c. Hand-rolled JSON contract for every agent call

`final_json` / `extract_json_object` / per-role `serde::Deserialize` structs (`main.rs:2022-2066`, plus `AgentAnswer`, `GeneratedSkill`, `GeneratedSkillFile`, `SkillProposal`) exist because the engine doesn't enforce the agent's output schema. The presenter writes "Reply with JSON only" prose, the agent may or may not comply, and the parser then has to defensively strip ``` fences, find `{`/`}` boundaries, and `serde_json::from_str`.

- Why painful: every role re-derives a fragile prose-contract for JSON output. The aspirational target (`evaluator_dspy_codex.v0.3.py:44`) does `output=lv.output.json_schema(JudgeResult)` — the schema is the contract, the boundary enforces it.

### 2d. Bespoke evidence storage for each role

`EvoSkillEvidence::AgentRoleSession { role, developer_instructions, session }` plus `RecordingBuilderRuntime` (wrapping the runtime just to call `evidence_store.put`) plus the proposer's manual `evidence_store.put` after parsing — each is paper-author code that says "make sure this session is durably recorded." The engine could auto-record per-stage sessions; today the paper has to thread an `&FileEvidenceStore<EvoSkillEvidence>` through every phase and wrap the runtime in a recording adapter when the trait stack doesn't.

- Why painful: it's the same logic repeated three times for three roles, with different mechanics each time (direct `put`, recording wrapper, returning EvidenceRef).

### 2e. Manual workspace lifecycle in non-trait-stack roles

`run_skill_proposer` shows the open-coded shape (`main.rs:1620-1672`): `factory.allocate(WorkspaceConfig::default())` → write meta-skill files → write task JSON → run session → parse → `finish_workspace` (which reconciles stage error with cleanup error). The `finish_workspace` reconciliation (`main.rs:2173-2183`) is necessary because cleanup can also fail and you can't lose either error.

- Why painful: every paper author writing a Leaven evaluator/proposer without the trait stack has to redo this exact lease/cleanup dance and the four-way error reconciliation.

### 2f. The paper-close harness is a giant write-and-validate refusal cascade

`p5_skill_paper_reproductions/src/evoskill.rs` is 9,351 lines of serializable schema types + sidecar I/O writers + fingerprint-bound merge-not-overwrite + role/source-id coverage validators. The shape of nearly every writer is identical: read CSV/JSONL → materialize sources → build manifest from sources → compute manifest+scorer fingerprints → derive expected slot keys → read existing sidecar → refuse if existing covers the same keys → merge → write → re-read and re-validate.

- Why painful: it is 80 schema types + 280 functions, and almost none of it is EvoSkill-domain logic. It is structured JSON I/O with strict validation that exists to prevent fake-proof regressions during no-spend bring-up. Almost every writer-function pair (officeqa-score / sealqa-judge-score; runner-inputs / runner-outputs; live-run-request) is duplicated because the schemas diverge.

### 2g. Mock/synthetic data plumbing for preflight

`write_preflight_report` (`main.rs:193-261`) requires the paper to invent a `synthetic_answer_for(case)` (`main.rs:1593-1598`) and a synthetic `AgentSession` so the scorer can be dry-run. The presenter dry-run, scorer dry-run, store dry-run, and checkpoint-store dry-run are all wired by hand because there's no `lv.preflight()` primitive.

## 3. What the Proposed Python SDK Would Remove

Mapped per pain point.

### 3a. Five-phase orchestration → eliminated for evaluator authors, still hand-rolled for optimizer authors

The aspirational Python (`evaluator_dspy_codex.v0.3.py`) shows an **evaluator** written as a single `async def evaluate(job, cx)` that iterates over `job.independent_cases()` and emits `cx.assessments.submit(...)`. There is no `RunContext`, no checkpoint enum, no resume short-circuit. The engine sits on the Rust side of the ACP wire; the Python evaluator is a stage callback.

- **Removed**: the entire `ResumeState`/`EvoSkillCheckpoint`/`ensure_*` machinery for the *evaluator role*. The five-variant checkpoint enum, the `from_checkpoint` matcher, the phase-by-phase `checkpoint_with_optimizer_state` calls — all replaced by engine-side stage sequencing because the Python decorator says "evaluator," and the engine knows how to checkpoint stage boundaries.
- **Removed**: the `OptimizationProblem` impl. Decorator metadata + typed records from JSON Schema codegen carry the case/artifact/evidence types over the wire.
- **Not removed**: if EvoSkill needs a custom **optimizer** (round-robin parent selector + category sampler + frontier admission + feedback history), and that optimizer is also expected to be written in Python, the same five-phase orchestration problem reappears one level up. The leaven-py-and-acp note's decorator surface includes `@lv.reflector` and `@lv.proposer` but says nothing about `@lv.optimizer` or `@lv.frontier_policy`; today those are Rust-only (`leaven-population::TopKParentSelector`, `leaven-eval::CategoryRoundRobinSampler`). If the SDK exposes them as parameter records the user composes (parent_selector="round_robin", frontier_k=3, sampler=...), then the Python user never writes the five-phase loop because the engine runs it. If the SDK requires the user to write a Python optimizer body, the loop returns.

### 3b. Agentic-proposer trait stack → eliminated by `@lv.proposer` decorator

The five Rust trait impls (`Materializer`, `Renderer`, `ProposalRepairPromptBuilder`, `ProposalParser`, runtime wrapper) collapse into one `@lv.proposer` async function whose signature is `(input, cx) -> Proposal`. The repair semantics become a decorator argument (`@lv.proposer(repair_attempts=2)`). Materialization, rendering, parsing, and evidence recording are the SDK's job because they are all wire-side concerns the engine already understands.

- **Removed**: `SkillBuilderMaterializer`, `SkillBuilderRenderer`, `SkillBuilderRepairPrompt`, `SkillBuilderParser`, `RecordingBuilderRuntime` — five trait impls totaling ~140 lines.
- **Caveat**: the actual *contract* still has to be parameterizable from Python — what file to read back, what change shape to emit, what counts as "invalid readback." The Python decorator helps only if the SDK exposes a typed `ProposalParser` interface (e.g., `cx.workspace.read_back_as(SkillBank)`) instead of forcing the user to drop down to raw workspace I/O. Without that, the user has to recreate the Rust `SkillBankReflector` (`crates/leaven-gepa-agentic-skill/src/skill_reflector.rs:1-310`) in Python.

### 3c. Hand-rolled JSON contract → eliminated by typed records + `lv.output.json_schema`

`evaluator_dspy_codex.v0.3.py:44` uses `output=lv.output.json_schema(JudgeResult)` where `JudgeResult` is a pydantic model. The Rust engine enforces the schema at the boundary; the Python evaluator receives a typed `agent.parsed` object. The fence-stripping, `find('{')`/`rfind('}')`, `serde_json::from_str`, and per-role `AgentAnswer`/`GeneratedSkill` structs disappear.

- **Removed**: `final_json`, `final_assistant_message`, `extract_json_object` (45 lines), plus the per-role JSON struct boilerplate.

### 3d. Bespoke evidence storage → eliminated by decorator metadata + `evidence=lv.EvidenceEnvelope`

The aspirational Python (`evaluator_dspy_codex.v0.3.py:58-62`) constructs `lv.EvidenceEnvelope.public_private(...)` as part of the `AssessmentWrite`; the engine handles the actual store call, fingerprinting, and EvidenceRef minting on the Rust side. The Python author never sees `FileEvidenceStore::put`.

- **Removed**: `EvoSkillEvidence` enum + manual `evidence_store.put` calls + `RecordingBuilderRuntime` wrapper. The trade is that the evidence shape becomes a typed envelope the SDK ships, with `public`/`private`/`data_classes` slots — which is what the public-seam spec already locks.

### 3e. Manual workspace lifecycle → eliminated by `cx.workspace.materialize_candidate(...)` + `lifetime="stage_call"`

`evaluator_dspy_codex.v0.3.py:27` shows `ws = await cx.workspace.materialize_candidate(item.candidate_id, surface="full_repo", lifetime="stage_call")`. The lease is auto-acquired and auto-cleaned at stage-call boundary. The four-way error reconciliation in `finish_workspace` is the engine's job.

- **Removed**: workspace allocate/cleanup/error-reconcile boilerplate (~25 lines per phase that hand-allocates).

### 3f. Paper-close manifest cascade → mostly NOT removed

This is the honest bad-news section. The 9,351-line `evoskill.rs` paper-close harness is not glue that a Python evaluator surface would touch. It is a no-spend bring-up substrate that exists because Leaven is still pre-live-run and needs refusal-capable sidecar pipes for source pinning, split policy, scorer replay, judge requests, runner-input materialization, etc. None of the decorator surface in `leaven-py-and-acp-transport.md` addresses sidecar generation, fingerprint binding, role/source-id coverage validation, or refusal-capable merge semantics.

- **Removed**: arguably none. The schemas might be code-generated from JSON Schema (the `leaven-types` Python package idea), which would let the Python harness build manifest objects with typed records instead of hand-rolled structs. That collapses the ~80 schema-type definitions into one import statement.
- **Not removed**: the 280 internal functions doing source materialization, CSV parsing, fingerprint computation, slot-key derivation, validator wiring, and merge-not-overwrite semantics. Those are domain logic, not Rust ceremony. They have to live somewhere — moving them to Python is a relocation, not an elimination.
- **Caveat for honesty**: most of `evoskill.rs` is paper-close *bring-up* code, not live-execution code. Once the paper-close gates are proven and a live run is approved, the relevant code path collapses dramatically (the live path is just `evaluate(candidate, validation_split)` → `score`). The 9,351 lines exist because we are validating-without-spending. A Python SDK that ships after paper-close is reached would face a much smaller surface here.

### 3g. Mock/synthetic preflight → partially removed

If the SDK gives `lv.preflight()` as a first-class operation that auto-runs presenter/scorer/store/checkpoint with engine-side synthetic data, the 70-line `write_preflight_report` collapses to one call. The synthetic-answer logic still has to come from somewhere (the Rust scorer needs valid input), but the engine can derive synthetic inputs from case schemas without paper-side help. Not in the current decorator sketch; would need to be added.

## 4. What Would NOT Be Removed

Load-bearing complexity that survives the Python SDK migration:

1. **Engine-side capability minting, trust scopes, and data-class enforcement**. The `trust_profile="managed_sandbox"`, `input_classes=[...]`, `forbidden_input_classes=[...]` annotations in the aspirational Python are Python-side *declarations*; the actual capability tokens, read-receipt issuance, and effect-receipt accounting happen in Rust on the engine side. None of that moves. (See `docs/specs/public-seam-v1/02_capability_tokens_spec_v0.3.md` for what stays.)
2. **RunGraph mutation**. `ctx.propose` / `ctx.apply_batch` / `ctx.record_proposal_batch` / `ctx.evaluate_with` are not exposed at the Python decorator surface in the spec; the engine still owns graph state and admission. Python authors emit `AssessmentWrite` / `Proposal` envelopes; the engine applies them through `RunContext` on the Rust side. (`crates/AGENTS.md:54` keeps "graph mutation stays in `leaven-engine` through `RunContext`.")
3. **Optimizer strategy state**. `TopKFrontier`, `TopKParentSelector`, `CategoryRoundRobinSampler`, `SkillUtilityState`, `SkillUtilityRanker` live in `leaven-population` and `leaven-eval`. These are the *interesting* part of EvoSkill (round-robin vs. best, K=3 frontier, paired rollouts, EMA utility). The Python decorator surface lets users compose stages, not write new optimizer strategies. If a paper invents a new optimizer rhythm (e.g., D2Skill's paired rollout grouping), today that requires either a new Rust crate or a paper-specific composition of existing primitives. The SDK does not change that.
4. **Workspace backend lifecycle setup**. `LocalWorkspaceFactory::new(...)` + `WorkspaceConfig::default()` + factory plumbing into the evaluator stack still happens engine-side. The Python user gets `cx.workspace`, but a maintainer extending the workspace stack (Git, Firkin, Tart) edits the Rust crate.
5. **Skill-bank artifact semantics**. `SkillBank` validation, `SkillBankChange` variants, `SkillBankDiff`, `SkillBankWorkspaceProposalParser`, `SkillBankReflector` — the reusable skill-bridge stack in `crates/leaven-agentic-skill` and `crates/leaven-gepa-agentic-skill` (~2,903 LOC together) does not move. It is paper-neutral substrate that Python evaluators consume; it does not become Python code.
6. **Source materialization, split policy, scorer replay** for paper-close. As covered in §3f, this is domain logic, not Rust ceremony.
7. **Public-seam schema fidelity**. `docs/specs/public-seam-v1/` locks the wire contract for plan IR, capability tokens, result receipts, stage payloads, evidence envelopes, evaluator/judge requests, and the ACP profile. The Python SDK rides on top of that contract; bugs in capability projection or receipt issuance are Rust-side bugs.
8. **Cache identity, fingerprint policy, durable resume**. The whole `Fingerprint`-and-`CachePolicy` substrate (used in `main.rs:175-187`, `222`, `1134`) stays Rust-side. Python authors get a stage-id-keyed cache transparently; they don't construct fingerprints.

## 5. Verdict

The proposed Python SDK would meaningfully reduce **evaluator and proposer authoring** for EvoSkill-shaped paper repros — concretely, it kills the five-phase orchestration boilerplate, the trait-stack ceremony, the JSON contract plumbing, the bespoke evidence storage, and the workspace lifecycle dance, which together account for roughly **1,500-2,000 of the ~2,400 lines** in `p5_evoskill_iteration/src/main.rs`. That is real and material.

It would **not** meaningfully reduce **paper-close bring-up** (the 9,351-line `p5_skill_paper_reproductions/src/evoskill.rs`), which is dominated by source materialization, refusal-capable sidecar I/O, fingerprint validation, and runner-input/output bridges — domain work that has to live somewhere regardless of host language. It would also **not** unblock cases where the paper invents a new optimizer rhythm (round-robin selection, paired rollouts, frontier capacity, EMA utility), because optimizer strategy state remains a Rust extension point. EvoSkill itself is mostly OK here because its rhythm is already covered by `TopKFrontier` / `TopKParentSelector` / `CategoryRoundRobinSampler` — but D2Skill's paired rollouts, Memento-Skills' contrastive router, and SkillReducer's ddmin are not, and Python won't help.

Net: the SDK absolutely earns its keep if the value claim is "200-line Python evaluators for paper-close *evaluator/proposer* authoring." It does not earn its keep if the value claim is "200-line Python *paper replication*," because the load-bearing weight of bring-up has moved from agent-call orchestration to source/split/score plumbing, and Python doesn't move that needle.

## 6. Open Questions

1. **Does `@lv.optimizer` exist?** The decorator list in `leaven-py-and-acp-transport.md:58-61` includes `evaluator/reflector/proposer/runner/scorer/judge`. It is silent on optimizer strategy. If EvoSkill-shaped papers need to express custom selector/sampler/frontier policy from Python, the SDK story is incomplete. If they only need to *configure* existing Rust primitives (paper-config-only path), it is sufficient but should be stated.
2. **What is the typed-record surface for `SkillBankChange`, `SkillBank`, `GitProgramChange`, `AgentTrajectoryEvidence`?** The codegen story (`leaven-types` from JSON Schema) is mechanical for the public-seam contract; it is not mechanical for the artifact crates whose changes the optimizer admits. If Python evaluators have to construct artifact changes by hand-writing JSON dicts, the "typed boundary" promise leaks.
3. **How does the paper-close harness's refusal cascade lower to ACP?** The 280 internal validators in `evoskill.rs` enforce manifest-vs-sidecar coverage, fingerprint binding, source-id matching, etc. If the Python SDK exposes only `cx.assessments.submit(...)`, who runs the validator? If the validator stays Rust-side, every new paper-close acceptance gate requires a Rust release. If it moves Python-side, we lose the refusal-capable type-driven guarantees.
4. **`leaven-pop` / `leaven-eval-py` reach?** Currently `TopKFrontier::observe`, `TopKParentSelector::select`, `CategoryRoundRobinSampler::next_batch` are Rust-only. If the Python SDK does not project these as call-through APIs, paper authors who want to *inspect* frontier state (which several EvoSkill ablations require) have to read engine reports and reconstruct state. Not in the decorator sketch.
5. **What is the per-iteration checkpoint shape over ACP?** The current `EvoSkillCheckpoint` enum (`checkpoint.rs:10-72`) carries 12 fields at its widest variant including `SkillBank`, `Vec<EvoSkillCase>`, `TopKFrontier`, `TopKParentSelector`, `CategoryRoundRobinSampler`, parent `CandidateId`, failure list, proposal, evidence ref, and `SkillBankChange`. If the engine auto-checkpoints stage boundaries on the Rust side, the Python evaluator never sees this — but if Python optimizers exist, they need a typed checkpoint surface. The locked seam spec covers result receipts but not optimizer-private state schemas. Unproven whether the wire contract is sufficient.
6. **How much does paper-close shrink after live-run approval?** Most of `p5_skill_paper_reproductions/src/evoskill.rs` exists to validate-without-spending. A measurement of "live-mode-only LOC" for an analogous paper-close harness post-approval would sharpen the §3f / §5 verdict; currently unproven.
