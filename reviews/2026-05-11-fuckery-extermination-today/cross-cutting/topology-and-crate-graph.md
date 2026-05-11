# Topology And Crate Graph

Status: active findings recorded.

This file audits whether the current workspace graph matches the repo-level
crate boundary contract and corrected topology specs.

## Findings

### X-001: `leaven-dsrs` is named in topology but not a workspace crate

- severity: high
- evidence:
  `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:132`,
  `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:210`,
  `crates/leaven-dsrs/src/artifact.rs:1`
- promised behavior: `leaven-dsrs` is a real domain adapter crate.
- actual behavior: `crates/leaven-dsrs` has no `Cargo.toml`, no `src/lib.rs`,
  is not a root workspace member, and contains only one-line public structs.
- why it matters: future DSRS/GEPA work can route toward a crate that is not
  compiled or tested.
- correction direction: delete the orphan or hard-cut it back in as a real
  workspace crate with a manifest, lib root, tests, and topology contract entry.

### X-002: Topology tests verify skeleton presence, not public maturity

- severity: medium
- evidence: `crates/leaven/tests/topology_contract.rs:421-459`
- promised behavior: topology tests protect crate-boundary health.
- actual behavior: tests prove membership, manifests, `src/lib.rs` skeletons,
  and exact dependency edges. They do not reject orphan crate directories,
  skeleton descriptions, empty public unit structs, or public placeholder
  exports.
- why it matters: the topology test can pass while large parts of the public
  graph remain inert.
- correction direction: extend topology checks to scan all `crates/*`, reject
  unregistered dirs, and ledger/deny public stubs unless explicitly allowed.

### X-003: GEPA cache dependency direction is ambiguous in specs

- severity: medium
- evidence: `docs/specs/gepa_optimizer_surface.md:174`,
  `docs/specs/gepa_optimizer_surface.md:190`,
  `crates/leaven-gepa/Cargo.toml:13`,
  `crates/leaven/tests/topology_contract.rs:272`
- promised behavior: dependency direction should be unambiguous.
- actual behavior: the GEPA spec both allows and forbids `leaven-lm-cache`; the
  live manifest and topology contract omit it.
- why it matters: implementors cannot tell whether cache composition belongs
  inside GEPA, above GEPA, or only inside LM runtime configuration.
- correction direction: choose one boundary and update the spec, manifest, and
  topology contract together. Current audit pressure favors cache configuration
  above GEPA, with GEPA consuming LM/agent capabilities.

### X-004: Public modules expose file layout as API

- severity: medium
- evidence: `crates/leaven-gepa/src/lib.rs:3`,
  `crates/leaven-workspace/src/lib.rs:3`,
  `crates/leaven-agent-command/src/lib.rs:3`,
  `crates/leaven-agent-codex-app-server/src/lib.rs:7`
- promised behavior: crate roots are curated maps: module declarations,
  deliberate re-exports, optional preludes.
- actual behavior: several crates expose internal module layout as `pub mod`
  alongside curated exports.
- why it matters: downstream users can depend on file-layout paths that should
  remain private design freedom.
- correction direction: make modules private by default and expose only
  deliberate root/prelude contracts.
