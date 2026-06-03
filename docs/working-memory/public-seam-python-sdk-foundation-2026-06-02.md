# Public Seam Python SDK Foundation — 2026-06-02

Status: active foundation slice, not the full Python SDK acceptance gate.

## Proven

- Commit `08562b5a` / change `wpqmklvw`: `leaven seam serve --stdio` executes
  `leaven/lm.complete` through `leaven-seam-service`.
- Commit `65060380` / change `tuolmqou`: `leaven-seam-service` executes
  capability-bound `workspace_materialize` + `agent_run` by composing
  `leaven-public-seam`, `leaven-workspace-local`, and
  `leaven-agent-codex-cli`.
- Commit `eb65497c` / change `trynuqxu`: `agent_session.transcript_ref` hashes
  the provider-neutral session transcript bytes instead of a placeholder.
- Commit `b8662920` / change `mnnxlvoo`: Python scaffold example
  `docs/specs/leaven_py/examples/10_live_codex_seam.py` drove the same public
  seam process and live Codex path from Python.
- Current slice: the runnable Python SDK project has hard-cut over to
  `sdk/python`, with `leaven._seam` as a split private module package for the
  public-seam process client.

## Verification Run

Rust/service:

- `cargo test -p leaven-seam-service`
- `cargo test -p leaven-cli`
- `cargo test -p leaven --test topology_contract`
- `python3 scripts/lint-line-count.py` passed with the pre-existing unrelated
  warning for `crates/leaven-acp/src/stdio.rs`.

Process-level live proof:

- `cargo run --quiet -p leaven-cli -- seam serve --stdio --root . --config <tmp>`
  with `/Users/darin/.codex/packages/standalone/current/codex`, model
  `gpt-5.4-mini`, returned completed `agent_session`, workspace and agent
  receipts, Codex CLI argv with `--sandbox workspace-write`, and transcript ref
  `bytes = 388`.

Python SDK project:

- `uv run python -c "import py_compile; from pathlib import Path; [py_compile.compile(str(p), doraise=True) for p in Path('examples').glob('*.py')]; print('compiled examples')"`
- `uv run ruff check src/leaven examples --exclude src/leaven/_types`
- `uv run ty check src/leaven --exclude src/leaven/_types`
- `uv run python examples/run_all.py`
- `uv run python examples/10_live_codex_seam.py` skips without
  `LEAVEN_LIVE_CODEX=1`.
- `LEAVEN_LIVE_CODEX=1 uv run python examples/10_live_codex_seam.py` completed
  with `gpt-5.4-mini`, transcript bytes `402`, and receipts
  `wrec_workspace, agentrec_completion`.

## Still Unproven

- `sdk/python` ergonomic `AgentBuilder.run` / `cx.agent.run` is still scaffold.
  Example 10 is a direct JSON-RPC client, not the high-level SDK path.
- Reward-vector execution from Python remains scaffold. `@lv.reward` bodies are
  not yet executed over the public seam except for the prior host-side exact
  match path in example 03.
- Blob refs are verified metadata, but the service does not persist or serve
  blob contents yet. Inspection can see refs, not fetch the transcript/stdout
  bytes through a public API.
- Codex CLI cost remains `{}` because the provider adapter records zero cost.
  Live spend happened, but the result is not yet cost-accounted.
- Workspace release/cleanup is not exposed through the service path; current
  materialized workspaces live for the service host lifetime.
- `lv.runs.open(...)`, lineage inspection, evidence query, and optimized run
  replay are not proven for the Python/Codex path.
- Full acceptance gate remains open: a P5-shaped `optimize(...).run()` through
  Python, with live LM, live Codex agentic stage, reward vector, receipts,
  evidence, lineage, cost, and inspectable output.

## Next Slices

1. Wire `AgentBuilder.run` for a single configured Codex CLI runtime through
   the private `leaven._seam` substrate, still live-gated in examples.
2. Add blob persistence/readback to `leaven-seam-service` or record an explicit
   unsupported-provider error if blob fetch is requested before storage exists.
3. Decide the cost bridge for Codex CLI: either parse provider usage from Codex
   JSONL when available or return a typed unsupported-cost marker instead of
   `{}`.
4. Move from direct `agent.run` proof to a tiny Python `optimize(...).run()`
   proof that uses a live agent stage and at least one Python-authored reward.
