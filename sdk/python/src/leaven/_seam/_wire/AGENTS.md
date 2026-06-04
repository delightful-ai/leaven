## Boundary

`leaven._seam._wire` is the private typed wire-codec layer for the
Leaven-owned public seam. It may parse generated public-seam method metadata,
encode JSON-RPC envelopes, decode JSON-RPC envelopes with `msgspec`, expose
top-level generated payload/result records, and expose JSON value aliases used
by `_seam` request builders.

It must not own transport, process execution, retries, request routing, service
semantics, capability policy, or Rust graph truth. Those stay in neighboring
`_seam` modules and the Rust public-seam crates.

## Dependencies

- Public runtime: `msgspec`.
- Private source of truth: `crates/leaven-public-seam/src/acp_profile/methods.rs`
  plus `docs/specs/public-seam-v1/schemas/*.schema.json` via
  `sdk/python/codegen/generate_seam_wire.py`.

## Verification

Run from `sdk/python` after changing this package:

```bash
uv run python codegen/generate_seam_wire.py --check
uv run pytest tests/_seam/_wire -q
uv run ruff check src/leaven/_seam/_wire tests/_seam/_wire codegen/generate_seam_wire.py
uv run ty check src/leaven/_seam/_wire tests/_seam/_wire
```
