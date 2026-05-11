## Boundary
This crate is the future SQLite backend for `leaven-store` capabilities.

Current public names are scaffolding. `SqliteStore` does not yet prove schema,
migrations, transaction boundaries, reopen behavior, or durability.

## Local Bait
- SQLite schema belongs here only as backend layout. Do not define run graph,
  evidence, or product result schema here.
- Migration tests should be fixture-backed and versioned before this backend is
  exposed through ordinary defaults.

## Verification
- `cargo check -p leaven-store-sqlite` proves only scaffold exports.
- Real behavior needs migration/reopen/transaction tests plus
  `cargo test -p leaven --test topology_contract` if dependencies or facade
  exposure change.
