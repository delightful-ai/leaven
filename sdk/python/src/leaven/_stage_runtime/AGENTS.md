## Boundary

`leaven._stage_runtime` owns private Python stage-context bindings. It turns an
already-authorized stage dispatch plus a callback client into concrete
`RolloutContext` / builder objects.

It is private SDK runtime machinery, not public API. Public users should see
only `cx.lm`, `cx.agent`, and role-specific context types supplied by Leaven.

## Public Dependencies

- Public context and builder types from `leaven.contexts` and
  `leaven.builders.*`.
- The locked `leaven/lm.complete` callback semantics from the public seam.

## Private Dependencies

- Sibling modules in this package.
- Callback protocols implemented by host/worker drivers such as
  `leaven._serve._ParentAgent`.
- No imports from `leaven._serve`; drivers depend on this package, never the
  other way around.

## Map

- `protocols.py`: private protocols required from the stage driver.
- `lm.py`: callback-backed `LmBuilder` implementation and response projection.
- `contexts.py`: role-scoped concrete context objects.
- `__init__.py`: map-only re-exports.
