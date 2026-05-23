# Public Seam V1 Capability Grant Enforcement Review

Scope: `ps1.capability.grant_enforcement` grant-envelope authorization slice in `crates/leaven-public-seam`.

Fresh evidence before review:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`

Adversarial reviewer:

- Agent id: `019e5450-edcd-7351-b39d-9121f11d65f2`

Review result:

- `ps1.capability.grant_enforcement`: signed off for marking `proven`, scoped to grant-envelope authorization only.

Signed-off facts:

- Allowed grant requests succeed only when action, resource selectors, constrained dimensions, and per-grant limits fit the grant envelope, and return the capability fingerprint.
- Typed denials cover unknown actions, missing required request dimensions, resource mismatches, forbidden case fields, partition mismatches, schema mismatches, surface mismatches, forbidden data classes, and per-grant limit overruns.
- Data-class denials carry redaction facts for forbidden classes such as `external.secret`.
- Per-grant limit enforcement covers every limit field declared by the active capability schema: `max_usd_micro`, `max_calls`, `max_concurrent`, `timeout_s`, `max_rows`, and `max_materialized_bytes`.
- The evidence rejects the row fake pass: checking only that a token exists.

Limits:

- This sign-off does not prove aggregate budget ledgers, delegated-token attenuation, ACP permission handling, runtime effect execution, or transport behavior.
- Engine, ACP, budget, delegation, and runtime rows remain pending until their own executable semantic proof and adversarial review exist.
