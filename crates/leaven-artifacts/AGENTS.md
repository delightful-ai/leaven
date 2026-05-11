## Boundary
This crate is the standard artifact vocabulary placeholder: text, directory,
and part-map artifact names that future concrete artifacts can share.

Current public structs are reservations, not behavior-bearing artifact
contracts. Do not cite them as proof that Leaven can edit text, directories, or
part maps until they implement core artifact/change traits and surface laws.

## Activation Rules
- Put reusable artifact semantics here only when at least two concrete
  artifact families need the same shape.
- Artifact-specific VCS facts belong in `leaven-artifact-git` or
  `leaven-artifact-jj`; skill-bank facts belong in `leaven-artifact-skill`.
- Surface projection laws belong in `leaven-surface` unless this crate owns the
  artifact-specific surface implementation too.

## Verification
- `cargo check -p leaven-artifacts` only proves the reservation names compile.
- When a placeholder becomes real, add trait/law tests in this crate and update
  `crates/leaven-std/AGENTS.md` or `crates/leaven/AGENTS.md` only if the facade
  should expose the new behavior.
