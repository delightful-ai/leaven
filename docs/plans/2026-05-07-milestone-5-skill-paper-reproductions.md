# Milestone 5: Skill Paper Reproductions

Status: first live EvoSkill iteration implemented in
`examples/p5_evoskill_iteration`; remaining papers are still reproduction
targets, not implemented examples.
Date: 2026-05-07.

P5 is the pressure test for whether Leaven is actually an agentic optimization
library. Toy skill registries, hardcoded paper outputs, fake proposal
application, and fake evidence stores do not count.

The first accepted gate is one full EvoSkill-shaped iteration:

```text
empty SkillBank
  -> executor agent fails train case
  -> proposer agent diagnoses failure and proposes create/edit
  -> skill-builder agent returns a real Agent Skills folder
  -> Leaven writes/parses/validates the folder through SkillBank primitives
  -> RunContext records and applies the proposal
  -> evaluator agent scores the child on validation
  -> population/admission state updates
  -> evidence and checkpoint records persist enough to resume
```

Runnable proof:

```bash
PATH="$HOME/.bun/bin:$PATH" LEAVEN_CODEX_LIVE=1 just milestone-p5
```

The live run must use Codex app-server with `gpt-5.4-mini`, low reasoning, and
developer instructions. The current one-iteration fixture uses final-message
JSON contracts for the agent outputs because the local app-server shell path was
not reliable enough for this proof; that is a stage choice, not a weakening of
the Leaven substrate. Tool-using Codex runs remain provider-adapter and future
paper-harness work.

## Source Grounding

Local source bundle:

```text
tmp/skill_opt_sources/
```

EvoSkill source inputs used by the P5 live gate:

- executor prompt:
  `~/vendor/github.com/sentient-agi/EvoSkill/src/agent_profiles/base_agent/prompt.txt`
- proposer prompt:
  `tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/proposer_placeholder.md`
- skill-builder prompt:
  `tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/skill_builder_placeholder.md`

The proposer and builder fixture files remove only the outer Python triple-quote
markers from the paper source prompt files. The runtime developer instructions
prepend no new paper logic; they append a Leaven/Codex wrapper that defines:

- skill mount path: `.agents/skills/<skill-name>/SKILL.md`
- no-shell final-message JSON contract for this live gate
- mandatory valid `SKILL.md` with `name`, `description`, and non-empty body
- repair feedback for invalid skill mutations

This means P5 is source-prompt faithful for the roles, but it is not yet a full
OfficeQA/SealQA reproduction.

## Implemented P5 Gate

Crate:

```text
examples/p5_evoskill_iteration
```

Fixture:

```text
examples/p5_evoskill_iteration/fixtures/treasury-notation/cases.json
```

The fixture is deliberately tiny. It forces the same semantic shape as EvoSkill:
the base executor has no relevant skill, fails a specialized reusable conversion
task, and then the proposer/builder produce a reusable skill folder that improves
held-out validation.

Generic Leaven primitives exercised:

- `SkillBank`, `SkillFolder`, `SkillBankChange`, and Agent Skills validation
- `SkillBankMaterializer` for agent-readable skill folders
- `SkillBankWorkspaceProposalParser` for workspace readback into typed changes
- `RunGraph` and `RunContext` for candidate insertion, proposal recording, and
  proposal application
- `TopKFrontier` population update over scalar validation evidence
- `FileEvidenceStore<EvoSkillEvidence>` for durable evaluation and agent-session
  evidence
- `FileCheckpointStore` for resumable phase checkpoints
- `CodexAppServerRuntime<StdioCodexAppServerConnector>` as the provider runtime

Paper-specific code owned by the example:

- EvoSkill role prompts and wrapper instructions
- the tiny treasury-notation dataset
- the multi-tolerance numeric scorer
- the one-iteration checkpoint enum
- parsing of Codex final-message JSON into paper-specific proposal/build schemas

## What This Proves

The live P5 gate proves that Leaven can run a real agentic skill optimization
iteration without pretending the optimizer is just a script:

- the agent creates a valid skill folder, not a fake local skill type;
- invalid skill folder output would route back through the same builder attempt;
- successful skill changes enter the graph only through `RunContext`;
- evidence is stored as product data, not just printed logs;
- resume can detect a completed iteration and avoid rerunning Codex;
- Codex provider details stay in the provider adapter and example wiring, not in
  `leaven-core`, `leaven-engine`, or `SkillBank`.

## What This Does Not Prove Yet

This is not a 1:1 full-paper reproduction.

Remaining EvoSkill gaps:

- full OfficeQA and SealQA task loaders
- paper train/validation/test splits
- full frontier size and parent-selection loop over many iterations
- feedback-history accumulation across accepted and rejected proposals
- real OfficeQA document/tool harness
- paper auto-grader behavior
- edit-path reproduction against an existing non-empty skill bank
- ablations for create/edit, feedback history, and skill-builder meta-skill

Remaining Codex/product gaps:

- tool-using executor runs through app-server in the paper harness
- provider-native Codex skill references, if needed
- shell/tool transcript capture in the P5 paper path
- container-local app-server connector for non-local workspaces

## Reproduction Targets

### EvoSkill (`arx_2603.02766`)

Load-bearing loop:

- executor agent uses the current skill bank;
- proposer analyzes failures and feedback history;
- skill-builder materializes a concrete Agent Skills folder;
- validator scores held-out cases;
- candidate enters the frontier only if it beats the weakest frontier member.

Current status:

- first live one-iteration gate: implemented;
- full paper setup: not yet implemented.

Next real step:

- replace the treasury fixture with an OfficeQA subset and preserve the same
  Leaven substrate. Any missing generic primitive discovered there should move
  into Leaven, not into the example.

### Trace2Skill (`arx_2603.25158`)

Load-bearing loop:

- analyze many trajectories in parallel;
- extract local lessons;
- hierarchically consolidate them;
- produce one transferable skill directory.

Expected Leaven pressure:

- batch proposal rhythm over many evidence refs;
- `AttributableEvidence<CaseId>` and trace-derived notes;
- aggregate `ProposalEffect::Create` with `CausalInputs::NAry`;
- materializer/parser support for consolidated skill output.

Status: not implemented.

### Memento-Skills (`arx_2603.18743`)

Load-bearing loop:

- maintain a persistent skill library;
- route by current stateful prompt;
- write new or edited skills after experience;
- roll back when validation fails.

Expected Leaven pressure:

- skill routing policy;
- optimizer-driven population/library observation;
- proposal validation failures as repair/admission signal;
- rollback/admission separate from graph append-only truth.

Status: not implemented.

### D2Skill (`arx_2603.28716`)

Load-bearing loop:

- maintain task-level and step-level skill banks;
- run paired baseline and skill-injected rollouts;
- derive utility from the performance gap;
- update/retrieve/prune dynamically.

Expected Leaven pressure:

- paired evaluation request or paired assessment metadata;
- `AttributableEvidence<SkillName>` for utility attribution;
- two-granularity skill surfaces;
- utility-aware population update and pruning.

Status: not implemented.

### SkillReducer (`arx_2603.29919`)

Load-bearing loop:

- compress routing descriptions;
- restructure body content into progressive disclosure;
- validate faithfulness and functional behavior;
- self-correct when a reduction breaks behavior.

Expected Leaven pressure:

- manifest/body/reference surfaces;
- transformation proposals over selected parts;
- behavioral gate before admission;
- materialized skill layout with on-demand references.

Status: not implemented.

## Done Criteria

P5 is complete only when:

- `just milestone-p5` runs a live Codex EvoSkill iteration with
  `gpt-5.4-mini` low;
- the run produces durable evidence records and checkpoints under `tmp/`;
- rerunning the command resumes from the complete checkpoint instead of paying
  for another Codex iteration;
- `just check` passes, or the remaining blocker is named with exact failing
  command output;
- no rejected toy paper examples remain in the workspace.

Full paper-reproduction completion is separate:

- EvoSkill must be scaled from the live fixture to the paper task setup;
- Trace2Skill, Memento-Skills, D2Skill, and SkillReducer each need their own
  separate examples using real Leaven primitives;
- examples may own paper-specific prompts, datasets, scorers, and harnesses, but
  must not reimplement generic skill, graph, evidence, workspace, runtime,
  checkpoint, repair, or population substrate.
