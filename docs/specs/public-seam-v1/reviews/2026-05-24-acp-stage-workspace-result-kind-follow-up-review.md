# Public Seam V1 ACP, Stage, and Workspace Result-Kind Follow-Up Review

Date: 2026-05-24

Reviewer: Mencius (`019e5966-752c-7d70-bb81-261bfda204bc`)

Reviewed revset: `qqolkswx::rznrqtnl`

Reviewed commits:

- `yzlmzsmk` / `faf6489b` — `public-seam: normalize candidate refs in stage provenance`
- `mwmorzkr` / `07483b29` — `public-seam: bind ACP extension primaries to method ops`
- `rznrqtnl` / `ac7106dd` — `public-seam: specialize ACP workspace result kinds`

## Sign-Off

The review found no blocking findings for keeping this tranche as honest partial
and pending evidence. The reviewer did not run tests and did not treat test
replay as sign-off; the review compared the tranche semantics against the
locked public-seam spec, schemas, ACP profile, and matrix fake-pass traps.

This is not sign-off to promote any row to `proven`.

## Accepted Partial Evidence

- Stage source-ref coverage now normalizes string and object-form
  `CandidateRef` values by candidate id before falling back to JCS identity for
  non-candidate `InfoRef` values.
- Generic ACP `extension` primaries are now bound to the locked method operation
  for graph, case, workspace-release, human-review, and event methods.
- ACP workspace extension-result primary kinds now match the Plan IR workspace
  query result-kind map.

## Non-Blocking Risks

- Candidate-ref normalization is intentionally id-only. The locked schema allows
  object-form refs to carry `run`; if `run` becomes identity-bearing later,
  this helper must be revisited with the schema change.
- ACP extension op binding uses a local method-to-op table. Future ACP method
  additions must update that table in the same change as the locked method set.
- The earlier review note in
  `2026-05-24-acp-evidence-stage-current-tranche-review.md` that string/object
  `CandidateRef` normalization was missing is now stale. This review supersedes
  that non-blocking risk.

## Status

Rows remain in their previous statuses. Pending rows remain pending, including:

- `ps1.visibility.reflector_target_safe`
- `ps1.stage.reflection_proposal_split`
- `ps1.stage.payload_receipts`
- `ps1.acp.extension_results`
- `ps1.workspace.handles_lifecycle`

