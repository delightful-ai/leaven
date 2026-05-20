## EvoSkill One-Iteration Loop

Status: active paper-specific example surface.

This package is the live Codex-backed EvoSkill reproduction. It preserves the
paper's failure-driven skill discovery loop before any shared Leaven abstraction
is extracted.

Paper anchors:

- EvoSkill uses Executor, Proposer, and Skill-Builder agents:
  `tmp/skill_opt_sources/arx_2603.02766/full_source.md:35`.
- The loop selects a parent from the frontier, samples train cases, scores
  failures, proposes a skill, builds a child program, evaluates validation, and
  admits or discards the child:
  `tmp/skill_opt_sources/arx_2603.02766/full_source.md:56`.
- The Proposer analyzes traces, predicted answers, ground truth, existing
  skills, and feedback history:
  `tmp/skill_opt_sources/arx_2603.02766/full_source.md:587`.
- The Skill-Builder materializes a high-level proposal into a concrete skill:
  `tmp/skill_opt_sources/arx_2603.02766/full_source.md:726`.
- Programs/frontier state are branch-like snapshots with lineage and validation
  scores:
  `tmp/skill_opt_sources/arx_2603.02766/full_source.md:794`.

The live command runs one tiny train/validation fixture through:

1. seed program and frontier initialization;
2. baseline validation with no relevant skill;
3. train failure collection;
4. proposer failure analysis and `SkillProposal`;
5. separate skill-builder materialization into `.agents/skills`;
6. child validation;
7. frontier admission and checkpoint/result persistence.

Latest inspected live proof:

```text
tmp/p5_evoskill_iteration/live-cli-20260520T203233Z/result_summary.json
```

That run used `gpt-5.4-mini`, produced `baseline_score: 0.0`,
`child_score: 1.0`, `admitted: true`, added
`fixed-income-quote-conversion`, and wrote 12 run-store checkpoints.

Known deviations:

- Codex/GPT-5.4-mini replaces Claude Code with Opus 4.5.
- The fixture uses Treasury quote conversion rather than OfficeQA/SealQA scale.
- A local run directory replaces paper git branches/tags, while preserving
  lineage/frontier/checkpoint state in Leaven run artifacts.
- The scorer is exact deterministic fixture scoring, not the full OfficeQA
  fuzzy multi-tolerance scorer or SealQA judge.
- One train/validation slice replaces 1.5 epochs over benchmark splits.
- Full OfficeQA/SealQA/BrowseComp datasets, skill-merge runs, held-out test
  tables, and cross-task transfer remain deferred.
- The CLI can inspect one real OfficeQA sample and one real SealQA sample
  without provider spend, but those samples are validation-only provenance
  inputs. They are not runnable train/validation splits.

Commands:

```bash
LEAVEN_CODEX_LIVE=1 cargo run -p p5_evoskill_iteration -- --live-codex
LEAVEN_CODEX_LIVE=1 cargo run -p p5_evoskill_iteration -- --live-codex --run-dir tmp/p5_evoskill_iteration/live-cli-YYYYMMDDTHHMMSSZ
cargo run -p p5_evoskill_iteration -- --officeqa-case tmp/paper_exact_samples/evoskill/officeqa/officeqa_pro_first_case.json --officeqa-source-text tmp/paper_exact_samples/evoskill/officeqa/treasury_bulletin_1941_01.txt --officeqa-split validation --inspect-cases
cargo run -p p5_evoskill_iteration -- --sealqa-case tmp/paper_exact_samples/evoskill/sealqa/seal_0_first_case.json --sealqa-split validation --inspect-cases
```
