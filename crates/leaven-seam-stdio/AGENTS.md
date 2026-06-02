## Boundary
This crate owns the line-delimited stdio adapter for the Leaven public seam
runtime.

It reads JSON-RPC request values from a `BufRead`, delegates request semantics
to `leaven-seam-runtime`, and writes one JSON-RPC response per input line. It
may translate malformed JSON lines into JSON-RPC parse errors. It must not own
public-seam schemas, method dispatch law, provider execution, graph mutation,
process spawning policy, or SDK demo plans.

## Proof
- `tests/stdio_contract.rs` proves malformed input, multiple request lines, and
  valid stage-run success all flow through the runtime over stdio.

## Verification
- Run `cargo test -p leaven-seam-stdio` after changing stdio adapter behavior.
