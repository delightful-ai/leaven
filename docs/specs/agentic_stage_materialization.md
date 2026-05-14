# Leaven v0.2.9 - Agentic Stage Materialization

> Status: implementation spec.  
> Date: 2026-05-13.  
> Governing spec: `docs/specs/initial_library.md`.  
> Runtime companion: `docs/specs/agentic_stage_runtime.md`.  
> Task-workload companion: `docs/specs/agentic_task_execution_substrate.md`.

This spec reconciles the stage-materialization plan with the current product
vocabulary. It is the durable target for optimizer-stage agent workspaces; the
dated plan under `docs/plans/` is execution order and provenance.

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
