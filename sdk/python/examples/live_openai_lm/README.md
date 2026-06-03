# Live OpenAI LM Proof

This is a self-contained Python example project for the Leaven public seam.
It proves the product path:

```text
Python SDK -> leaven seam serve --stdio -> Python runner -> cx.lm.complete -> OpenAI provider
```

The proof is live-spend gated and skips unless `LEAVEN_LIVE_OPENAI=1` is set.

Run from `sdk/python/`:

```bash
LEAVEN_LIVE_OPENAI=1 uv run --project examples/live_openai_lm live-openai-lm
```

`LEAVEN_OPENAI_MODEL` overrides the default model. `LEAVEN_OPENAI_API_KEY_ENV`
overrides the environment variable name used by the Rust seam service; by
default that is `OPENAI_API_KEY`.

## Dependency Boundary

Public runtime dependency:

- `leaven`: the local SDK under test, declared as an editable path dependency.

Public optional dependencies:

- None.

Private runtime dependencies:

- None in Python. Provider execution is private to the configured Rust seam
  service process.

Private dev dependencies:

- `ruff`
- `ty`

The exact machine-readable declaration lives in
[`pyproject.toml`](pyproject.toml) under `[tool.leaven.dependency-boundaries]`.
