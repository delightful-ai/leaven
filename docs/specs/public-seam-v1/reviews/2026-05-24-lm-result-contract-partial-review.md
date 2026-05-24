# Public Seam V1 LM Result Contract Partial Review

Scope: `ps1.lm.contract` partial LM result validation in
`crates/leaven-public-seam`.

Reviewer: Helmholtz (`019e5885-5c6d-7103-9599-3763ba11daea`)

Review mode: read-only adversarial semantic inspection. The reviewer was
explicitly instructed not to treat rerunning the same tests as sign-off.

Reviewed sources:

- `docs/specs/public-seam-v1/manifest.json`
- `docs/specs/public-seam-v1/01_plan_ir_spec_v0.3.md`
- `docs/specs/public-seam-v1/schemas/leaven.plan.v1.schema.json`
- `docs/specs/public-seam-v1/schemas/leaven.plan_result.v1.schema.json`
- `docs/specs/public-seam-v1/notes/CONFORMANCE_TESTS_v0.3.md`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `crates/leaven-public-seam/src/plan_execution/receipts.rs`
- `crates/leaven-public-seam/tests/plan_document.rs`

## Findings And Resolution

1. Initial review found that result-side `tool_call_id` or `name` metadata could
   survive on an otherwise text-only assistant result. Resolution:
   `lm_complete` result messages now reject both fields, with live-host and
   forged Plan Result negatives.

2. Initial review found the external forged-result surface was weaker than the
   live-host surface. Resolution: forged/rebound Plan Result negatives now cover
   wrong role, `tool_result` content, extension content, result-side tool
   metadata, and oversized `final_message` text.

3. Follow-up review found the forged oversized negative could reject because the
   validation plan used a different request preimage. Resolution: the forged
   oversized case now builds its baseline result from the same `max_bytes: 2`
   plan used for validation, mutates only the result text, and rebinds the
   result hash.

## Sign-Off

Final follow-up review reported no blocking findings. The tranche is acceptable
as partial pending-row evidence for public-seam validation of V1 LM result
semantics.

This does not prove concrete LM provider execution, streaming behavior, ACP
delivery, or full `ps1.lm.contract` closeout. `ps1.lm.contract` remains
`pending`.
