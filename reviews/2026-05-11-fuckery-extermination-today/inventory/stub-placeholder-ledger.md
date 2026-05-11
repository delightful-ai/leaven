# Stub And Placeholder Ledger

Status: active findings recorded.

This file records stubs, fake implementations, fixed fixtures, TODOs,
placeholder crates, `unimplemented!`, `todo!`, and public scaffolding.

## Ledger

### Public Fixed Fixtures Masquerading As Optimizer Behavior

- `crates/leaven-gepa/src/proposer.rs:21-47`:
  `ReflectiveMutation` is a fixed edit fixture with a production name.
- `examples/p8_aime_gepa/src/main.rs:91`: the AIME example uses that fixture as
  the reflector.

Correction: rename/move the fixture to a test/demo-only surface or replace it
with the real reflective mutation contract.

### GEPA Placeholder Public Types

- `crates/leaven-gepa/src/proposer.rs:50`: `ReflectiveMutationConfig`
- `crates/leaven-gepa/src/proposer.rs:54`: `SystemAwareMerge`
- `crates/leaven-gepa/src/optimizer.rs:716`: `GepaConfig`
- `crates/leaven-gepa/src/optimizer.rs:720`: `MergeScheduler`

Correction: remove placeholder exports or implement the contracts before
presenting them as user-facing strategy slots.

### Evidence / Preference / Standard Placeholder Vocabulary

- `crates/leaven-evidence/src/lib.rs:1`: module doc still says skeleton.
- `crates/leaven-evidence/src/lib.rs:36-77`: standard evidence names such as
  diff/json/listwise/mixed/score-vector are public empty structs.
- `crates/leaven-preference/src/lib.rs:7`: preference names are mostly public
  markers.
- `crates/leaven-std/src/lib.rs:3`: standard facade re-exports many such names.

Correction: keep only real standard vocabulary in public facades. Move future
names behind explicit scaffolding status until they carry data and laws.

### Provider And Backend Placeholder Crates

- `crates/leaven-lm-anthropic/src/client.rs:1`
- `crates/leaven-lm-local/src/client.rs:1`
- `crates/leaven-workspace-docker/src/factory.rs:1`
- `crates/leaven-workspace-e2b/src/factory.rs:1`
- `crates/leaven-store-sqlite/src/store.rs:1`

Correction: optional features must not imply usable provider/backend
integrations when the public type is inert.

### Derive Macros Are Public Compile Errors

- `crates/leaven-derive/src/lib.rs:9`
- `crates/leaven-derive/src/unimplemented.rs:3`
- `crates/leaven/Cargo.toml:39`

Correction: remove derive from default-facing public API until implemented, or
land a real derive contract.

### Orphan DSRS Directory

- `crates/leaven-dsrs/src/artifact.rs:1`

Correction: delete the orphan or add a real workspace crate with manifest,
library root, tests, and topology contract entry.
