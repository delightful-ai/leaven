# Constraints and Limitations

## Paper-Protocol Constraints

- SpreadsheetBench-Verified uses 400 samples with rows `0..200` for evolving/training and `200..400` held out.
- Main spreadsheet results are averaged over seeds `41`, `42`, and `43`.
- The reported model pair is Qwen3.5-122B-A10B and Qwen3.5-35B-A3B.
- The upstream reproduction notes use vLLM/OpenAI-compatible serving.
- Stage 2 uses 128 sub-agents in the paper configuration.
- The merge batch size is 32 in the paper configuration.
- ReAct-style agents use a turn budget of 100.

## Leaven Goal Constraints

- Target plots are not reproduction results.
- `trace2skill_tiny_live` is a proxy causal loop, not SpreadsheetBench parity.
- Case `13-1` is a one-case denominator only.
- Mechanics tests are evidence for lowering/replay/scoring behavior only.
- Full-scale Qwen/vLLM execution requires explicit approval before launch.

## Source Gaps

- Some environmental details are "Not specified in paper" and must be obtained from upstream code, run logs, or approved live execution.
- The paper itself notes work-in-progress limits around causal effect quantification of individual patches and tracing the utility of specific skill sections.
- This first ARA pass captures source evidence; it does not yet contain Leaven result overlays.
