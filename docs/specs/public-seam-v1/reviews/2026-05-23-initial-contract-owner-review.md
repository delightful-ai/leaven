# 2026-05-23 Initial Contract Owner Review

Scope: `crates/leaven-public-seam` first implementation slice.

Evidence commands:

- `cargo test -p leaven-public-seam --test contract_package`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo fmt --check`
- `cargo test -p leaven --test topology_contract`

Reviewer: adversarial sub-agent `019e540b-5557-75f3-b285-9961b0e0d3a2`.

## Signed Off Rows

- `ps1.authority.active_package_only`
  - Evidence: `active_package_loader_accepts_only_locked_public_seam_v1_package`.
  - Positive proof: active repo package loads from `docs/specs/public-seam-v1`.
  - Negative proof: archived package path and copied active-looking package under a temp `docs/specs/public-seam-v1` path are rejected.
  - Follow-up review: no blocking findings after canonical active-package path check.

- `ps1.authority.manifest_inventory`
  - Evidence: `manifest_inventory_drives_contract_file_loading`.
  - Positive proof: inventory is manifest-driven for gate, matrix, schemas, and profiles.
  - Negative proof: a manifest-listed missing schema fails inventory loading.

- `ps1.schema.fingerprints`
  - Evidence: `schema_fingerprints_use_jcs_sha256_not_pretty_printed_bytes`.
  - Positive proof: schema fingerprints use `fp_schema_sha256_` plus a 64-byte hex SHA-256 digest.
  - Negative proof: pretty formatting is stable while semantic JSON changes alter the fingerprint.

## Rows Kept Pending

- `ps1.watch.deferred`: current code proves manifest markers only. It does not yet prove finite diff behavior through `consistency.since_revision` or reject runtime watch subscriptions from a worker path.
- `ps1.worker_protocol.deprecated`: current code proves manifest markers only. It does not yet prove the product transport path cannot expose deprecated worker-protocol runtime behavior.
- `ps1.acp.no_mcp_v1`: current code proves manifest markers only. It does not yet prove product/default ACP paths cannot enable MCP or reject archived MCP draft payloads.
- `ps1.harness.negative_denominator`: current code parses the matrix and row refs. It does not yet model row proof fields deeply enough to reject a fake closeout that cites schema-only/example/topology proof.

## Non-Blocking Notes

- The active `reflect_then_propose` example previously used `"<above>"` for `propose_request.reflection_result`. The schema requires a `ReflectionResult` object, so the example now embeds the object. This preserves the locked reflection/proposal split and proves example validity only, not stage semantics.
