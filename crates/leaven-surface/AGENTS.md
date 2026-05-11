## Boundary
This crate owns explicit read/edit surfaces over artifacts: parts, addresses,
selections, path surfaces, surface errors, and surface-definition fingerprints.

A surface is a chosen projection. It must not imply that every artifact has one
intrinsic decomposition or that artifact identity is the same thing as part
identity.

## Route Here
- Generic surface vocabulary belongs here: `EditSurface`, `Part`, `PartView`,
  `PartAddress`, `PartSelection`, `SurfaceError`, and `SurfaceFingerprint`.
- Projection rules that are artifact-neutral or path-generic belong here.
- Fingerprint changes that describe how a surface interprets an artifact belong
  here; artifact content fingerprints stay in `leaven-kernel`/`leaven-core`.

## Local Helper Stack
- Use `EditSurface::parts` for projection and `EditSurface::change_part` for
  translating a part edit into an artifact-native `Change`; this crate never
  applies the change.
- Use `PartId` for stable identity under a surface and `Address` for the
  external locator shown to humans, prompts, CLIs, or agents. They may be equal
  for path surfaces and different for manifest/frontmatter surfaces.
- Use `SurfaceFingerprint` as part of any downstream evidence/cache key that
  names a part. A `PartId` without the surface fingerprint is not globally
  meaningful.
- Use `PartSelection::Only` when a caller is declaring a scope; enforcement of
  read/trust scope still belongs in engine or the calling stage.

## Route Away
- Artifact traits, change application, content/cache identity, proposals, and
  evaluation vocabulary belong in `leaven-core`.
- Concrete artifact family parsing or semantic views belong in
  `leaven-artifact-*` or `leaven-artifacts`; this crate supplies the surface
  trait and generic path surface, not every domain surface.
- Rendering an artifact into a workspace, materializing files, truncation
  policy, and renderer/materializer traits belong in `leaven-engine` or
  `leaven-render`.
- Optimizer surface-choice policy, reflection gates, and GEPA-specific lowering
  belong in optimizer crates such as `leaven-gepa`.

## Decision Cards
- when: adding a new generic surface trait method or error
  do: prove it for an artifact-neutral law here, then update concrete artifact
    surfaces as consumers
  preserve: projection/edit translation as pure functions over the artifact
  avoid: adding file materialization, workspace IO, or graph mutation to the
    surface contract
  verify: run `cargo nextest run -p leaven-surface`

- when: building a path-like surface for a concrete artifact
  do: reuse `PathPartId`, `PathAddress`, and `PathSurfaceConfig` only if path is
    the identity
  preserve: rename as remove/add for path identity
  avoid: pretending path identity provides logical continuity
  verify: add the concrete artifact-surface test plus
    `cargo nextest run -p leaven-surface --test part_contract`

## Proof Anchors
- `src/lib.rs` is the vocabulary map and records the surface laws. Keep
  implementation in the owning surface module.
- `tests/part_contract.rs` proves semantic payloads live in the surface view,
  not in an intrinsic artifact decomposition.
- `cargo nextest run -p leaven-surface` proves the local projection vocabulary.
- `cargo test -p leaven --test topology_contract` proves `leaven-surface`
  remains below engine/run and keeps the intended dependency edge shape.

## Local Bait
- Path identity is intentionally path identity. Rename continuity requires a
  concrete surface that extracts a logical ID; do not add rename semantics to
  `PathSurfaceConfig` by guessing from file movement.
- `SurfaceFingerprint` must change when interpretation changes. Cache bugs from
  stale fingerprints are surface bugs, even when they appear as evaluator cache
  hits later in the engine.
- Borrowed `View<'a>` values are inspection views. Convert to owned request or
  render data before async/provider boundaries instead of extending the surface
  trait to own runtime behavior.
- Do not route "component" or "decomposable artifact" concepts back into
  `leaven-core`; use an explicit surface.
