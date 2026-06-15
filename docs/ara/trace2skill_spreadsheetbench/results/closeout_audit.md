# Trace2Skill Closeout Audit

Overall complete: `false`

Full paper-denominator reproduction is not proven: approval preflight is blocked and no paper-denominator Leaven result JSONL rows exist.

## Acceptance Status

| Acceptance id | Status | Remaining |
|---------------|--------|-----------|
| `ara_level1_valid` | `satisfied_current_package` | Re-run the validator after any ARA evidence or schema change. |
| `plots_from_ara` | `satisfied_targets_only` | No paper-denominator Leaven result JSONL overlay rows exist yet.<br>Target plots remain target evidence, not reproduction evidence. |
| `current_mechanics_classified` | `satisfied_current_tests` | Re-run focused Rust tests after changing example mechanics or proof classifications. |
| `one_case_live_or_explicit_blocker` | `satisfied_deterministic_one_case` | This is deterministic local ACP one-case evidence only.<br>Model-backed one-case evidence remains absent until approved. |
| `full_denominator_plan_approved` | `blocked` | models.qwen_122b is unresolved<br>models.qwen_35b is unresolved<br>serving.host is unresolved<br>serving.version is unresolved<br>serving.tensor_parallel is unresolved<br>serving.gpu_type is unresolved<br>serving.gpu_count is unresolved<br>budget.max_usd is unresolved<br>budget.max_wall_clock_hours is unresolved<br>budget.max_gpu_hours is unresolved<br>credentials.api_key_env is unresolved<br>credentials.redaction_policy is unresolved<br>credentials.log_retention is unresolved<br>artifacts.root is unresolved<br>artifacts.retention is unresolved<br>approval.approved_by is unresolved<br>approval.approved_at is unresolved<br>tolerance.approved must be True, got None |
| `reproduced_claim_limited_to_actual_denominator` | `guardrail_active_not_final_closeout` | No held-out 200..400 result rows exist.<br>No seed aggregate rows exist.<br>No cross-model paper-denominator rows exist.<br>Final closeout remains impossible while normal approval preflight fails. |

## Current Denominators

Reproduced or captured:
- `paper-targets-captured`
- `mechanics-tests-classified`
- `deterministic-one-case-13-1`

Missing:
- `model-backed-one-case-13-1`
- `small-N-paper-subset`
- `evolving-split-0..200`
- `held-out-split-200..400`
- `seed-aggregate-41-42-43`
- `cross-model-paper-rows`
- `full-paper-denominator`

## Dataset Manifest

- Case count: `400`
- Dataset JSON SHA-256: `bcecaa89a005bd4e3bbe98da150a86e8062c27f262e575d5e47bd9861b3525e7`
- Case order SHA-256: `ac05d2035ad776af9d901689423645316e707e6e8426a04d2eae6591929b64e9`
- Missing workbook directories: `0`

## Proxy Refusal

These labels remain forbidden as full reproduction closeout evidence:
- `historical-yaml`
- `ara-shape-only`
- `paper-target-plot`
- `trace2skill-tiny-live`
- `one-case-only`
- `mechanics-tests`
- `harbor-adapter`
- `subset-improvement`
