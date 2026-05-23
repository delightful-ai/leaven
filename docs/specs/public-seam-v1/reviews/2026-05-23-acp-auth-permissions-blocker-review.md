# ACP Auth Permissions Blocker Review

Reviewer: Einstein (`019e5705-3acb-7531-9740-7247783940e6`)

Scope: `ps1.acp.auth_permissions`

Decision: not proven at review time.

Findings:

- P1: Permission authorization accepted a raw `CapabilityDocument` without requiring an `AcpAuthenticatedSession`, so the permission path could bypass ACP authenticate.
- P1: ACP authenticate fingerprint binding was optional even though the stdio profile requires `LEAVEN_CAPABILITY_FINGERPRINT`.
- P2: `AcpPermissionRequest` did not project all grant dimensions enforced by the capability document, including case fields, partitions, purposes, model roles, and limit usage.
- P3: Denial tests only checked `allowed == false` for model and sandbox denials, not closed `PlanError` shape.

Resolution status:

- Kept `ps1.acp.auth_permissions` pending after review.
- Follow-up implementation now requires an authenticated session for permission authorization, makes ACP authenticate fingerprint binding mandatory, requires `fingerprint_env` in the ACP profile schema, projects the missing permission dimensions, and asserts closed denial errors for model and sandbox denials.
- A follow-up adversarial review is still required before the row can move to `proven`.
