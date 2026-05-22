# Leaven Agentic Trace Reflection Product Backend

Date: 2026-05-21
Status: implementation spec; active successor for the Git/repo portions of
`agentic_reflection.md`.
Governing spec: `docs/specs/initial_library.md`.
Companions:
`docs/specs/agentic_reflection.md`,
`docs/specs/agentic_stage_materialization.md`,
`docs/specs/gepa_reference_behavior.md`,
`docs/specs/gepa_reflection_evidence_visibility.md`,
`docs/specs/per_case_assessment_rows.md`,
`docs/specs/firkin_git_workspace_backend.md`,
`docs/specs/firkin_git_workspace_api_shape.md`,
`docs/specs/agentic_skill_optimization_primitives.md`,
`docs/working-memory/skill-paper-replication.md`.

## 0. Denominator

The Git workspace backend slice is no longer the blocker. Current code owns:

- `crates/leaven-artifact-git`: `GitProgramArtifact`,
  `GitProgramChange`, repo revisions, layout, lineage, and typed apply law.
- `crates/leaven-agentic-git`: `GitProgramMaterializer` and
  `GitProgramReadback`, including explicit patch/bundle import and checkout
  fallback into typed `GitProgramChange`.
- `crates/leaven-workspace-firkin`: product-pod workspace backend contract and
  signed Apple/VZ live proof path.
- `crates/leaven-gepa`: graph-connected GEPA reflection, reflective examples,
  per-case rows, frontier/admission state, checkpoint state, and run events.
- `crates/leaven-gepa-agentic-skill`: the precedent bridge from GEPA
  reflection to a materializing agentic proposer.

The remaining product-backend gap is the integration loop:

```text
GEPA selects parent + reflective examples
  -> GitProgramArtifact parent is materialized into an agent workspace
  -> agent edits checked-out repos or writes patch/bundle proposal output
  -> readback yields GitProgramChange
  -> RunContext::propose records ProposalEffect::Change
  -> apply_batch admits or rejects the child through optimizer policy
  -> evidence, trace, report, and checkpoint state preserve the attempt
```

That loop is the first real "agentic trace reflection system" denominator for
repo-shaped programs. The signed Firkin proof makes it possible; it is not
itself the product proof.

## 1. Product Promise

A Leaven user can run a GEPA/EvoSkill-shaped optimization where the candidate
program is a `GitProgramArtifact`. Each reflection attempt is a real agentic
proposal over a disposable workspace projection of the current candidate. The
agent's output becomes a typed artifact change, not a workspace side effect or
private backend mutation. The run graph remains the only source of candidate
truth.

The ordinary proof path must show all of these together:

- parent selection and reflective example construction happen in GEPA;
- the agent sees the parent repo contents, not only metadata;
- readback imports the child revision into durable Git storage;
- graph mutation goes through `RunContext::propose` and `apply_batch`;
- score/admission metadata records whether the child entered the frontier;
- checkpoint/report state names parent, child, proposal, case rows, and trace
  or session evidence enough to resume and inspect the run.

## 2. Hard Boundaries

- `leaven-artifact-git` owns Git identity, revisions, layout, lineage, and
  typed changes. It must not allocate workspaces, run git commands for a stage,
  decide frontier admission, or know Firkin.
- `leaven-agentic-git` owns Git-program materialization/readback. It may turn
  workspace edits, patches, or bundles into typed changes. It must not own
  GEPA search policy, scoring, graph admission, or provider protocol.
- A GEPA/Git bridge should mirror the `leaven-gepa-agentic-skill` pattern. If a
  new crate is added, its home is `crates/leaven-gepa-agentic-git`; it owns
  only the adapter between `ReflectRequest`, `AgenticProposer`, and
  `GitProgramMaterializer`/`GitProgramReadback`.
- `leaven-workspace-firkin` remains workspace substrate. It must not know
  candidate identity, hidden targets, graph admission, or paper scoring.
- `leaven-gepa` owns optimizer rhythm. It must not learn Git-specific file
  layout, Firkin layout, EvoSkill prompts, or provider-specific agent protocol.
- `RunContext::propose` plus `apply_batch` remain the final graph mutation
  path. A workspace commit, patch, bundle, or backend import is not graph truth
  until it becomes a proposal and is applied.

No compatibility shim or duplicate route is allowed. If an old reflection path
is retained during construction, it must be marked scaffold at the owning
surface and removed or demoted by the hard cutover.

## 3. Required Loop Shape

The Git bridge input carries the parent artifact value and the already-built
GEPA request:

```text
GitProgramGepaReflectionInput {
  parent_artifact: GitProgramArtifact,
  request: ReflectRequest<Part>,
}
```

The bridge does not rebuild reflective examples. GEPA builds them once and
passes the same `ReflectRequest` to the configured reflector, preserving the
existing LM/agent parity law.

Materialization writes the Git program layout into the workspace and writes
reflection context beside it. The agent-visible context must include:

- checked-out repo paths from `GitProgramLayout`;
- selected part identity when a surface can name one;
- target-safe reflective examples;
- explicit output contract for patch, bundle, or committed workspace change;
- no hidden test targets or evaluator-only artifacts.

Readback yields either no proposal or one typed proposal batch whose effect is
`ProposalEffect::Change(GitProgramChange)`. The proposal's provenance must
include the `ReflectRequest` source refs and the agent attempt/session evidence
when available.

## 4. The Two 1:1 Denominators

### 4.1 Product-backend 1:1

The implementation must be faithful to Leaven's architecture: artifact,
workspace, agent runtime, proposal, graph admission, evaluation, report, and
checkpoint are separate concepts with one final mutation path. A passing
workspace smoke, topology test, or fake runtime alone is not enough.

### 4.2 EvoSkill paper 1:1

EvoSkill remains the first paper pressure target, but this spec does not claim
OfficeQA/SealQA replication. Full EvoSkill still requires paper datasets,
splits, prompts, scorers, provider/model pins, epochs, frontier capacity,
feedback-history schedule, skill-merge condition, held-out reports, and
ablations. Until those exist, P5 and GitProgram tests are product-backend
proofs or mechanics smokes, not paper-parity claims.

## 5. Acceptance Path

1. Add the GEPA/Git bridge with the same build-once-pass-down reflection law as
   `leaven-gepa-agentic-skill`.
2. Prove a deterministic no-spend GitProgram reflection attempt:
   seed parent repo, fake agent edit or patch/bundle output, read back a
   `GitProgramChange`, apply through `RunContext`, and assert child admission
   metadata.
3. Thread attempt evidence into the durable report/checkpoint shape. Reuse
   existing GEPA attempt/event state when it is sufficient; add a typed envelope
   only for facts that cannot be represented today.
4. Add a P5/EvoSkill-shaped no-spend scenario over a tiny GitProgram fixture.
   It must report actual frontier parent/child truth and score/admission state.
5. If Firkin behavior is touched, run the signed live script with the Alpine
   git template image. Otherwise keep the live proof as an unchanged backend
   precondition.
6. Update `docs/working-memory/skill-paper-replication.md` with the new
   substrate state and the remaining paper blockers.
7. Finish with focused crate/example tests and `just check`.

## 6. Proxy Traps

Do not count any of these as completion:

- another local workspace backend proof;
- mounting host bare stores or durable object paths into proposer workspaces;
- a fake agent that emits a proposal without seeing repo contents;
- a patch/bundle import test with no `RunContext::propose` admission;
- a GEPA run over a non-Git artifact;
- a report that omits parent revision, child revision, proposal id, or
  admission outcome;
- an EvoSkill tiny fixture presented as OfficeQA or SealQA parity;
- a live Firkin smoke that does not run the optimizer/reflection/admission
  loop.

## 7. Verification

Current narrow gates:

```text
cargo test -p leaven-agentic-git
cargo test -p leaven-gepa --test gepa_smoke --test agent_stage_routing
cargo test -p p5_evoskill_iteration
```

Once the bridge crate lands, add its owning crate test to the same narrow gate:

```text
cargo test -p leaven-gepa-agentic-git
```

If a new crate, dependency, facade route, or topology entry is added:

```text
cargo test -p leaven --test topology_contract
```

If Firkin runtime code changes:

```text
LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE=docker.io/alpine/git:latest \
  CARGO_TARGET_DIR=/tmp/leaven-firkin-target \
  CARGO_BUILD_JOBS=1 \
  scripts/run-signed-live-firkin-git-workspace-test.sh
```

Completion gate:

```text
just check
```
