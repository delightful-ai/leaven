# Trace2Skill ARA Rigor Review

## Verdict

**Grade**: Weak Accept

The ARA is good enough as an anti-proxy denominator package. It is not good
enough to claim the paper has been reproduced, and it is not ready to launch the
full Qwen/vLLM run until the approval packet is filled.

## Strengths

- Paper target values are consistently labeled as targets, not Leaven results.
- Mechanics tests, one-case ACP evidence, and full paper parity are kept in
  separate proof classes.
- The full-run plan names the exact denominator knobs: Qwen3.5 model identity,
  vLLM, rows `0..200` / `200..400`, seeds `41/42/43`, 128 workers, merge batch
  size 32, ReAct turn budget 100, cost, credentials, and artifact retention.

## Blockers Before Any Reproduced Claim

| ID | Severity | Finding | Fix |
|----|----------|---------|-----|
| F01 | Major | `results/full_run_plan.md` still has null approval fields such as `max_usd: null`. | Fill every approval field before launching full denominator execution. |
| F02 | Major | `trace/exploration_tree.yaml` has dead-end nodes but no explicit `failure_mode` or `lesson` fields. | Addressed after review. |
| F03 | Major | Claims refer to an agreed reproduction tolerance, but no tolerance policy exists yet. | Proposed policy added after review; approval still required. |
| F04 | Minor | C07 cites only E08 even though E09 now owns the approval gate. | Addressed after review. |
| F05 | Minor | Prompt templates remain referenced but not transcribed into a dedicated evidence file. | Prompt family index added after review; rendered prompts remain future run artifacts. |

## Post-Review Follow-Up

Addressed after the initial review:

- `src/configs/tolerance.md` proposes metric, runtime, protocol-drift, retry,
  and failure-accounting policy.
- `evidence/prompt_templates.md` indexes upstream prompt-template families.
- `trace/exploration_tree.yaml` now includes `failure_mode` and `lesson` for
  DE1 and DE2.
- C07 now cites both E08 and E09.

Still blocking full reproduction:

- the full-run approval packet is unresolved;
- the proposed tolerance policy is not approved;
- exact rendered prompts must be captured during live analyst/model runs.

## Dimension Scores

| Dimension | Score | Note |
|-----------|-------|------|
| Evidence relevance | 4 | Tables and local proof docs mostly support their claims; C07 needs the E09 link. |
| Falsifiability | 3 | Directionally good, but tolerance and closeout checks are not concrete enough. |
| Scope calibration | 4 | Strong proxy refusal and denominator labels. |
| Argument coherence | 4 | Clear arc from paper targets to Leaven denominators to approval gates. |
| Exploration integrity | 3 | Real dead ends exist, but they need explicit failure/lesson fields. |
| Methodological rigor | 3 | Good config capture; still missing approval, tolerance, retry, variance, and prompt evidence. |

## Recommendation

Continue using this ARA as the denominator. Do not call the current state a
reproduction. The next highest-leverage fix is filling and approving the
full-run packet only when the user is ready to approve model/hardware/cost.

The machine-readable review is in `level2_report.json`.
