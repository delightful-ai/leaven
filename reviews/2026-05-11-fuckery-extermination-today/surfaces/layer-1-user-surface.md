# Layer 1 User Surface Audit Seed

Layer 1 users should be able to run an optimizer without learning engine
actors, graph scopes, evidence stores, cache wrappers, or provider escape
hatches.

Current intended ordinary shape:

```rust
leaven::optimize(seed)
    .train(train)
    .validation(validation)
    .test(test)
    .runner(runner)
    .score(score)
    .using(Gepa::default_or_builder())
    .budget(budget)
    .run()
    .await
```

## Already Found Problems

### AIME Live Path Does Not Prove Leaven LM

The AIME example has the right public builder shell, but the live OpenAI solver
uses a Python script instead of `leaven-lm-openai`. That means an end user can
run a live benchmark without exercising the LM crates that are supposed to make
provider use first-class.

This is a direct contract failure for examples: examples should prove the
library, not route around it.

### The Public Runner Shape Forces Sync Execution

The public builder takes a sync runner and sync score function. That fits
deterministic examples but is a bad first-class shape for LM programs and
agentic tasks. It turns the natural implementation into hidden blocking or
manual runtime management.

This is not just a performance issue. It changes API pressure: a user building
an LM runner reaches for hacks before reaching for the library's intended
async/provider/cache primitives.

### Cache Composition Is Taught As A Wrapper Type

The current LM spec and likely examples teach users to write:

```rust
let lm = CachedLm::read_write(lm, InMemoryLmCache::default());
```

That is too low-level for Layer 1. Cache policy and backend choice are real
power-user concerns, but ordinary users should configure a runtime, not stack a
wrapper type that becomes part of their mental model.

## Layer 1 Audit Questions For The Broader Pass

- Does every example exercise the public surface it claims to prove?
- Does every public example avoid provider/process escape hatches?
- Can an ordinary user run a real LM-backed optimizer without naming cache
  implementation types?
- Can an ordinary user run an async LM/agent scorer without blocking hacks?
- Are train/validation/test semantics reflected in reports without exposing
  internal split policy types?
- Are missing capabilities rejected before run start with useful errors rather
  than silently defaulting to no-op runner/scorer behavior?

