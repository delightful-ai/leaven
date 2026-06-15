# Reproduction Tolerance Policy

This is the proposed judgment policy for future Leaven result rows. It must be
approved with `results/full_run_plan.md` before full-denominator execution.

## Numeric Metrics

| Metric family | Paper display | Proposed reproduction tolerance | Notes |
|---------------|---------------|----------------------------------|-------|
| SpreadsheetBench `Vrf`, `Soft`, `Hard` | percent with two decimals | absolute difference `<= 1.00` point on the seed-aggregated mean | Applies only after seeds `41/42/43` are all run under the same protocol. |
| WikiTQ spreadsheet metrics | percent with two decimals | absolute difference `<= 1.00` point on the seed-aggregated mean | Required only for claims that include WikiTQ rows. |
| DocVQA `ANLS` / accuracy | percent with two decimals | absolute difference `<= 1.00` point on the approved evaluation split | Out of current SpreadsheetBench-centered scope unless explicitly approved. |
| Math pass/avg metrics | percent with two decimals | absolute difference `<= 1.00` point | Out of current SpreadsheetBench-centered scope unless explicitly approved. |
| Average deltas | delta points with two decimals | absolute difference `<= 1.00` point after recomputing from component metrics | Do not compare Avg alone if component metrics drift differently. |

## Runtime Metrics

| Runtime claim | Proposed tolerance | Notes |
|---------------|--------------------|-------|
| Parallel faster than sequential | ordering must match paper: Parallel faster than Seq-B=4 faster than Seq-B=1 | More stable than raw minutes across hardware. |
| Approximate minutes | reported separately from score reproduction | Raw minute parity requires matching hardware or an explicit hardware-normalized plan. |

## Protocol Drift

Any of the following changes must be labeled as a deviation and cannot be called
1:1 reproduction without separate approval:

- model id differs from `Qwen3.5-122B-A10B` or `Qwen3.5-35B-A3B`;
- serving backend differs from vLLM;
- seed set differs from `41`, `42`, `43`;
- rows `0..200` / `200..400` are not used as the evolving/held-out split;
- Stage 2 worker count differs from `128`;
- merge batch size differs from `32`;
- ReAct turn budget differs from `100`;
- output workbook, transcript, score report, trajectory, or result JSONL
  artifact paths are missing.

## Failure Accounting

Failed or timed-out cases stay in the denominator. They must be reported with
their failure status and artifacts rather than silently retried out of the
aggregate.

Retries are allowed only when the full-run approval packet records:

- retry trigger;
- retry count limit;
- whether the first failure remains in logs;
- whether the final metric uses first attempt or best successful attempt.

Until that policy is approved, use first-attempt results for denominator claims.
