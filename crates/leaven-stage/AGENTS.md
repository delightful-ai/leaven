## Boundary

`leaven-stage` owns optimizer-stage agent workspace setup and query support.
It gives an optimizer stage a bounded workspace, a typed plan, scoped read
authority, declared output contracts, parsers, and receipts.

It is not the candidate-evaluation workload crate. `AgentCase`,
`AgentWorkload`, and `AgentCaseEvaluator` stay in `leaven-agentic`.

## Route Away

- Candidate task/case semantics belong in `leaven-agentic`.
- Raw workspace files, commands, paths, slots, and factory context belong in
  `leaven-workspace`.
- Graph mutation and stage finalization belong in `leaven-engine` through
  `RunContext`.
- GEPA strategy state and feedback selection belong in `leaven-gepa`.
- Provider runtime protocols belong in `leaven-agent*` leaves.

## Local Rules

- `StageReadAuthority` is the only query-derived workspace-entry path.
- `StageQueryPolicy` has allowed query kinds, prewarm queries, and caps. Do not
  reintroduce eager/lazy materialization names.
- `StageAttemptReceipt` is mandatory once behavior lands. Do not add a
  no-receipt mode.
- Keep `lib.rs` as a tiered map: user, adapter, and receipt/debug surfaces.

## Local Bait

- `docs/specs/public-seam-v1/` is the locked public seam contract for
  external-language workers (plan IR, capability tokens, stage payloads, ACP
  profile). It supersedes `worker_protocol.v1` and is the durable target this
  crate's stage surface lowers toward. The lowering is not yet done; do not
  claim alignment until the bridge lands.

## Verification

- `cargo nextest run -p leaven-stage` proves the stage surface.
- `cargo test -p leaven --test topology_contract` proves dependency shape and
  facade inventory when this crate is added or its dependencies change.
