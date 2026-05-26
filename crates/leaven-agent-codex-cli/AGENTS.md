## Boundary
This crate owns the backend-neutral Codex CLI runtime adapter.

It builds a `codex exec` command config, delegates execution to
`leaven-agent-command`, reads the stable last-message file, retains configured
raw stdout/stderr, and returns a provider-neutral `AgentSession`.

## Map
- `CodexCliConfig` owns CLI binary, model, reasoning effort, approval/sandbox
  mode, explicit goal-mode feature opt-in, `CODEX_HOME`, timeout, and
  last-message path.
- `CodexCliRuntime` is an `AgentRuntime` implemented by delegation to
  `CommandAgentRuntime<CodexCliSessionParser>`.
- `CodexCliSessionParser` treats `--output-last-message` as the stable final
  assistant channel and uses stdout fallback only for command-backed test
  binaries.
- Skill discovery stays native to Codex in the workspace; this runtime does not
  copy skill banks into a private home.

## Route Away
- Codex app-server protocol and typed JSON-RPC handling belong in
  `leaven-agent-codex-app-server`.
- Command execution mechanics belong in `leaven-agent-command`.
- Skill bank layout, `.agents/skills` materialization, and mutated skill parsing
  belong in `leaven-agentic-skill` and `leaven-artifact-skill`.
- Proposals, assessments, engine graph, and GEPA rhythm belong above this leaf.

## Proof Anchors
- `crates/leaven-agent-codex-cli/tests/codex_cli_runtime.rs` proves command
  template construction, approval/sandbox variants, native skill-discovery
  non-copying, explicit Codex goal-mode opt-in, runtime identity/capabilities,
  last-message parsing, and stdout fallback.
- `docs/specs/codex_cli_agent_runtime.md` owns this adapter's product path.
- Run `cargo nextest run -p leaven-agent-codex-cli` to prove deterministic CLI
  adapter behavior without live Codex.
- Run `cargo check -p leaven-agent-codex-cli` to prove the direct provider leaf
  route.

## Decision Cards
- when: changing Codex CLI invocation
  do: assert the rendered command vector in `codex_cli_config_builds_backend_neutral_exec_template`
  preserve: `codex exec`, stdin-rendered instructions, `--output-last-message`, backend-neutral cwd, and execution through `leaven-agent-command`
  avoid: relying on host cwd, private Codex home copying, JSONL stdout as normalized transcript, or app-server protocol crates
  verify: run `cargo nextest run -p leaven-agent-codex-cli`

- when: changing approval or sandbox defaults
  do: make the mode explicit in `CodexCliConfig` and tests
  preserve: `WorkspaceWrite` as the ordinary sandbox default and bypass as an opt-in configuration used by known live reproductions
  avoid: hiding `--dangerously-bypass-approvals-and-sandbox` behind a convenience constructor
  verify: run `cargo nextest run -p leaven-agent-codex-cli` and inspect the expected argv in the config test

- when: changing Codex goal-mode behavior
  do: keep it as an explicit `CodexCliConfig` flag that renders Codex's feature switch
  preserve: all goal/spec/stage/jj policy above this provider leaf
  avoid: teaching the CLI runtime about proposals, evals, proof denominators, or jj snapshots
  verify: keep `codex_cli_goal_mode_is_explicit_feature_flag` passing

- when: changing skill behavior for Codex CLI
  do: keep native Codex skill discovery as provider behavior over the already-materialized workspace
  preserve: this runtime not copying `.agents/skills` or owning `SkillBank` layout
  avoid: creating a private `CODEX_HOME` skill mirror here
  verify: keep `codex_cli_config_leaves_repo_skills_native` passing

## Local Bait
- `--dangerously-bypass-approvals-and-sandbox` is a configurable mode for cases
  where the workspace backend is already the sandbox boundary. Do not make it a
  hidden default for all Codex CLI use.
- Do not parse Codex JSONL stdout as the durable transcript contract while the
  last-message file is the stable assistant-message channel.
- `live_codex_runtime` in P5 currently discards the developer-instructions
  argument before constructing `CodexCliRuntime`; P5 renders role instructions
  into request bodies separately. Do not cite this runtime config as proof of a
  provider-level developer-instruction channel.
