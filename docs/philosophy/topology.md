# Topology: Who Is Allowed to Know What

Because crate/module/workspace design is not “where files go.”

It is the part of the philosophy that decides:

> Which code is allowed to know which facts?

That is the project-scale version of your existing principles. Bad topology creates scatter, implicit knowledge, drift, translation, and noise even if the local types are excellent. A perfect `IssueState` still rots if every crate can reach into every adapter, every DTO, every helper, every config global, and every public module path.

The forcing function I’d use:

> **Every boundary must make at least one wrong dependency impossible.**
> If a crate/module split does not prevent a bad dependency, hide a real implementation detail, reduce compile/rebuild scope, isolate a feature/dependency set, or clarify a public surface, it is probably ceremony.

## The core distinction

A **module** is a locality/privacy/naming boundary.

A **crate** is a compile/dependency/feature/API/privacy boundary.

A **workspace** is the coordination boundary for crates developed together.

Cargo workspaces are explicitly for managing multiple related packages together: common commands can run across members, members share one lockfile and target directory, and the root manifest can centralize things like workspace metadata, dependencies, profiles, and lints. ([Rust Documentation][1])

So the prescription should be brutally simple:

> Use a module when the code should live together but does not need a separate dependency graph.
> Use a crate when the code must not accidentally know things, compile things, or expose things.

## `lib.rs` rule

`lib.rs` is not where code lives.

`lib.rs` is the public map.

It should mostly contain:

```rust
//! Crate-level docs.

#![forbid(unsafe_code)]

mod issue;
mod user;
mod storage;

pub use issue::{Issue, IssueId, IssueState};
pub use user::{CreateUser, CreateUserError, User, UserId};
pub use storage::{Storage, StorageError};

pub mod prelude {
    pub use crate::{
        Issue, IssueId, IssueState,
        User, UserId,
        Storage,
    };
}
```

No real implementation. No business logic. No random helpers. No “temporary” mega-function. No adapter code.

The mental model:

> `lib.rs` is an import experience, not an implementation location.

Rust already gives you the machinery for this. Items are private by default, and public access depends on the item and its ancestor modules being accessible; private modules plus `pub use` let you expose a curated API while hiding the actual file/module structure. ([Rust Documentation][2]) Rustdoc’s re-export model also supports this pattern directly: public items can live in private modules and be re-exported at the public surface. ([Rust Documentation][3])

## Visibility ladder

Make this mechanical:

```rust
private      // default
pub(super)  // parent may assemble this
pub(crate)  // internal crate surface
pub(in ...) // scoped internal surface
pub         // external promise
```

The rule:

> Start private. Widen only when the compiler proves the next boundary needs it.

And the sharper version:

> Never make something `pub` so tests can touch it.

If a test needs private internals, one of three things is true:

1. The behavior is public and should be tested through the public/capability boundary.
2. The test belongs inside the module with `#[cfg(test)]`.
3. You need an intentional `test-support`/contract-test surface.

Do not let tests punch random public holes through the design.

## Default crate graph

For a serious project, I’d bias toward this shape:

```text
workspace/
  Cargo.toml

  crates/
    product-core/
    product-contracts/
    product-adapter-postgres/
    product-adapter-redis/
    product-adapter-http-client/
    product-app/
    product-server/
    product-cli/
    xtask/
```

With dependency arrows like this:

```text
product-core
    ↑
    ├── product-contracts
    ├── product-adapter-postgres
    ├── product-adapter-redis
    ├── product-adapter-http-client
    ↑
product-app
    ↑
    ├── product-server
    └── product-cli
```

Or, more precisely:

```text
server / cli / jobs
        depend on
app orchestration
        depends on
core traits + domain types + canonical errors
        implemented by
adapter crates
```

`core` should be cold and boring. It owns:

```text
domain types
cold traits
canonical capability errors
laws/docs
small pure domain logic
```

It should generally not depend on:

```text
axum
tonic
sqlx
diesel
redis
reqwest
tokio, unless the core capability is inherently async
serde_json
tracing implementation policy
environment variables
filesystem layout
cloud SDKs
```

Those belong in adapters, binaries, or boundary crates.

This matches your trait essay: once a trait is cold, it becomes a promise to strangers, so its home should be somewhere stable and dep-light, not buried inside an implementation crate. 

## The contracts crate is important

Your test essay says cold traits need shared contract suites; that’s exactly where topology matters. 

Do not duplicate trait-law tests inside every backend.

Do this:

```text
product-core
  defines Storage, StorageError, Key, Value

product-contracts
  depends on product-core
  exposes storage_laws(...)

product-adapter-postgres
  depends on product-core
  dev-depends on product-contracts

product-adapter-memory
  depends on product-core
  dev-depends on product-contracts
```

Then every implementation opts into the same law suite:

```rust
#[test]
fn postgres_storage_satisfies_storage_contract() {
    product_contracts::storage_laws(|| make_postgres_storage());
}
```

This keeps the trait’s promises in one place and keeps implementors honest without making `core` depend on concrete backends.

## Module shape inside a crate

Inside a crate, organize by concept first, technical role second.

Good:

```text
src/
  lib.rs

  issue/
    mod.rs
    id.rs
    state.rs
    command.rs
    error.rs
    repo.rs
    service.rs

  user/
    mod.rs
    id.rs
    name.rs
    email.rs
    command.rs
    error.rs
```

Suspicious:

```text
src/
  models.rs
  errors.rs
  traits.rs
  services.rs
  utils.rs
  handlers.rs
```

The first shape lets you ask “where is issue lifecycle?” and go to `issue`. The second forces you to assemble the truth from a bunch of technical buckets, which is exactly the scatter failure mode from the disappearing-code essay. 

Inside each concept module, `mod.rs` should curate the local surface:

```rust
mod command;
mod error;
mod id;
mod state;
mod validate;

pub use command::{CreateIssue, CloseIssue};
pub use error::{CreateIssueError, CloseIssueError};
pub use id::IssueId;
pub use state::{Issue, IssueState};

pub(crate) use validate::validate_title;
```

The public path should be conceptual:

```rust
use product_core::{Issue, IssueId, IssueState};
```

Not archaeological:

```rust
use product_core::issue::state::machine::v2::IssueState;
```

## Crate split decision table

Use a **module** when:

| Situation                                     | Default                      |
| --------------------------------------------- | ---------------------------- |
| Same dependency set                           | module                       |
| Same change cadence                           | module                       |
| Same public stability                         | module                       |
| Need locality/privacy only                    | module                       |
| Split is only because file got long           | module/file split, not crate |
| Code is private implementation of one concept | private submodule            |

Use a **crate** when:

| Situation                                                | Default                           |
| -------------------------------------------------------- | --------------------------------- |
| Heavy dependency should not infect core builds           | crate                             |
| Adapter depends on DB/HTTP/cloud/runtime SDK             | crate                             |
| Code has different feature matrix                        | crate                             |
| Code has different stability temperature                 | crate                             |
| Code should not be able to access internals              | crate                             |
| Multiple implementations share one contract              | core + contracts + adapter crates |
| Boundary is semver/API significant                       | crate                             |
| Unsafe/FFI/build-script/proc-macro code needs quarantine | crate                             |
| Compile/rebuild isolation matters                        | crate                             |
| Thing may become reusable outside this binary            | maybe crate                       |

The key anti-rule:

> Never create a crate merely because “this file is big.”

Big files are a locality problem. Crates are permission/dependency/compilation boundaries.

## Compile-time prescription

The compile-time version of the philosophy is:

> Hot code should depend on cold code; cold code should not depend on hot code.

Keep frequently edited orchestration/boundary code above stable core crates. Keep heavy dependency crates at the leaves. Keep `sqlx`, `diesel`, `tonic`, `prost`, `bindgen`, browser/web framework stacks, cloud SDKs, and proc macros away from domain/core crates unless they are genuinely part of that crate’s central meaning.

Cargo features can express optional dependencies and conditional compilation, but they should not become a hidden mode system. Cargo documents that features enable optional dependencies and `cfg(feature = "...")` code, and also warns that default features are automatically enabled by dependencies unless explicitly disabled. ([Rust Documentation][4]) It also says enabling a feature should not introduce a SemVer-incompatible change, which is a good local version of the broader rule: features should add capabilities, not silently change contracts. ([Rust Documentation][4])

So:

Good feature:

```toml
[features]
postgres = ["dep:sqlx"]
redis = ["dep:redis"]
```

Bad feature:

```toml
[features]
production = []
mock = []
new-semantics = []
```

If a feature changes what a trait means, it is not a feature. It is a different implementation, crate, or capability.

## Public API prescription

Every `pub` item is a promise.

Every public type must answer:

1. Can downstream users construct this?
2. Can they exhaustively match it?
3. Can they implement this trait?
4. Can we add fields/methods/variants later?
5. Are we leaking a dependency type?
6. Is this path stable, or just today’s file layout?

The Rust API Guidelines call out exactly this future-proofing pressure: sealed traits can protect against downstream implementations, structs with private fields give you room to evolve, and public dependencies of stable crates need to be stable too. ([Rust Documentation][5]) Rust’s `#[non_exhaustive]` exists for the same family of problems: it marks structs/enums/variants as open to future fields or variants. ([Rust Documentation][6])

My prescriptive version:

### Public structs

Default:

```rust
pub struct User {
    id: UserId,
    email: Email,
}
```

Expose constructors and accessors intentionally.

Avoid:

```rust
pub struct User {
    pub id: UserId,
    pub email: String,
}
```

Unless you truly mean: “callers may construct any combination of these fields forever.”

### Public enums

If callers need exhaustive domain reasoning, let them match exhaustively:

```rust
pub enum IssueState {
    Open,
    Closed,
    Archived,
}
```

If you need future variants and callers should not assume the universe is closed:

```rust
#[non_exhaustive]
pub enum ProviderEvent {
    MessageCreated { id: MessageId },
    MessageDeleted { id: MessageId },
}
```

But don’t slap `#[non_exhaustive]` everywhere. Your own essays value closed universes because the compiler can propagate change. If a new variant should force every caller to think, exhaustive is better.

### Public traits

Decide whether downstream implementors are part of the promise.

If yes: laws + contract tests.

If no: seal it.

```rust
mod sealed {
    pub trait Sealed {}
}

pub trait Storage: sealed::Sealed {
    fn get(&self, key: &Key) -> Result<Option<Value>, StorageError>;
}
```

A public trait without a decision about external implementation is a loaded gun.

## The “no common crate” rule

Ban these until proven innocent:

```text
common
shared
utils
helpers
models
types
misc
prelude-but-actually-everything
```

They are topology’s version of `Other(String)` in error design.

Sometimes you really do need shared foundations. But then name the actual concept:

```text
product-time
product-ids
product-money
product-protocol
product-contracts
```

Not:

```text
product-common
```

A `common` crate usually means: “we did not decide who owns this idea.”

## The dependency leak rule

If a public signature mentions a dependency’s type, that dependency is part of your public API.

This is a leak:

```rust
pub fn create_user(row: sqlx::postgres::PgRow) -> Result<User, sqlx::Error>;
```

This is an intentional boundary:

```rust
pub trait UserRepo {
    fn create(&self, user: NewUser) -> Result<UserId, UserRepoError>;
}
```

And the adapter absorbs the dependency:

```rust
pub struct PostgresUserRepo {
    pool: sqlx::PgPool,
}
```

This is the topology version of your error essay’s “don’t leak dependencies” rule. Public APIs should speak in domain/capability terms; implementation crates and `#[source]` fields can know about concrete libraries. 

## The anti-`lib.rs` checklist

When reviewing a PR, look for:

1. `lib.rs` contains real logic.
2. `pub mod foo;` for every file.
3. `pub(crate)` used because tests or sibling modules are nosy.
4. `common`, `utils`, `models`, or `helpers`.
5. Core/domain crate depends on DB, HTTP, web framework, cloud SDK, or concrete runtime.
6. Adapter type appears in core.
7. DTO/wire type appears in domain logic.
8. Public API path mirrors file layout.
9. Feature flag changes behavior instead of adding capability.
10. Trait lives next to first implementation instead of next to its laws.
11. Tests widened visibility instead of moving to the right boundary.
12. One crate has mutually unrelated heavy optional dependency families.
13. Binaries contain domain decisions instead of rendering/wiring.
14. Every module imports from every other module.

The brutal review question:

> What wrong thing does this boundary prevent?

No answer means probably noise.

# The other missing essays

I’d put these in the same “extremely smart people still get this wrong” class.

## 1. Effects, runtime, and cancellation

Rust makes ownership explicit, but async code often smuggles reality back into invisible places.

Prescribe:

> No hidden spawning. No immortal tasks. No ambient runtime assumptions. Cancellation, timeout, retry, and backpressure are part of the capability contract.

Smells:

```rust
tokio::spawn(async move {
    do_important_work().await;
});
```

with no handle, cancellation path, error path, span, or shutdown story.

You want rules for:

```text
Who owns this task?
Who can cancel it?
What happens on shutdown?
Where do errors go?
Can this be retried?
Is this operation idempotent?
What backpressure exists?
```

This sits right next to the error essay because “unknown side effect” and “cancelled halfway through” are real world states.

## 2. Serialization, wire types, and persistence types

This one is huge.

Prescribe:

> Domain types are not automatically wire types. Wire formats are not automatically storage schemas. Storage schemas are not automatically domain models.

Bad default:

```rust
#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
}
```

This looks efficient but often collapses three different truths into one struct:

```text
domain truth
HTTP/API compatibility
database representation
```

Better default:

```text
User              // domain
UserRow           // database shape
UserResponse      // outbound API shape
CreateUserRequest // inbound wire shape
```

Then conversions become conscious information reshaping, which matches your type essay’s “preserve information until explicit destruction” principle. 

## 3. Configuration, environment, clocks, and randomness

Smart people still casually call:

```rust
std::env::var("FOO")
Utc::now()
Uuid::new_v4()
rand::thread_rng()
```

deep inside core logic.

Prescribe:

> The world enters at the edge.

Config should be parsed once into typed config. Time should be a capability where decisions depend on time. Randomness should be passed or owned. Environment variables should not be read from domain logic.

Bad:

```rust
fn issue_is_expired(issue: &Issue) -> bool {
    Utc::now() > issue.deadline
}
```

Better:

```rust
fn issue_is_expired(issue: &Issue, now: Instant) -> bool {
    now > issue.deadline
}
```

Or, where appropriate:

```rust
pub trait Clock {
    fn now(&self) -> Instant;
}
```

But only if `Clock` is a real seam, not testability theater.

## 4. Observability

Your error essay already says “log once, at the boundary where you decide.”  That probably deserves its own essay.

Prescribe:

> Logs are not error handling. Logs are rendered evidence.

Rules:

```text
Inner layers return structured errors.
Boundaries decide and log once.
Spans follow operations, not files.
Metrics count decisions/classes, not string messages.
Never make callers parse logs to recover structure.
```

Smell:

```rust
error!("failed: {}", err);
return Err(err);
```

in six layers.

## 5. API evolution / cold-surface design

You already have hot/cold traits and hot/cold tests. Apply the same idea to public API.

Prescribe:

> A cold public surface should be smaller, more boring, and harder to implement than you want.

Rules:

```text
Private fields by default.
Constructors over public struct literals.
Seal traits unless external impls are intended.
Use builders for large option sets.
Use exhaustive enums when callers should revisit logic on new variants.
Use #[non_exhaustive] only when wildcard handling is actually acceptable.
Never expose implementation modules as public paths.
```

This is where extensibility gets precise. “Extensible” does not mean “everything public.” It means the right things can change without breaking the wrong promises.

## 6. Dependency and feature hygiene

This could be part of topology, but it may deserve its own doc.

Prescribe:

> Dependencies are architectural facts.

Every dependency should have an owner and a reason. Every feature should describe an added capability. Public dependencies are part of API design. Optional dependencies should not infect the default build accidentally.

Smells:

```text
default = ["everything"]
feature = "prod"
feature = "mock"
feature = "new-parser"
core depends on adapter
adapter feature changes trait semantics
```

## 7. Unsafe / FFI / escape hatches

Even if you rarely use unsafe, prescribe it before you need it.

Rule:

> Unsafe code lives behind one safe story.

Require:

```text
small unsafe module/crate
safe public wrapper
documented invariants
no casual unsafe in business logic
tests around boundary
Miri/sanitizer path if relevant
```

Unsafe should be topologically quarantined.

## 8. Performance and cost semantics

Not “optimize everything.”

More like:

> APIs should not hide costs that change caller decisions.

Prescribe where allocation, cloning, blocking, I/O, locking, and retries are allowed to be invisible and where they must be explicit.

Smells:

```rust
fn name(&self) -> String
```

when it should be:

```rust
fn name(&self) -> &Name
```

Or:

```rust
fn users(&self) -> Vec<User>
```

when the collection could be huge, streamed, paged, borrowed, or filtered.

This connects to your “decision surface” language: if a cost would change caller behavior, the API should reveal it.

# The one-sentence doctrine

I’d make the topology essay’s thesis this:

> **Project structure is a permission system: crates decide who may depend, modules decide who may know, and public re-exports decide what story strangers are allowed to believe.**

Then the practical default becomes:

```text
core is cold
adapters are leaf-heavy
apps wire capabilities
binaries render the world
contracts live next to promises
lib.rs is a map
modules own concepts
pub is a promise
features add capability
common is a smell
```

That is the thing that keeps people from doing the obvious bad thing: dumping reality into `lib.rs` until the codebase has no walls.

