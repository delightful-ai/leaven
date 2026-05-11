## Boundary
This crate owns the Codex app-server runtime leaf: app-server protocol access,
connector/transport seams, Codex config lowering, transcript normalization,
raw event policy, and `CodexAppServerRuntime`.

It is not the default backend-neutral Codex path, not an agentic stage adapter,
and not a skill optimizer.

## Map
- `config.rs` owns Codex app-server initialize/thread/turn config vocabulary
  and approval/raw-event policy.
- `transport.rs` owns `CodexAppServerConnector`, `CodexAppServerTransport`,
  and the stdio connector. Stdio requires a host-local mount.
- `client.rs` owns app-server JSON-RPC interaction behind the `app-server`
  feature.
- `history.rs` owns Codex notification/history normalization into Leaven
  transcript, commands, output files, and raw provider events.
- `runtime.rs` maps `AgentRunRequest` into one Codex thread/turn and validates
  the provider-neutral output contract.

## Route Away
- `codex-app-server-protocol` and `codex-protocol` must stay leaf-only here.
  Do not import them from `leaven-agent`, `leaven-agentic`, workspace crates, or
  the umbrella crate.
- Backend-neutral Codex CLI execution belongs in `leaven-agent-codex-cli`.
- Provider-neutral runtime vocabulary belongs in `leaven-agent`.
- Skill layout ownership belongs in `leaven-agentic-skill` and
  `leaven-artifact-skill`; this crate may describe Codex provider ABI over
  already-materialized files, not own skill validation.
- Graph mutation, proposal repair, and evaluator scoring belong in
  `leaven-agentic` or engine-facing stages.

## Proof Anchors
- `docs/specs/codex_app_server_agent_runtime.md` owns this leaf's protocol,
  feature, connector, and local-mount semantics.
- `cargo check -p leaven-agent-codex-app-server --no-default-features` proves
  config/error vocabulary builds without protocol dependencies.
- When touching gated protocol, client, transport, or runtime code, run
  `cargo check -p leaven-agent-codex-app-server --features app-server`; it is
  currently a known-failing drift gate until the initializer supplies the
  vendored `InitializeCapabilities::request_attestation` field. When it passes,
  it proves Codex protocol drift has been reconciled in this leaf.
- When touching the stdio connector, run
  `cargo check -p leaven-agent-codex-app-server --features stdio`; this
  currently fails for the same vendored protocol drift because stdio enables
  the app-server path. When it passes, it proves the local-mount connector path
  builds.
- `cargo test -p leaven --test topology_contract` proves app-server protocol
  dependencies remain leaf-only.
- `LEAVEN_CODEX_LIVE=1 cargo test -p leaven-agent-codex-app-server --features live-codex-tests -- --ignored`
  is currently blocked by the same vendored protocol drift because
  `live-codex-tests` enables `stdio`, which enables `app-server`. After the
  drift is fixed, it proves the opt-in live stdio path against local Codex auth.

## Decision Cards
- when: changing initialize/thread/turn mapping
  do: update typed config, fingerprint inputs, protocol conversion tests, and runtime request mapping together
  preserve: materialized non-ephemeral threads by default, fail-closed approval mode, and provider raw events as opt-in evidence
  avoid: accepting protocol drift by weakening config types or moving protocol fields into neutral crates
  verify: run `cargo check -p leaven-agent-codex-app-server --features app-server`; until the known vendored drift is fixed, keep the failure tied to this leaf

- when: adding a connector
  do: implement `CodexAppServerConnector` with an honest `WorkspaceAccessMode`
  preserve: stdio as `RequiresLocalMount` and remote/container support as separate connectors rather than new meanings for stdio
  avoid: claiming backend-neutral support when `WorkspaceView::local_mount()` is required
  verify: run `cargo check -p leaven-agent-codex-app-server --features stdio` for stdio or add a connector-specific test under the `app-server` feature

- when: normalizing history/transcripts
  do: map Codex notifications into provider-neutral `AgentSession` facts and retain raw events only according to `CodexRawEventPolicy`
  preserve: `AgentSession` as observation, not proposal/evidence interpretation
  avoid: importing `SkillBank`, `Proposal`, `Assessment`, GEPA, or run graph vocabulary because a notification looks semantically rich
  verify: extend the runtime/history tests under the `app-server` feature

## Local Bait
- Stdio is `RequiresLocalMount`, not container-neutral. Do not hide it behind a
  backend-neutral capability claim.
- Codex approval requests should fail closed unless the config explicitly names
  a different approval mode.
- If a vendored Codex protocol field changes, update this crate's request
  mapping and history normalization rather than moving protocol types into
  `leaven-agent` or `leaven-agentic`.
- A missing protocol field such as `InitializeCapabilities::request_attestation`
  is app-server protocol drift to fix in this crate's request mapping, not a
  reason to weaken the leaf-only dependency boundary.
- Do not copy DSRs repo-agent ownership wholesale. Leaven copies app-server
  transport/history ideas while keeping repo materialization and graph readback
  in Leaven stages.
- The live stdio test spends local Codex auth/runtime and requires
  `LEAVEN_CODEX_LIVE=1`, but it cannot run until the app-server feature drift
  is fixed. When unblocked, it proves provider-adapter connectivity, not an
  agentic paper reproduction or product builder path.
