# Public Seam V1 Capability Document Review

Scope: `ps1.capability.document_truth` capability-document slice in `crates/leaven-public-seam`.

Fresh evidence before review:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`

Adversarial reviewer:

- Agent id: `019e5439-3a29-7633-b64d-5a890391d273`

Review result:

- `ps1.capability.document_truth`: signed off for marking `proven`, scoped to schema-valid capability documents resolved from opaque token handles.

Signed-off facts:

- `CapabilityDocument::from_value` validates arbitrary input against the locked active `leaven.capability.v1.schema.json` and `common.schema.json` before serde parsing.
- The public document surface preserves issuer, audience, issued-at, execution policy, aggregate budgets, grant resources, grant constraints, grant limits, delegation allowed actions, and token binding facts.
- Opaque token resolution rejects bare/missing token handles, missing subject fingerprints, expired tokens, revoked tokens, and binding mismatches before a new operation is authorized.
- Schema-invalid but serde-shaped documents are rejected, including missing required authority fields, unexpected top-level fields, invalid revocation enum values, and invalid grant action patterns.
- The evidence rejects the row fake pass: passing token strings around as authorization facts.

Limits:

- This sign-off does not prove signed-token authentication or signed-JWT operation resolution.
- This sign-off does not prove grant enforcement, delegation attenuation, aggregate budget spending, ACP authentication, or runtime permission behavior; neighboring rows remain pending until their own executable semantic proof and adversarial review exist.
