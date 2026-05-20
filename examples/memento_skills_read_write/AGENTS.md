## Boundary
This example is the Memento-Skills paper lane for a tiny Read-Write live loop.
It is deliberately outside the Cargo workspace so it can advance the paper
replication without touching root workspace membership or the concurrent
`default` workspace edits.

## Proof Model
- `bash examples/memento_skills_read_write/scripts/run_tiny_live.sh --preflight`
  is a no-spend source/fixture sanity check. It is not replication proof.
- `LEAVEN_CODEX_LIVE=1 bash examples/memento_skills_read_write/scripts/run_tiny_live.sh --live`
  runs one tiny train case through Codex with `gpt-5.4-mini`: Observe, Read,
  Act, Feedback, Write, unit-test gate, retry, and report persistence.
- The live run spends provider/runtime resources through the Codex CLI. It
  writes generated artifacts under `tmp/memento_skills_read_write/`.

## Fidelity
- Preserve the Memento-Skills state transition: skill library, router/read
  selection, skill-conditioned execution, judge feedback, utility/error state,
  failure attribution, skill rewrite, validation gate, and retry.
- The tiny runner uses Codex as the general-purpose agent harness. It does not
  introduce a custom agent runtime.
- Document every paper deviation in `README.md` and the generated report.

## Bait
- Do not cite preflight as proof of Memento-Skills replication.
- Do not treat the tiny deterministic judge or one hand-authored train case as
  GAIA/HLE parity.
- Do not move router, utility, or write-loop substrate into Leaven until more
  paper examples prove a shared seam.
