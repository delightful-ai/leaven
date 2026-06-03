## Boundary

`leaven._seam` is the private Python process client for the Leaven-owned public
seam server. It owns typed private config records, JSON-RPC request construction,
binary/repo discovery, and one-shot `leaven seam serve --stdio --config`
process execution.

It is private substrate for scaffold examples and future role-scoped builders.
It must not become the public SDK surface; public users should eventually reach
this route through `lv.optimize(...).run()`, `cx.agent.run`, `cx.lm.complete`,
or other documented builders.

## Public Dependencies

- Python standard library only.
- The installed/built `leaven` CLI public command:
  `leaven seam serve --stdio --root <repo> --config <json>`.
- The locked Leaven public seam JSON-RPC/Plan IR wire in
  `docs/specs/public-seam-v1/`.
- Provider executables passed as configuration, currently Codex CLI and
  command-runner stage worker processes.
- Capability actions enforced by `leaven-public-seam`, currently
  `lm.complete`, `workspace.materialize`, `agent.run`, and
  `proposal.submit_batch` for these helpers.

## Private Dependencies

- Sibling modules inside `leaven._seam`.
- No imports from `leaven._serve`; that module owns the older bidirectional
  prompt-optimization scaffold path.
- Public scaffold builders may import this package only through focused,
  private bound-client slices such as `AgentBuilder.run` and
  `LmBuilder.complete`; `_seam` must not import those builders back.

## Map

- `resolve.py`: repo, Leaven CLI, and Codex CLI discovery.
- `config.py`: private service config records serialized for Rust.
- `capability.py`: current effect/proposer capability helpers for
  mechanics/live proofs.
- `plans.py`: Plan IR / JSON-RPC request construction for `agent.run`,
  `lm.complete`, `proposal.submit_batch`, runner `stage.run`, and proposer
  `stage.run`.
- `client.py`: one-shot process execution and JSON-RPC result/error handling.
- `__init__.py`: map-only re-exports.

## Verification

When changing this package:

```bash
cd sdk/python
uv run python -c "import py_compile; from pathlib import Path; [py_compile.compile(str(p), doraise=True) for p in Path('examples').glob('*.py')]; print('compiled examples')"
uv run ruff check src/leaven examples --exclude src/leaven/_types
uv run ty check src/leaven --exclude src/leaven/_types
uv run python examples/10_live_codex_seam.py
```

Run the live proof only with explicit spend intent:

```bash
LEAVEN_LIVE_CODEX=1 uv run python examples/10_live_codex_seam.py
```
