## Boundary
This is the Rust automation package for repo tasks that are better expressed as compiled code than shell or Python scripts.

Do not add library behavior here; it is only for repository automation with clear local side effects.

Current commands:
- `git-trust-bench`: local-only Git trust/projection/materialization/readback benchmark with an AISI/Inspect-shaped internal structure: task, sample, solver, scorer, environment, and report. It runs focused trust tests unless skipped, writes reports under `target/git-trust-lane/`, can run `--intermediate-count N` to reconstruct every imported child revision in a local Git chain, and explicitly refuses `--environment firkin` until live product-pod benchmark execution is wired.

## Local Rules
- Keep automation deterministic and safe to rerun.
- Prefer typed Rust here when a task needs workspace parsing, structured reports, or cross-platform process handling that would become brittle in shell.
- If an automation task becomes part of the canonical gate, expose it through `Justfile` and document it in `docs/testing/README.md`.
- Do not let `xtask` depend on Leaven library crates unless the automation genuinely needs to inspect Leaven crate metadata or public APIs.
- An empty `cargo run -p xtask` proves only that the automation package builds and exits. Do not cite it as proof of topology, coverage, or product behavior.
- If this grows beyond a single task, add explicit subcommands instead of positional ad hoc behavior. Repository automation should be boring to call from `Justfile` and CI.
- Keep source mutation out of default commands. A task that rewrites files must have a dry-run or check mode before it becomes a canonical gate.

## Decision Cards
- when: moving a Python/shell script into `xtask`
  do: keep the same printed side effects and failure behavior, then document the new invocation in `scripts/AGENTS.md` or `docs/testing/README.md` if the public path changed
  preserve: deterministic local defaults and generated outputs under `target/`
  avoid: hiding network/provider credentials or source rewrites behind a compiled binary
  verify: run `cargo run -p xtask -- <subcommand>` plus the `just` recipe that calls it

- when: adding workspace-inspection automation
  do: parse manifests or metadata structurally instead of grepping strings
  preserve: `crates/leaven/tests/topology_contract.rs` as the executable topology authority
  avoid: a second divergent crate inventory in `xtask`
  verify: run the xtask command and `cargo test -p leaven --test topology_contract`

## Verification
- Run `cargo run -p xtask` for changes here.
- If the task is wired into a `just` recipe, run that recipe too.
