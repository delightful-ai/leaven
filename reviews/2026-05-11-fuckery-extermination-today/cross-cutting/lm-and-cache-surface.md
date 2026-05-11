# LM And Cache Surface Audit Seed

This file tracks LM/runtime/cache-specific smells already found.

## Desired Shape

Provider-specific crates should sit at the edge. Optimizers should consume
provider-neutral LM capabilities. Ordinary users should not have to stack cache
wrappers manually to get sane repeated-run behavior.

Reflective mutation should accept an LM or agent capability at the reflection
slot and should see the selected evidence/trace subset needed to propose a
change.

## Already Found Problems

### Cache As Wrapper Is Too Public

`CachedLm<M, C>` is useful as an implementation shape, but it is a bad ordinary
API story. It makes caching a wrapper users manually compose instead of a run
or LM-runtime policy.

Power users still need:

- cache policy;
- cache store trait;
- in-memory/disk/other backends;
- deterministic key semantics.

But Layer 1 examples should show something more like:

```rust
let lm = LmRuntime::openai("gpt-4.1-mini")
    .cache(CachePolicy::ReadWrite)
    .build_from_env()?;
```

The exact name can change. The point is that ordinary API should describe the
capability, not the wrapper.

Finding status: high. The implementation currently exposes
`leaven-lm-cache::CachedLm` as the obvious composition shape
(`crates/leaven-lm-cache/src/lib.rs:9`,
`crates/leaven-lm-cache/src/cached.rs:6-16`). That is useful internally and for
advanced users, but it should not be the primary Layer 1 story. The product
builder also never wires response cache policy into solver or reflector roles,
and `ScoringEvaluator::cache_policy()` returns `CachePolicy::Never`
(`crates/leaven-run/src/evaluator.rs:61`).

### OpenAI Provider Model Argument Is Misleading

`OpenAiLm::from_env("gpt-4.1-mini")` accepts a model-looking argument but does
not store or use it. Either the provider owns a default model or the request
owns the model. The current halfway shape is a public lie.

Finding status: high. The provider constructor accepts `_default_model` and
ignores it (`crates/leaven-lm-openai/src/client.rs:27-36`). Requests can set a
model (`crates/leaven-lm/src/request.rs:18-33`), so the provider constructor
must either stop taking a model or store a real default that is used when the
request has none.

### LM Reflection And LM Solving Are Separate Runtime Roles

AIME needs two LM uses:

- solver LM: run the candidate prompt on each case;
- reflector LM or agent: inspect failures/traces and propose candidate edits.

Those should be independently configurable and independently cached/costed.
The current example proves neither role through Leaven: solver live path shells
out, and reflector is a fixed edit.

Finding status: blocker. The live solver path shells out to
`examples/p8_aime_gepa/scripts/openai_solver.py` from
`examples/p8_aime_gepa/src/main.rs:293-301`, and the reflector path uses a
fixed `ReflectiveMutation` fixture. Neither role proves the provider-neutral
`Lm` trait, OpenAI adapter, response cache, or token/cost accounting.

### Cache Key And Multi-Turn Semantics Are Not Proven End-To-End

Finding status: medium. `LmRequest` includes messages, continuation, provider
options, and metadata (`crates/leaven-lm/src/request.rs:18-33`), while
`LmContinuation` exists for multi-turn flows (`crates/leaven-lm/src/request.rs:7-16`).
The audit has not found an end-to-end example that proves cache identity,
continuation handling, and provider lowering together.

Correction direction: add a small LM contract/example that runs one cached
single-turn request and one cached continuation request through `leaven-lm`,
`leaven-lm-cache`, and a provider/mock adapter. The high-level optimizer
example should consume the same runtime role rather than a bespoke wrapper.

## Broader LM Audit Questions

- Is `Lm` the right trait for both solver calls and reflector calls?
- Should there be an `LmRuntime`/`LmClient` composition root separate from the
  provider trait?
- Where should response-cache policy be configured for examples, runs, and
  optimizer internals?
- How do LM calls report cost into Leaven budget without confusing metric-call
  budget and provider-token cost?
- Does multi-turn continuation interact correctly with cache identity and
  examples?
