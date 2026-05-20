## Boundary
This crate owns skill-specific helpers over the generic agentic adapter layer:
skill-bank materialization, skill workspace layouts, skill-bank proposal input,
workspace proposal parsing, diffs, and skill change reports.

It depends on `leaven-agentic` and `leaven-artifact-skill`; it does not own the
generic agent runtime, generic case-suite substrate, provider protocols, or
optimizer rhythm.

## Map
- `SkillWorkspaceLayout` defines where a `SkillBank` is projected inside a
  workspace, including nested provider-native layouts such as `.agents/skills`.
- `SkillBankMaterializer` writes the parent skill bank into that layout for an
  agent to inspect or edit.
- `SkillBankProposalInput` points an agentic proposer at the parent candidate.
- `SkillBankWorkspaceProposalParser` reads workspace mutations back into
  skill-bank proposals.
- `SkillBankDiff` and change report types explain how a proposed skill bank
  differs from its parent.
- `SkillPatchPlan` validates agent-authored file-level patch plans against a
  parent skill bank: target files must exist before modification/deletion,
  create-file edits must not overwrite existing files, support counts must be
  positive, same-file ranges must not overlap, and new `references/*.md` files
  must be atomically paired with `SKILL.md` links.
- `SkillPatchMergeTree` records paper-neutral hierarchical consolidation
  provenance for validated patch plans: leaf plan ids, merge levels, accepted
  and discarded inputs, output plans, and final plan identity. It does not own
  merge prompts, prevalence thresholds, batch sizes, or result selection.

## Route Away
- `SkillBank`, `SkillFolder`, Agent Skills validation, and skill surfaces belong
  in `leaven-artifact-skill`.
- Generic proposer/evaluator runtime flow belongs in `leaven-agentic`.
- Codex, Claude Code, OpenCode, and provider-native skill discovery behavior
  belong in provider leaves. This crate may target a layout those providers can
  read, but it must not import provider protocol types.
- EvoSkill, Trace2Skill, Memento-Skills, D2Skill, and SkillReducer paper rhythm
  belong in examples or paper-specific optimizer/product crates.

## Proof Anchors
- `crates/leaven-agentic-skill/tests/skill_agentic.rs` proves materialization,
  nested layouts, parser validation, invalid skill rejection, loose-file
  rejection, and reports over real `SkillBank` artifacts.
- `docs/specs/agentic_skill_optimization_primitives.md` owns skill-specific
  responsibilities and the split from generic agentic workload code.
- Run `cargo nextest run -p leaven-agentic-skill` to prove skill-specific
  agentic helpers.

## Decision Cards
- when: changing skill workspace layout
  do: change `SkillWorkspaceLayout`, materializer, parser, and tests together
  preserve: layout as projection ABI only; `SkillBank` identity still comes from validated skill folders, not workspace mount paths
  avoid: teaching Codex/Claude/OpenCode discovery semantics in this crate or deriving artifact identity from `.agents/skills`
  verify: run `cargo nextest run -p leaven-agentic-skill` and keep the nested-layout test explicit

- when: parsing agent-authored skill changes
  do: parse the final workspace tree back into a valid `SkillBank`, diff it against the parent, and emit artifact-native `SkillBankChange`
  preserve: loose-file rejection, invalid folder/path rejection, unchanged-workspace rejection, and executable-bit preservation
  avoid: treating provider transcript text as the proposal when the workspace tree is the claimed source of truth
  verify: extend `skill_agentic.rs` near the parser rejection tests and run `cargo nextest run -p leaven-agentic-skill`

- when: adding paper-specific skill optimization behavior
  do: put EvoSkill/Memento/Trace2Skill/D2Skill/SkillReducer rhythm in examples or product crates that compose this adapter
  preserve: this crate as reusable skill materializer/parser/report glue over `leaven-agentic`
  avoid: adding paper utility thresholds, router training, train/validation split policy, or provider prompts here
  verify: run this crate's tests plus the paper/example gate that owns the rhythm

- when: validating agent-authored patch plans
  do: keep checks mechanical and paper-neutral: existing-file guards, create
  guards, support-count presence, same-file line-range conflicts, and
  `references/*.md` create/link pairing
  preserve: prevalence, deduplication, merge prompting, and analyst-role policy
  outside this crate
  avoid: encoding Trace2Skill batch sizes, utility thresholds, prompt wording,
  or result-selection policy in the patch-plan types
  verify: extend `skill_agentic.rs` around `SkillPatchPlan` tests and run
  `cargo nextest run -p leaven-agentic-skill`

- when: recording hierarchical patch consolidation
  do: use `SkillPatchMergeTree` for graph provenance over already validated
  `SkillPatchPlan`s
  preserve: each merge level consuming only plans available before that level,
  explicit accepted/discarded input decisions, unique plan ids, and a resolvable
  final plan id
  avoid: adding paper-specific worker counts, merge batch sizes, support
  thresholds, prompt wording, or final-metric selection to this crate
  verify: extend `skill_agentic.rs` around `SkillPatchMergeTree` tests and run
  `cargo nextest run -p leaven-agentic-skill`

## Local Bait
- `.agents/skills` is a workspace projection choice, not the artifact identity.
  Do not derive `SkillBank` identity from provider mount paths.
- Do not add paper-specific utility scores, router training, or EvoSkill
  iteration policy here. This crate supplies reusable skill adapters that those
  loops can compose.
- `SkillBankDiff` observes only final tree state. It cannot infer renames from
  delete/create pairs; stages that know they performed a rename should propose a
  rename directly instead of expecting readback to recover history.
