# Public Maturity Gates

Status: integrated refinement pass.

This doc captures the crate-graph refinement: topology alignment is not product
alignment. A public name is mature only when it is safe for the intended user
layer to depend on it.

## Scaffolding Categories

### Private Fixture

Allowed when:

- it is not re-exported from ordinary facades or preludes;
- examples/tests name it as a fixture;
- it cannot be mistaken for production capability.

Examples that should move here: fixed-edit GEPA mutation helpers.

### Test-Support Public

Allowed when:

- the module path says `test`, `fake`, or equivalent;
- feature gating makes intended use clear;
- docs say it is for tests/examples.

Examples to classify carefully: fake agent runtimes and scripted LM mocks.

### Explicit Scaffold Feature

Allowed only if:

- the feature name says scaffold/experimental;
- ordinary `leaven` defaults do not enable it;
- topology/public-maturity tests allowlist it.

This category is for future-work crate shells that still need compile-time
placement, not for product examples.

### Ordinary Public Contract

Allowed when:

- it has behavior-bearing types or trait laws;
- examples exercise the real path;
- errors are typed;
- tests prove the public contract;
- docs do not call it a skeleton;
- default facades can expose it without misleading users.

## Default-Facing Gate

The default `leaven` import experience must expose no:

- compile-error derives;
- inert standard names;
- placeholder provider/backend types;
- production-looking fixtures;
- engine-author names in ordinary prelude;
- public modules that expose file layout as semver surface without intent.

## Topology Gate Additions

The topology contract should fail on:

- orphan `crates/*` directories not listed in the workspace or an explicit
  orphan allowlist;
- workspace crates whose public module docs still say skeleton unless
  allowlisted;
- public empty unit structs in non-scaffold features unless allowlisted;
- default features that export placeholders or compile-error macros;
- crate-root `pub mod` exports that are not in an export ledger;
- stale topology specs that name crates absent from the workspace.

## Inventory Refinement

Future crate inventory should use these categories instead of one broad
"placeholder" bucket:

- real;
- mixed real code with public stubs;
- stale skeleton metadata on behavior-bearing crates;
- true placeholder;
- orphan/stale directory.

This avoids overstating negative status for crates that are wired and tested on
the happy path while still refusing public lies.
