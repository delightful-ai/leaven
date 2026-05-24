# Blocked ACP Status Review

Date: 2026-05-24

Reviewer: Maxwell (`019e59ab-962d-7c11-960c-5a9b57b90fe3`)

Reviewed revset: `mpvsywyzqwlx::lnzzklkrrmkp`

## Reviewed Claim

The conformance matrix can honestly distinguish rows that still need code work
from ACP rows that are blocked on a production process/session transport owner.
The tranche marked only these rows blocked:

- `ps1.acp.transport_profile`
- `ps1.acp.extension_results`
- `ps1.acp.lifecycle_backpressure`

No row was promoted.

## Findings

Initial review found no Critical findings and two Important findings:

- `review_evidence` was documented as a proven-row field, but pending and
  blocked rows intentionally use it as partial-evidence and blocker provenance.
- `blocked_on` only had a non-empty-vector check. Blank prerequisites passed,
  and stale `blocked_on` entries on non-blocked rows were not rejected.

It also found one Minor finding:

- `ps1.harness.negative_denominator` did not cite the blocked-row audit
  negative test.

The reviewer explicitly did not treat rerunning tests as sign-off; the review
was semantic inspection of the spec, matrix, code, tests, and status claims.

## Resolution

Follow-up changes resolved the findings:

- `ConformanceRow::review_evidence` now documents that proven rows require
  review evidence, while pending and blocked rows may use it only for partial
  evidence or blocker provenance.
- `audit_conformance_evidence` rejects blank blocked prerequisites.
- `audit_conformance_evidence` rejects `blocked_on` on non-blocked rows.
- `ps1.harness.negative_denominator` cites both blocked-row negative tests.

## Status

The three ACP rows remain blocked because current evidence proves profile,
schema, lifecycle vocabulary, JSON-RPC envelope, and extension-result wire
contracts only. It does not prove production ACP process I/O, live worker
lifecycle cancellation/progress control, or extension-result envelopes carried
through a real public ACP session for all V1 method families.

The remaining pending rows remain pending. No row is promoted by this tranche.
