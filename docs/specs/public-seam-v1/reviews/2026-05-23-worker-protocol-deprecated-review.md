# Public Seam V1 Deprecated Worker Protocol Review

Scope: `ps1.worker_protocol.deprecated` transport-scope deprecated-marker enforcement in `crates/leaven-public-seam`.

Fresh evidence before review:

- `cargo fmt --check`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven-public-seam`
- `cargo test -p leaven --test topology_contract`

Adversarial reviewer:

- Agent id: `019e547b-3a58-7353-a786-5fc1180dd510`

Initial review result:

- No sign-off. The reviewer blocked closeout because the first negative test rejected only the legacy transport kind, leaving a fake pass where worker-protocol runtime behavior could be revived as an ACP-shaped method such as `leaven/worker_protocol.run`.

Resolved findings:

- The negative test asserts the locked ACP profile exposes no `worker_protocol` methods.
- The negative test rejects `WorkerTransportKind::AcpProfile` with `leaven/worker_protocol.run`, so revival through an ACP-shaped route is denied by the locked profile method set.
- The deprecated marker validates only as `leaven.worker_protocol.v1.deprecated` with replacement `leaven.acp_profile.v1`.
- The product worker path exercised by this proof uses ACP extension methods authorized through `V1Scope`.

Review result:

- `ps1.worker_protocol.deprecated`: signed off for marking `proven`, scoped to public-seam transport-scope deprecated-marker enforcement and ACP-profile routing.

Limits:

- This sign-off does not prove ACP session/runtime behavior, broad worker transport maturity, process lifecycle, authentication, or runtime worker execution.
- Runtime ACP and worker rows remain pending until their own executable semantic proof and adversarial review exist.
