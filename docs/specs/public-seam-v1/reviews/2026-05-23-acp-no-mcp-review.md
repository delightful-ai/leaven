# Public Seam V1 ACP No-MCP Review

Scope: `ps1.acp.no_mcp_v1` transport-scope validation in `crates/leaven-public-seam`.

Fresh evidence before review:

- `cargo fmt --check`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven-public-seam`
- `cargo test -p leaven --test topology_contract`

Adversarial reviewer:

- Agent id: `019e5475-3f26-72d2-8a55-6af3652ab2a6`

Initial review result:

- No sign-off. The reviewer blocked closeout because arbitrary renamed `leaven/*` tool-negotiation methods could pass prefix checks, and because archived MCP-over-ACP draft refusal was not part of the proposed row evidence.

Resolved findings:

- `V1Scope` now authorizes only methods extracted from the locked ACP profile, so renamed MCP/tool-negotiation surfaces such as `leaven/tools.list` are denied.
- The negative test covers MCP-over-ACP transport kind, legacy worker protocol, literal MCP method names, renamed tool-negotiation methods, watch runtime requests, and archived package refusal through `PublicSeamPackage::from_path`.
- The positive test derives Leaven callback methods from the locked ACP profile and authorizes those methods through the ACP-profile transport route.

Review result:

- `ps1.acp.no_mcp_v1`: signed off for marking `proven`, scoped to locked V1 public-seam transport-scope selection and MCP/watch/legacy-worker exclusion.

Limits:

- This sign-off does not prove ACP session/runtime behavior, ACP authentication or permissions, extension result envelopes, lifecycle/backpressure, or the broader `ps1.acp.transport_profile` row.
- Runtime ACP rows remain pending until their own executable semantic proof and adversarial review exist.
