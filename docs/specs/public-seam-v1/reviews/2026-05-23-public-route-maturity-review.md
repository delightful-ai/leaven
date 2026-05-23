# Public Seam V1 Public Route Maturity Review

Scope: `ps1.public_routes.maturity_classified` route-maturity slice for the V1 public seam owner and the `leaven` umbrella route.

Fresh evidence before review:

- `cargo test -p leaven-public-seam --test contract_package public_seam_routes_reject_ordinary_facade_leaks -- --exact`

Adversarial reviewer:

- Agent id: `019e5444-9c4e-7f41-ad47-35c805bdba13`

Review result:

- `ps1.public_routes.maturity_classified`: signed off for marking `proven`, scoped to public-route maturity classification and absence from ordinary umbrella routes.

Signed-off facts:

- Public seam crate-root exports are classified in `crates/leaven-public-seam/AGENTS.md` as advanced public contract or advanced harness contract, not ordinary product proof.
- `CapabilityDocument`, `CapabilityRegistry`, `CapabilityError`, `ConformanceTest*`, `ConformanceRow`, and matrix/status types are not routed through `leaven::prelude`, default umbrella features, or ordinary examples.
- `crates/leaven/Cargo.toml` does not depend on `leaven-public-seam`.
- `crates/leaven/src/lib.rs`, `prelude.rs`, `extend.rs`, and `plumbing.rs` do not expose immature public-seam names.
- The existing umbrella `public_surface_contract` keeps ordinary, extension, and plumbing routes classified and rejects loose crate-root type exports.
- The evidence rejects the row fake pass: passing topology tests while exposing immature names through default imports.

Limits:

- This sign-off does not prove runtime public-seam behavior, ACP transport, grant enforcement, example product proof, or ordinary umbrella exposure.
- Future public-seam exports, default features, facades, preludes, or examples must update the route-maturity classification and proof again.
