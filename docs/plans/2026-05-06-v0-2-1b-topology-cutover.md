# Leaven v0.2.1b Topology Cutover Plan

## Goal

Hard-cut the workspace from the earlier v0.2.1a first-two-subsystems shape to
the corrected v0.2.1b crate topology in
`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`.

## Governing Specs

- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/initial_library.md`

When they disagree, the corrected v0.2.1b topology wins for crate ownership:
`leaven-core` is cold optimizer algebra only, and graph/context/stage/runtime
behavior belongs in `leaven-engine`.

## Cutover Decisions

- Move universal IDs, content IDs, cost values, metadata, error records,
  fingerprints, and time primitives into `leaven-kernel`.
- Keep `leaven-core` free of `RunGraph`, `RunContext`, stage traits, workspace
  traits, stores, renderers, components, and `Decomposable`.
- Replace artifact-intrinsic decomposition with `leaven-surface::EditSurface`.
- Keep generic blob/evidence/checkpoint storage traits in `leaven-store`; the
  store crate does not know the run graph.
- Keep workspace/sandbox substrates in `leaven-workspace`; the workspace crate
  does not know artifacts, surfaces, stores, or the engine.
- Keep `RunGraph` storage, graph views, budget ledger, stage traits, trust
  policy, reports, events, and `RunContext` in `leaven-engine`.
- Add spec-listed standard, optimizer, LLM, agent, workspace backend, store
  backend, and domain adapter crates as compiling skeletons with dependency
  edges matching the allowlist.

## Verification

- Topology contract tests in `crates/leaven/tests/topology_contract.rs` assert
  the full workspace member list, each crate's `src/lib.rs` skeleton, the
  Leaven-to-Leaven dependency DAG, and cold-core leak boundaries.
- Engine graph tests in `crates/leaven-engine/tests/graph_surface.rs` assert
  the first graph laws still hold through `RunContext` after the crate split.
- Completion gate remains `just check`.
