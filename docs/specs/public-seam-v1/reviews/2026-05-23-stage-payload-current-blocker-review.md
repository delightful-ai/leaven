# Stage Payload Current Blocker Review

Reviewer: Gauss (`019e571c-c9f0-7c60-824f-525ba34065a1`)

Scope:

- `ps1.visibility.reflector_target_safe`
- `ps1.stage.reflection_proposal_split`
- `ps1.stage.payload_receipts`

Decision: all three rows remain pending.

Findings:

- `ps1.visibility.reflector_target_safe` had an explicit target-leak hole: reflective examples allow `output` and `feedback`, but the semantic leakage scan only inspected `input`, `side_info`, and `score`. Follow-up code now scans `output` and `feedback` too, but the row still needs follow-up review and separate evidence for reflector LM-call input-class denial or a deliberate row split.
- `ps1.stage.reflection_proposal_split` is still structural JSON validation, not a durable stage split proof. `ProposeRequest` embeds a `ReflectionResult` object, and current validation proves role shape only; it does not bind the reflection result to a separately produced reflector stage via stage identity, receipt, or prior-stage reference.
- `ps1.stage.payload_receipts` overclaims uniform provenance/receipt fields. Current role schemas are not uniform: runner, scorer, judge, callback, and adapter payloads do not all carry the row's listed source refs, query policy, read receipts, data classes, and capability fields. The current allowed-effects negative rejects unknown allowed-effect values, not a proposed effect outside an allowlist.

Current status:

- Keep all reviewed rows pending.
- The reflective example output/feedback leakage bug is fixed in code, but the rows still need either stronger binding/provenance primitives or narrower row wording before promotion.
