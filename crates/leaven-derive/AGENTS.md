## Boundary
This crate owns Leaven derive macros.

The current derive macros are reserved only: `Artifact`, `ContentAddressed`,
and `EditSurface` expand to explicit compile errors until the real derive
contracts land. Do not silently turn them into partial codegen.

## Current Public-Maturity Status
`Artifact`, `ContentAddressed`, and `EditSurface` are public compile-error
macros. They are reserved API shape, not usable derive behavior, and audit docs
flag them as invalid default-facing surface until real codegen lands.

## Route Away
- Runtime artifact and proposal behavior belongs in `leaven-core` or concrete
  artifact crates, not in macro helpers.
- Surface laws belong in `leaven-surface`; this crate may generate impls only
  after the derive contract names the exact generated surface behavior.
- Test-only public holes for generated code belong nowhere. Prove macro output
  through UI tests and public trait contracts.

## Decision Cards
- when: implementing a reserved derive for real
  do: start from the governing derive contract, then generate the smallest impl
    set needed for the named trait
  preserve: field inclusion by default, explicit skip attributes, and the
    surface-vs-artifact separation
  avoid: one macro that silently implements multiple semantic contracts
  verify: add `trybuild` pass/fail fixtures and the owning trait contract tests
    in `leaven-core` or `leaven-surface`

- when: changing diagnostics or reserved attributes
  do: update `tests/ui/reserved_derives.stderr` intentionally
  preserve: explicit "reserved but not implemented" failure until behavior is
    real
  avoid: letting a derive appear to work while generating partial/no-op impls
  verify: run `cargo nextest run -p leaven-derive`

## Proof Anchors
- `src/lib.rs` declares the reserved macro entry points and accepted attributes.
- `src/unimplemented.rs` owns the explicit compile-error expansion for reserved
  derives.
- `cargo nextest run -p leaven-derive` runs the `trybuild` contract in
  `tests/derive_macros.rs` and proves reserved derives fail with the expected
  message.
- When real codegen lands, add UI pass/fail fixtures and the lowest owning
  crate contract tests for the trait behavior being generated.

## Local Bait
- Attribute names such as `content_skip` and `leaven_surface` are reserved API
  shape, not implemented semantics. Do not document them as behavior until
  codegen and tests exist.
- A passing `cargo check` is not enough for proc-macro contracts. Keep the
  `trybuild` stderr fixtures aligned with intentional diagnostics.
- Do not keep this crate in an ordinary/default import path until the public
  maturity gate can prove the macros generate real trait implementations.
