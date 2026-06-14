# Validation

## 2026-06-14 Seal Level 1 Structural Check

Command:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench (23 files)
```

Scope:

- Confirms mandatory ARA directories and files exist and are non-empty.
- Confirms `PAPER.md` frontmatter and Layer Index exist.
- Confirms claims, experiments, concepts, heuristics, code refs, trace nodes, and evidence `Source` fields satisfy the local Seal Level 1 validator.

Limit:

- This does not prove source coverage completeness, full paper reproduction, Leaven result overlays, one-case live proof, held-out split execution, seed aggregation, or Qwen/vLLM parity.

## 2026-06-14 Plot Target Generation

Command:

```bash
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png
```

Scope:

- Reads Markdown tables in `evidence/tables/`.
- Generates a paper target sheet with four panels.

Limit:

- The generated PNG contains paper target values only. It is not a Leaven reproduced result.

## 2026-06-14 Updated Seal Level 1 Check

Command:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench (26 files)
```
