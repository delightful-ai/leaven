# Stage 2 Rendered Prompt Artifacts

**Source**: `tmp/trace2skill-one-case-live/stage2_analyst_prompt.md` and
`tmp/trace2skill-one-case-live/stage2_fanout.json`, rendered from the scored
deterministic one-case run plus upstream prompt templates under
`tmp/repros/trace2skill-upstream/skill_evolver/prompts/`.

This evidence records the one-case Stage 2 MAP analyst prompt artifact that a
future approved analyst model call would consume. It has not executed an
analyst model call, parsed an analyst response, merged patches, or produced a
paper-denominator result row.
In the rendered prompt's own words, this pending fan-out has "not executed an analyst model call."

Regenerate the artifacts with:

```bash
cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-analyst-fanout --run-dir tmp/trace2skill-one-case-live
```

Regenerate and check the artifacts with:

```bash
uv run --with pyyaml python scripts/check_trace2skill_stage2_prompt_artifacts.py docs/ara/trace2skill_spreadsheetbench
```

| Artifact | Path | Bytes | SHA-256 | Status |
|----------|------|-------|---------|--------|
| Rendered Stage 2 MAP analyst prompt | `tmp/trace2skill-one-case-live/stage2_analyst_prompt.md` | `13031` | `94893fef2c3459bbe76bb63854dd2e9aab813625877c584867d34eadba700ba4` | Pending model call; not executed. |
| Pending Stage 2 fanout envelope | `tmp/trace2skill-one-case-live/stage2_fanout.json` | `654` | `71856dffdfbb4db1ebcfa43f32845a44ef1c37021f6965215523a9fbd33dd8c8` | One `Success` call, `success-13-1-1`, status `Pending`, response `null`. |

| Prompt source family | Source files embedded in the rendered prompt | Claim links |
|----------------------|---------------------------------------------|-------------|
| Base skill-evolving system prompt | `skill_evolving_agent/system_prompt_base.txt` | C04, C07 |
| Parallel MAP output format | `parallel_evolving_agent/map_output_format.txt` | C02, C04, C07 |
| Success-evolving prompt pieces | 14 files under `success_evolving_agent/`, including `success_record_section.txt`, `success_modification_strategies_section.txt`, and size/reference status lines | C01, C03, C07 |

Boundary: this closes a rendered-prompt artifact gap only for the deterministic
one-case pending Stage 2 call. Exact rendered prompts for approved full
Qwen/vLLM analyst and merge calls remain future run artifacts.
