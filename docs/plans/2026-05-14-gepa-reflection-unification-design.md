# Tombstone: GEPA Reflection Unification Design

Status: implemented and superseded.

This file used to specify the GEPA reflection unification slice: build the
`ReflectRequest` once, restore case input visibility, make reflective dataset
selection swappable, and delete the old divergent feedback paths.

Current owners:

- `docs/specs/gepa_reference_behavior.md`
- `docs/specs/gepa_optimizer_surface.md`
- `crates/leaven-gepa/AGENTS.md`
- `crates/leaven-gepa/src/reflection.rs`
- `crates/leaven-gepa/tests/gepa_contract`

Do not reintroduce `SelectedFeedback`, `GepaReflectionEvidence`, or reflector
local feedback projection from this historical design note.
