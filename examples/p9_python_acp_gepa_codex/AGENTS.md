## Boundary
This example owns the P9 live denominator: a Rust host harness runs live Codex,
launches a Python ACP subprocess, validates locked V1 ACP request/response
envelopes, and records a tiny GEPA-shaped seed-to-child acceptance artifact.

This is not the Python SDK implementation. Do not add behavior to
`docs/specs/leaven_py` to satisfy this example.

## Proof Classification
`just milestone-p9` is a live mechanics proof. It proves:

- live Codex execution through `leaven-agent-codex-cli`
- a Python worker process across the locked `leaven-acp` stdio seam
- public-seam validation of the ACP extension responses
- a two-candidate GEPA-shaped accept decision with a durable
  `result_summary.json`

It does not prove durable Codex agent-kit installation, Codex hooks, a real
Python `leaven` package, full GEPA optimizer policy, ACP bidirectional provider
execution, or paper metric reproduction.

## Verification
Run the cheap compile/unit gate with:

```bash
cargo test -p p9_python_acp_gepa_codex
```

Run the live proof with:

```bash
LEAVEN_P9_LIVE=1 LEAVEN_CODEX_LIVE=1 just milestone-p9
```

The live command may spend provider credits and is allowed to write run
artifacts under `tmp/p9_python_acp_gepa_codex/`.
