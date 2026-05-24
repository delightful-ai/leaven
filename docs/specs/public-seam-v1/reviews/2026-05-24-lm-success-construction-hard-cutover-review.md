# LM Success Construction Hard Cutover Review

Date: 2026-05-24
Reviewer: Kuhn (`019e5af3-7de7-7543-997b-2ae09401513c`)

Scope: partial pending-row evidence for `ps1.lm.contract`.

The reviewed claim was not full row closeout. The claim was that successful
`lm_complete` host outcomes can no longer be publicly constructed from
hand-written response JSON and ad hoc cost fields. Successful construction now
goes through
`PlanLmCompleteOutcome::from_lm_response(Metered<leaven_lm::LmResponse>, Fingerprint)`.
`with_parsed` remains public only to attach JSON-schema parsed payloads, and
`failed_provider_error` remains public for failed paid provider effects.

## Review Result

Kuhn found no blocking findings for the stated partial claim.

The successful outcome escape hatch is closed at the public outcome-builder
layer: the outcome fields are crate-scoped, `new` is private, and successful
cost attachment is private. The only public successful constructor is
`from_lm_response`. The crate root re-exports the opaque outcome type, not a
raw JSON success builder.

Keeping `with_parsed` public was reviewed as acceptable for this partial proof:
it can attach a parsed JSON-schema payload but cannot mutate response message,
cost, replayability, or data classes. The execution and replay/result paths
validate parsed payload presence and schema for JSON-schema outputs.

The tests were reviewed as meaningful for this tranche. The positive trait-host
proof preserves provider-neutral request shape, tool-result id, tools,
provider hints, final output, and cost projection. The forged Plan Result
negatives rebind result hashes before rejection, proving semantic validation
rather than stale-hash rejection.

No topology leak was found. The public-seam crate still projects to and from
`leaven_lm` and `leaven_kernel` vocabulary; provider-runtime lowering remains
outside the seam.

## Residual Risk

This is acceptable as partial pending-row evidence only. Full
`ps1.lm.contract` closeout still needs an enforced runtime path that cannot
ignore `PlanLmCompleteRequest::to_lm_request()` and shell out provider-
specifically from raw call JSON, plus provider/ACP/streaming closure. Metered
cost is still trusted host input through `Metered<leaven_lm::LmResponse>`, so
this proves typed cost projection and receipt binding, not independent provider
metering truth.
