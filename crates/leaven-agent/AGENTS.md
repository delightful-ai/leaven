## Boundary
This crate owns provider-neutral agent runtime contracts for one session in an
already-materialized workspace.

It knows `WorkspaceView`, `WorkspacePath`, execution instructions, output
contracts, runtime capabilities, transcripts, raw provider events, status,
cost, and output files. It must not know candidates, proposals, assessments,
GEPA, skill banks, run graphs, or optimizer rhythm.

## Map
- `AgentRuntime` runs one session and returns `Metered<AgentSession>`.
- `AgentRunRequest` is execution vocabulary: requested runtime identity,
  optional expected runtime fingerprint, instructions, cwd, output contract,
  environment, tool policy, and limits. `OutputContract::JsonSchema` preserves
  structured final-message requirements by schema fingerprint and schema body;
  it is not a workspace JSON file path.
- `AgentRunContext` is not `RunContext`; it carries execution facts such as
  session id, budget snapshot, and cancellation.
- `AgentSession` is fact, not interpretation. Stage parsers decide whether
  session/workspace output becomes proposals or assessments.
- `WorkspaceAccessMode` makes backend-neutral versus local-mount requirements
  explicit before a runtime is paired with a workspace backend.

## Route Away
- Agentic proposers/evaluators, case suites, proposal parsers, evidence
  parsers, repair loops, and scoring adapters belong in `leaven-agentic`.
- Command launching substrate belongs in `leaven-agent-command`.
- Codex, Claude Code, OpenCode, and other provider/runtime lowering belongs in
  `leaven-agent-*` leaves.
- Graph mutation and budget charging against the run ledger stay in
  `leaven-engine` through the stage that owns the runtime call.

## Proof Anchors
- `crates/leaven-agent/tests/runtime_contract.rs` proves fake runtime behavior,
  output-contract validation, cancellation, capabilities, transcripts, and
  public error shapes.
- `docs/specs/agentic_stage_runtime.md` section "Provider-Neutral
  `AgentRuntime`" owns this crate's boundary.
- Run `cargo test -p leaven-agent` to prove runtime vocabulary and fake
  runtime contracts.

## Decision Cards
- when: adding or changing provider-neutral runtime fields
  do: keep the field about a single session in an already-materialized workspace
  preserve: `AgentRunRequest` as execution input, `AgentRunContext` as session/budget/cancel facts, and `AgentSession` as observed output
  avoid: candidate ids, proposal/evidence interpretation, case-suite partitions, GEPA selectors, provider protocol structs, or graph mutation handles
  verify: run `cargo test -p leaven-agent` and inspect that the new field is exercised through `runtime_contract.rs`, not only derived serialization

- when: changing output contracts
  do: make validation prove only provider-neutral facts visible here: file exists, JSON syntax parses, final assistant message exists, or workspace-diff roots are recorded
  preserve: schema-specific interpretation and workspace-diff parsing as parser/stage responsibilities
  avoid: turning `JsonSchemaRef` into a global schema engine or making `WorkspaceDiff` create artifacts
  verify: run `cargo test -p leaven-agent` and add one failing-contract case in `runtime_contract.rs`

- when: using `FakeAgentRuntime`
  do: import it through `leaven_agent::test_support` for deterministic contract tests, examples, diagnostics, and adapter proofs before a provider is involved
  preserve: its status as explicit test/example support outside the crate root and prelude
  avoid: citing fake runtime behavior as evidence that a production provider path, approval policy, sandbox, or skill discovery works
  verify: pair fake-runtime tests with the concrete provider leaf test once provider lowering is the claim

## Local Bait
- `docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md` is the
  legacy-named locked Leaven worker profile that lifts this crate's session
  vocabulary into the external-worker public seam. All worker callbacks travel
  as Leaven `leaven/*` extension methods; there is no MCP layer in v1. Use it
  as the durable target when shaping runtime/output-contract changes, not as
  proof of an implemented bridge.
- `FakeAgentRuntime` is a contract-test helper under `test_support`, not a
  provider architecture. New real providers should implement `AgentRuntime` in
  provider leaves.
- `OutputContract::JsonSchema` validates the same final-assistant-message
  presence as `FinalMessage`; provider leaves own actual schema-constrained
  decoding/enforcement until a runtime-specific verifier is added.
- `OutputContract::WorkspaceDiff` does not parse diffs and does not create
  artifacts. It preserves the public seam `surface_fingerprint` when one is
  supplied, then states what kind of output the runtime should leave for a
  stage-owned parser.
- `AgentToolPolicy` is request vocabulary. It defaults to no shell access and
  no command allow-list. This crate does not enforce shell, command, or tool
  allowlists; provider leaves or stages must translate/reject them honestly.
- Returning `Metered<AgentSession>` reports cost to the caller. This crate does
  not mutate the engine budget ledger directly.
