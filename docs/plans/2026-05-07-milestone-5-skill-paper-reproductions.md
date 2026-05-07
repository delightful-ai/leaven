# Milestone 5: Skill Paper Reproductions

Status: execution plan for reproducing the five selected skill-optimization
papers through Leaven primitives.
Date: 2026-05-07.

This milestone sits on top of the milestone examples contract:

- P0: graph skeleton
- P1: scalar keep-best
- P2: pairwise tournament
- P3: GEPA over an edit surface
- P4: workspace materialization and fresh authored artifacts

P5 is the pressure test: the library is not done until the five papers below
can be expressed as small, deterministic Leaven reproductions before any live
agent/provider smoke is attempted.

## Flywheel Control

Flywheel control node:

```text
1e79e9b6-6e70-5024-95a2-68effe2d7e21
damp-fog-8583
```

Budget contract: local machine and already-configured local credentials only.
No Flywheel managed-compute lease or paid external compute is acquired without
an explicit budget ceiling.

Local source bundle:

```text
tmp/skill_opt_sources/
```

Each paper directory contains raw arXiv source, flattened TeX, Markdown,
all-source concatenation, and Paperclip full text/metadata.

## Reproduction Targets

### EvoSkill (`arx_2603.02766`)

Load-bearing loop:

- analyze failures,
- choose create-vs-edit for a skill folder,
- materialize a structured skill artifact,
- validate on held-out cases,
- admit only frontier-improving candidates.

Leaven pressure:

- `ProposalEffect::Create` vs `ProposalEffect::Change`
- skill-directory artifact surface
- failure evidence attribution
- population admission policy
- materializer/workspace bridge for agent-readable skill folders

Minimal deterministic reproduction:

- seed skill registry with one weak skill,
- replay two fixed failure traces,
- propose one edit and one created skill,
- evaluate both against a held-out fixture,
- admit only the improving candidate.

### Trace2Skill (`arx_2603.25158`)

Load-bearing loop:

- analyze many trajectories in parallel,
- extract local lessons,
- hierarchically consolidate them,
- produce one transferable skill directory.

Leaven pressure:

- batch proposal rhythm over many evidence refs
- `AttributableEvidence<CaseId>` and trace-derived notes
- aggregate `ProposalEffect::Create` with `CausalInputs::NAry`
- materializer for consolidated skill output

Minimal deterministic reproduction:

- fixture contains success and failure traces,
- analyst stage emits lesson evidence per trace,
- consolidation proposer creates a new skill artifact from all lessons,
- evaluator verifies that consolidated skill passes cases that raw retrieved
  lessons fail.

### Memento-Skills (`arx_2603.18743`)

Load-bearing loop:

- maintain a persistent skill library,
- route by current stateful prompt,
- write new or edited skills after experience,
- roll back when validation fails.

Leaven pressure:

- skill routing policy
- optimizer-driven population/library observation
- proposal validation failures as learning signal
- rollback/admission separate from graph append-only truth

Minimal deterministic reproduction:

- run two episodes against a toy task,
- route to the best skill by state,
- generate an edit after failure,
- accept the edit only if replay validation improves,
- keep failed edits in graph as rejected attempts without mutating live library
  state.

### D2Skill (`arx_2603.28716`)

Load-bearing loop:

- maintain task-level and step-level skill banks,
- run paired baseline and skill-injected rollouts,
- derive utility from the performance gap,
- update/retrieve/prune dynamically.

Leaven pressure:

- paired evaluation request or paired assessment metadata
- `AttributableEvidence<SkillId>` for utility attribution
- two-granularity skill surfaces
- utility-aware population update and pruning

Minimal deterministic reproduction:

- evaluate the same candidate with no skill and with a selected skill,
- compute utility from the paired gap,
- update task and step skill scores,
- prune a low-utility skill while preserving graph evidence.

### SkillReducer (`arx_2603.29919`)

Load-bearing loop:

- compress routing descriptions,
- restructure body content into progressive disclosure,
- validate faithfulness and functional behavior,
- self-correct when a reduction breaks behavior.

Leaven pressure:

- skill body surface with routing/body/reference parts
- transformation proposals over selected parts
- behavioral gate before admission
- materialized skill layout with on-demand references

Minimal deterministic reproduction:

- seed a verbose skill folder,
- propose routing compression and body restructuring,
- evaluate token reduction plus behavior preservation,
- reject a destructive compression and accept a faithful one.

## Shared Primitives To Implement Before Paper-Specific Logic

1. `Materializer` hard cutover from `WorkspaceRenderer`.
2. `WorkspacePath` and backend-neutral workspace file APIs.
3. Pairwise evidence and tournament population.
4. Casewise evidence and minimal Pareto/frontier admission.
5. Minimal GEPA rhythm over `EditSurface`.
6. Skill registry artifact and surfaces:
   - routing description,
   - skill body,
   - references/assets,
   - validation contract metadata.
7. Skill-library policies:
   - route,
   - choose hard cases,
   - choose skill target,
   - admit/edit/prune/rollback.
8. Deterministic agent-runtime fixture before live Codex app-server smoke.

## Codex App-Server Smoke

Only after deterministic reproductions pass:

- use the local Codex app-server path,
- run real-agent smoke through `leaven-agent-codex`,
- use model `gpt-5.4-mini`,
- use reasoning effort `low`,
- keep the smoke small enough to isolate Leaven wiring from model quality.

## Done Criteria

- `just milestone-examples` passes.
- Five P5 reproduction examples or scenario tests pass.
- Each paper has a checked mapping from paper concepts to Leaven primitives.
- `just check` passes or any remaining blocker names the exact missing external
  credential, dataset, or provider capability.
