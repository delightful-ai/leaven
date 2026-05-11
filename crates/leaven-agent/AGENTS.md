## Boundary
This crate owns provider-neutral agent runtime contracts for one session in an
already-materialized workspace.

It knows `WorkspaceView`, `WorkspacePath`, execution instructions, output
contracts, runtime capabilities, transcripts, raw provider events, status,
cost, and output files. It must not know candidates, proposals, assessments,
GEPA, skill banks, run graphs, or optimizer rhythm.

## Map
- `AgentRuntime` runs one session and returns `Metered<AgentSession>`.
- `AgentRunRequest` is execution vocabulary: instructions, cwd, output contract,
  environment, tool policy, and limits.
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
- Run `cargo nextest run -p leaven-agent` to prove runtime vocabulary and fake
  runtime contracts.

## Decision Cards
- when: adding or changing provider-neutral runtime fields
  do: keep the field about a single session in an already-materialized workspace
  preserve: `AgentRunRequest` as execution input, `AgentRunContext` as session/budget/cancel facts, and `AgentSession` as observed output
  avoid: candidate ids, proposal/evidence interpretation, case-suite partitions, GEPA selectors, provider protocol structs, or graph mutation handles
  verify: run `cargo nextest run -p leaven-agent` and inspect that the new field is exercised through `runtime_contract.rs`, not only derived serialization

- when: changing output contracts
  do: make validation prove only provider-neutral facts visible here: file exists, JSON syntax parses, final assistant message exists, or workspace-diff roots are recorded
  preserve: schema-specific interpretation and workspace-diff parsing as parser/stage responsibilities
  avoid: turning `JsonSchemaRef` into a global schema engine or making `WorkspaceDiff` create artifacts
  verify: run `cargo nextest run -p leaven-agent` and add one failing-contract case in `runtime_contract.rs`

- when: using `FakeAgentRuntime`
  do: use it for deterministic contract tests, examples, and adapter proofs before a provider is involved
  preserve: its status as test/example support even though it is currently public and prelude-exported
  avoid: citing fake runtime behavior as evidence that a production provider path, approval policy, sandbox, or skill discovery works
  verify: pair fake-runtime tests with the concrete provider leaf test once provider lowering is the claim

## Local Bait
- `FakeAgentRuntime` is a contract-test helper, not a provider architecture.
  New real providers should implement `AgentRuntime` in provider leaves.
- `OutputContract::WorkspaceDiff` does not parse diffs and does not create
  artifacts. It only states what kind of output the runtime should leave for a
  stage-owned parser.
- `AgentToolPolicy` is request vocabulary. This crate does not enforce shell or
  tool allowlists; provider leaves or stages must translate/reject them honestly.
- Returning `Metered<AgentSession>` reports cost to the caller. This crate does
  not mutate the engine budget ledger directly.
