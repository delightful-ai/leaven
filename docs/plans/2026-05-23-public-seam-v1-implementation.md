# Public Seam V1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the locked `docs/specs/public-seam-v1` package executable through an owning Rust seam surface without claiming unimplemented worker runtime rows.

**Architecture:** Add `crates/leaven-public-seam` as the public-seam wire-contract owner. The first slice proves active package authority, manifest inventory, schema compilation, RFC 8785 + SHA-256 schema fingerprints, matrix row structure, and deferred markers; worker execution, ACP runtime, and graph mutation rows stay pending until their real surfaces exist.

**Tech Stack:** Rust 2024, `serde`, `serde_json`, `serde_yml`, `sha2`, `jsonschema`, `expect-test`, `jj`.

---

### Task 1: Contract Owner And Harness Skeleton

**Files:**
- Create: `crates/leaven-public-seam/Cargo.toml`
- Create: `crates/leaven-public-seam/AGENTS.md`
- Create: `crates/leaven-public-seam/src/lib.rs`
- Create: `crates/leaven-public-seam/tests/contract_package.rs`
- Modify: `Cargo.toml`
- Modify: `crates/AGENTS.md`
- Modify: `crates/leaven/tests/topology_contract.rs`

**Steps:**
1. Write failing tests for active-package-only loading, manifest inventory, schema compilation, schema fingerprint stability, matrix row uniqueness, and deferred watch/worker markers.
2. Run `cargo test -p leaven-public-seam --test contract_package` and confirm the tests fail because the owner API is missing.
3. Implement the smallest owner API needed by the tests. Keep runtime worker behavior out of this slice.
4. Run the focused test and `cargo test -p leaven --test topology_contract`.
5. Commit the slice with a rich jj message.

### Task 2: Row Evidence Discipline

**Files:**
- Modify: `docs/specs/public-seam-v1/conformance-matrix.yaml`
- Create: `docs/specs/public-seam-v1/reviews/2026-05-23-initial-harness.md`

**Steps:**
1. Run an adversarial sub-agent review over the implemented slice, matrix rows, tests, and owner docs.
2. Resolve blocking findings in code/tests/docs or record why they do not block.
3. Update only rows proven by executable evidence plus reviewer sign-off; leave runtime and worker rows pending.
4. Run the focused tests, topology contract, and then `just check` if the slice remains feasible.
