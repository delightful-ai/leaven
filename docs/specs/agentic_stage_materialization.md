# Leaven v0.2.9 - Agentic Stage Materialization

> Status: implementation spec.  
> Date: 2026-05-13.  
> Governing spec: `docs/specs/initial_library.md`.  
> Runtime companion: `docs/specs/agentic_stage_runtime.md`.  
> Task-workload companion: `docs/specs/agentic_task_execution_substrate.md`.

This spec reconciles the stage-materialization plan with the current product
vocabulary. It is the durable target for optimizer-stage agent workspaces; the
dated plan under `docs/plans/` is execution order and provenance.

## Authority

This file is the only authoritative agentic-stage materialization spec path.
Use this spelling:

```text
docs/specs/agentic_stage_materialization.md
```

Do not create or route work through misspelled duplicate paths. The older
goal-state draft was folded into this implementation spec and the dated plan at
`docs/plans/2026-05-13-leaven-agentic-stage-materializer.md`; any remaining
full-detail snippets in that plan are provenance and implementation guidance,
not a second spec.

## Boundary

There are three separate surfaces:

- `AgentCase`, `CaseSuite`, `AgentWorkload`, and `AgentCaseEvaluator` are
  candidate-evaluation workload vocabulary. They own case input, hidden scorer
  targets, case files, setup requirements, run policy, scoring, and evaluator
  records.
- `AgentStagePlan`, `AgentBacked`, `StageReadAuthority`, and
  `StageAttemptReceipt` are optimizer-stage workspace vocabulary. They own a
  bounded workspace for one optimizer deliberation, scoped graph/evidence reads,
  agent output parsing, mandatory receipt recording, and finalization through
  `RunContext`.
- `Workspace`, `WorkspaceView`, `WorkspaceFactory`, `WorkspaceSlot`, and
  `WorkspacePath` are raw workspace substrate. They know files, commands,
  leases, cleanup, factory context, and path containment. They do not know
  cases, proposals, assessments, GEPA, or agent semantics.

An optimizer-stage workspace must not depend on `AgentCase`. A candidate
evaluation stage may use `AgentCase`, but graph mutation still happens only
through typed stage outputs and `RunContext`.

## Required Flow

The product path is:

```text
optimizer builds typed stage request
  -> AgentBacked<ProposerSlot<Req>, Runtime, Bootstrap, Parser>
  -> AgentStageBootstrap builds AgentStagePlan<Req>
  -> setup_stage_workspace writes plan-derived files
  -> StageReadAuthority handles prewarm and requested queries
  -> AgentRuntime executes in the scoped workspace
  -> StageOutputParser reads declared output entries
  -> ProposalBatch
  -> RunContext::propose records the batch
  -> RunContext::apply_batch applies candidates
  -> StageAttemptReceipt / StageAttemptRecorded / cleanup
```

`StageReadAuthority` is the only entry point from scoped graph/evidence queries
to query-derived workspace entries. The stage crate must not receive an
unscoped `RunGraphView`.

## Naming

Use these names for the optimizer-stage surface:

```text
setup_stage_workspace
StageQueryPolicy { allowed, prewarm, caps }
AllowedQuerySet
StageQuery
WorkspaceEntry
WorkspaceEntryReceipt
StageAttemptReceipt
StageReadAuthority
AgentBacked<Slot, Runtime, Bootstrap, Parser>
SlotMarker<P>::Output
```

Do not introduce compatibility names such as `EagerMaterializationPolicy`,
`QueryPolicy`, `MaterializationEntry`, `MaterializedEntryReceipt`,
`StageReceipt`, `ScopedStageSource`, or `materialize_stage_workspace`.

## Proof Denominator

Promotion requires behavior tests for:

- hidden-target presenter instruction and workspace non-leakage in
  `leaven-agentic`;
- workspace id, factory context, slot containment, command cwd scoping, and
  tree fingerprints in `leaven-workspace`;
- `StageAttemptRecorded` success and error events in `leaven-engine`;
- stage crate dependency shape, serde round trips, output contracts, query
  policy, setup receipts, parser behavior, and read authority;
- `AgentBacked<ProposerSlot>` with a fake runtime whose randomized output bytes
  become an applied candidate through `RunContext::propose` and
  `RunContext::apply_batch`;
- GEPA feedback/routing through the stage proposer path;
- `leaven_query` parsing and JJ materializable artifact prerequisites.

Fixed-edit GEPA fixtures, compile-only crates, and examples that do not execute
the production stage path are proxy proof only.

## Prerequisite Readiness

As of the `agentic-stage: satisfy workspace-wide completion gate` jj slice, the
stage substrate is ready for a focused implementation goal, with explicit
scaffolding still outside completion proof:

- The stage/workload/workspace boundary is encoded in crate `AGENTS.md` files
  and in the `leaven-stage`, `leaven-agentic`, and `leaven-workspace` APIs.
- `leaven-stage` exists and owns `AgentStagePlan`, `AgentBacked`,
  `StageReadAuthority`, `StageAttemptReceipt`, `StageQueryPolicy`,
  `setup_stage_workspace`, output contracts, parsers, receipt stores, and the
  `ProposerSlot` adapter.
- `RunContext::propose` can carry a receipt-backed `StageAttemptRecorded` event,
  and the fake-runtime proof applies agent-written proposal bytes through
  `RunContext::apply_batch`.
- `leaven_query` parsing, help text, read-authority query execution, and
  workspace installation of an executable help shim are scaffolded and tested;
  runtime-integrated agent-requested shell queries remain follow-on work.
- JJ artifact materialization is a scaffold with deterministic workspace-file
  tests. It is not live `jj` command execution or apply semantics.
- GEPA has request/bootstrap routing into the stage vocabulary, but the full
  optimizer switch away from fixed-edit reflection remains follow-on work.
- Setup/runtime/parse error paths now persist failed attempt receipts; workspace
  allocation and pre-plan serialization failures still occur before a workspace
  receipt can exist.
