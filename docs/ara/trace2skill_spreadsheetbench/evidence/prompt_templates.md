# Prompt Template Evidence

**Source**: `tmp/repros/trace2skill-upstream/skill_evolver/prompts/`

The upstream prompt tree is part of the paper denominator because Stage 2
analyst calls and Stage 3 merge behavior depend on prompt wording and output
formats. This file indexes the prompt families without copying full prompt text.
Exact source identity for every indexed prompt file is recorded in
`prompt_templates.manifest.json`.

Regenerate and check the prompt-source manifest with:

```bash
uv run --with pyyaml python scripts/check_trace2skill_prompt_manifest.py docs/ara/trace2skill_spreadsheetbench --write
uv run --with pyyaml python scripts/check_trace2skill_prompt_manifest.py docs/ara/trace2skill_spreadsheetbench
```

| Prompt family | File count | Representative paths | Reproduction role | Claims |
|---------------|------------|----------------------|-------------------|--------|
| Spreadsheet agent system prompts | 2 | `spreadsheet_agent/system_prompt/cli_skill_preloaded_full_system_v1.txt`; `spreadsheet_agent/system_prompt/cli_only_full_system_v1.txt` | Agent runtime prompt for SpreadsheetBench task solving. | C01, C06, C07 |
| Error evolving agent | 18 | `skill_evolver/prompts/skill_evolving_agent/system_prompt_base.txt`; `skill_evolver/prompts/skill_evolving_agent/error_record_section_skill.txt`; `skill_evolver/prompts/skill_evolving_agent/final_consolidation_checklist.txt` | Error-derived patch proposal and validation prompt pieces. | C04, C07 |
| Success / combined evolving agent | 43 | `skill_evolver/prompts/success_evolving_agent/success_record_section.txt`; `skill_evolver/prompts/success_evolving_agent/combined_record_section.txt`; `skill_evolver/prompts/success_evolving_agent/combined_merge_system_prompt.txt` | Success-derived and combined patch proposal/merge prompt pieces. | C01, C03, C07 |
| Parallel merge/application agent | 36 | `skill_evolver/prompts/parallel_evolving_agent/map_output_format.txt`; `skill_evolver/prompts/parallel_evolving_agent/merge_system_prompt.txt`; `skill_evolver/prompts/parallel_evolving_agent/verification_system_prompt.txt` | Parallel map output, hierarchical merge, translation, verification, and patch-application prompt pieces. | C02, C04, C07 |
| Released skill prompts | 4 skill files | `released_skills/trace2skill-xlsx-122B-combined/SKILL.md`; `released_skills/trace2skill-xlsx-35B-combined/SKILL.md`; `released_skills/xlsx-122B/SKILL.md`; `released_skills/xlsx-35B/SKILL.md` | Published skill inputs/targets for same-model and cross-model checks. | C01, C03, C07 |

## Boundary

This index is not a prompt-fidelity proof by itself. A full live analyst
reproduction must record the exact rendered prompt artifacts for each call,
including source template paths, filled trajectory fields, model id, and output
parser/validator result.
