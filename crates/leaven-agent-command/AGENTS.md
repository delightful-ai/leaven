## Boundary
This crate owns the command-backed agent runtime substrate: rendering an
`AgentRunRequest` into setup/run commands, executing through `WorkspaceView`,
capturing command records, and parsing command output into `AgentSession`.

It is a reusable provider-runtime substrate, not a specific provider and not an
agentic stage adapter.

## Map
- `CommandAgentConfig` describes setup commands, the run command, prompt
  delivery mode, and retained raw event behavior.
- `CommandTemplate` and `CommandTemplateArg` keep command construction typed
  enough that provider leaves can build commands without string-splicing whole
  shells unless the provider explicitly needs a shell.
- `CommandSessionParser` converts captured command output plus workspace state
  into the provider-neutral `AgentSession`.
- `CommandAgentRuntime` validates the output contract after parser success.

## Route Away
- Provider-specific flags and defaults belong in provider crates such as
  `leaven-agent-codex-cli`; this crate should not know Codex, Claude Code, or
  OpenCode option names.
- Workspace backend mechanics stay in `leaven-workspace-*`; this crate calls
  `WorkspaceView::run_command` and does not require host paths.
- Proposal/evidence parsing belongs in `leaven-agentic`; command parsers only
  build an `AgentSession`.

## Proof Anchors
- `crates/leaven-agent-command/tests/command_runtime.rs` proves setup/run order,
  env/template handling, stdin rendering, output-contract validation, raw event
  retention, cancellation, and command records.
- `docs/specs/agentic_stage_runtime.md` section "Harbor-style command-backed
  provider path" owns why this crate is the backend-neutral runtime substrate.
- Run `cargo test -p leaven-agent-command` to prove command-backed
  runtime behavior against the local workspace backend.

## Decision Cards
- when: adding a new CLI-backed provider runtime
  do: put provider defaults and option names in the provider leaf, then lower to `CommandAgentConfig` / `CommandTemplate`
  preserve: this crate as reusable substrate over `WorkspaceView::run_command`
  avoid: adding provider enum variants, Codex flags, Claude/OpenCode defaults, or optimizer words here
  verify: run the provider leaf tests plus `cargo test -p leaven-agent-command`

- when: changing prompt delivery
  do: prefer `CommandPromptMode::StdinInstructions` when system, task, and context refs must survive together
  preserve: `CommandPromptMode::StdinTask` as a narrow tool for task-only command fixtures
  avoid: switching a provider adapter to task-only stdin and silently dropping system/developer instructions or context file refs
  verify: extend `command_runtime_can_pass_rendered_instructions_to_stdin` or the provider leaf parser test that consumes the prompt

- when: command construction seems to need shell syntax
  do: use explicit `sh -c` only when the provider operation really is shell-shaped
  preserve: ordinary args as typed `CommandTemplateArg` values so workspace backends see the command shape
  avoid: string-splicing complete provider commands here for convenience
  verify: assert the rendered `CommandRecord.command` in `command_runtime.rs` or the provider leaf test

## Local Bait
- Do not use `std::process::Command` or host `PathBuf` directly here. The
  backend-neutral promise is that commands run through `WorkspaceView`.
- Do not make command stdout mean proposal text globally. That is only the
  default `StdoutSessionParser`; provider leaves and agentic parsers own richer
  interpretations.
- Raw stdout/stderr retention is evidence capture, not normalized transcript
  semantics. A provider parser should choose the stable channel first, then
  retain raw streams as raw provider events.
