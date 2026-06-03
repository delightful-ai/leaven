## Boundary

`leaven.run_status` owns public, user-visible status facts attached to run
results. It names what Leaven knows, what it does not know yet, and which
declared dependency is responsible for an unsupported status.

It may define small immutable public records and pure projection helpers used by
`RunSummary`. It must not import private seam clients, provider config builders,
worker protocols, process adapters, or optimizer mechanics.

## Public Dependencies

- Python standard-library typing only.
- Pydantic public model base/config for SDK records.
- Stable string literals that are safe to serialize in result JSON.

## Private Dependencies

- None. Private mechanics packages may import this package, but this package
  must not import them back.

## Map

- `unsupported.py`: public unsupported-run-fact record and literal vocabularies.
- `cost.py`: cost/usage status literals and pure summary projection helpers.
- `__init__.py`: map-only re-export.
