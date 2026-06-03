## Boundary

`leaven.builders` owns role-scoped public builder objects such as `cx.agent`,
`cx.lm`, and `cx.proposals`. Builders expose public SDK methods and may hold a
private seam requester when they are supplied by a running stage context.

## Public Dependencies

- Public typed SDK records from sibling public modules, for example
  `leaven.proposal`, `leaven.output`, `leaven.agent_instructions`, and
  `leaven._receipts` opaque handles.
- The governing Python SDK spec in `docs/specs/leaven_python.md`.

## Private Dependencies

- Private request builders from `leaven._seam` for focused process-seam
  bindings.
- No imports from `_seam_worker`, `_seam_optimize`, or `_serve`; those packages
  may construct bound builders, but builder modules must not depend on their
  stage-driver internals.

## Invariants

- Unbound builders stay scaffolded with explicit `NotImplementedError`.
- Bound builders may perform real seam calls only by lowering to locked
  `leaven/*` JSON-RPC methods.
- Proposal submission and proposal application are separate capabilities; do
  not hide an apply behind submit unless the public method says so.
