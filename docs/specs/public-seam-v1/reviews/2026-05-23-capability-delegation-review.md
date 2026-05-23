# Public Seam V1 Capability Delegation Review

Scope: `ps1.capability.delegation_attenuates` parent-child capability attenuation slice in `crates/leaven-public-seam`.

Fresh evidence before review:

- `cargo fmt --check`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven-public-seam`
- `cargo test -p leaven --test topology_contract`

Adversarial reviewer:

- Agent id: `019e5467-5426-7780-8bcd-c9b64f51e29c`

Initial review result:

- No sign-off. The reviewer blocked closeout because the first implementation could still accept a same-operational-authority child, lacked aggregate-budget negative coverage, compared only token-binding kind, and treated exhausted depth or empty `allowed_actions` too permissively.

Resolved findings:

- Same-operational-authority children are rejected when `must_attenuate` is true; delegation-policy-only narrowing is not enough.
- Aggregate and grant-budget widening and omission are rejected.
- Token-binding attenuation checks kind-specific authority: opaque lookup preserves parent `lookup_audience`, while signed JWT and mTLS same-kind authority changes are denied.
- Exhausted parent delegation depth rejects delegation, and empty parent `allowed_actions` grants no child actions.
- Negative tests cover the fake pass named by the row: issuing a fresh full-power child token from a valid parent.

Review result:

- `ps1.capability.delegation_attenuates`: signed off for marking `proven`, scoped to public-seam semantic validation of parent-child capability documents.

Limits:

- This sign-off does not prove token minting, registry insertion as a delegation workflow, ACP permission flow, transport/session behavior, runtime delegation behavior, or engine aggregate budget-ledger enforcement.
- Engine, ACP, aggregate-ledger, and runtime rows remain pending until their own executable semantic proof and adversarial review exist.
