# Leaven

Leaven is a Rust library for optimizing agents.

Leaven can improve things you can change and measure: skills, codebases, harnesses, environments.

Doesn't matter what it is or where it lives. If it affects the agent, throw it in leaven.

Leaven is built for agents. Concretely:

- written in Rust
- reusable primitives (yes, you can vibecode your optimizers)
- skills (soon to come)
- agents can inspect/work with all of it

You need, at minimum, to have some form of reward.

This means a way to tell if something is good (numbers), valid (binary), or if it's better than something else (pairwise).

You should also be able to provide rich feedback on failures, though for agentic optimization, this is less necessary.

## How It Optimizes Anything

The loop is small. Five moves per iteration:

1. **select** - pick something to change
2. **propose** - generate a variant
3. **evaluate** - score it
4. **keep** - retain what worked
5. **prune** - discard what did not

Leaven runs the cycle. You bring the artifact, the way to change it, and the way to score it. The engine does not decide what "better" means. You do.

Different optimizers come from different choices inside the loop. GEPA is one. TextGrad is another. Your own optimizer is another. They all fit in the same shape.

## Install

Leaven is not published on crates.io yet. Add it from the latest git remote:

```bash
cargo add leaven --git https://github.com/delightful-ai/leaven.git
```

Or in `Cargo.toml`:

```toml
leaven = { git = "https://github.com/delightful-ai/leaven.git" }
```

## Quick Look

The high-level builder path is:

```rust
use leaven::gepa::Gepa;
use leaven::prelude::{optimize, Budget};

let result = optimize(seed_prompt)
    .train(train_cases)
    .validation(validation_cases)
    .test(test_cases)
    .runner(|prompt, case| async move {
        run_solver(prompt, case).await
    })
    .score(score_answer)
    .using(
        Gepa::reflect_with_lm(reflection_lm, reflection_model)
            .surface(PromptSurface)
            .build(),
    )
    .budget(Budget::metric_calls(512))
    .run()
    .await?;

if let Some(best) = result.best() {
    println!("better artifact:\n{best:?}");
}
```

For a real checked example of this shape, see the
[AIME GEPA example](examples/p8_aime_gepa/README.md) and its
[builder call](examples/p8_aime_gepa/src/main.rs). It exercises:

- train / validation / held-out test splits
- async runner and async scorer surfaces
- GEPA reflection through provider-neutral `leaven-lm`
- deterministic no-spend execution by default
- opt-in live OpenAI solver and reflection roles

Run the deterministic AIME path with:

```bash
cargo run -p p8_aime_gepa
```

## What You Can Optimize

Anything that has:

- **identity** - a stable way to refer to it
- **change** - a typed way to modify it
- **evaluation** - a way to score one version against another

Prompts, configs, code modules, agent harnesses, traces, directories of files, whole repos. The sweet spot is agents themselves: their prompts, their harness, and the scaffolding they run on.

## Design

The engine is dumb. The strategies are smart.

Leaven gives you the loop machinery and lets you swap the pieces that actually differ between optimizers: what to select, how to propose, how to evaluate, and what to keep. GEPA, TextGrad, MIPRO, MAP-Elites, and whatever optimizer your paper has not written yet all fit in the same shape.

Agents are first-class. The API surface is designed for coding agents to read and integrate against. When you say "use leaven to optimize X," they can read the spec, write the proposer and evaluator, and run the loop.

Lineage runs through Stanford NLP: Leaven extends ideas from [GEPA](https://github.com/gepa-ai/gepa) and [DSPy](https://github.com/stanfordnlp/dspy) into Rust, and reframes them around what coding agents can now do autonomously.

## Status

Leaven is early alpha. The Rust workspace is where the current implementation lives, and the public API is still moving quickly. The high-level API shown above is the direction of travel and is backed by checked examples in this repo, but it is not a formal stability promise yet.

Python support is on the roadmap and has design/spec work in progress, but it is not shipped as a supported package today.

## Links

- [Spec](docs/specs/initial_library.md) - the load-bearing product spec
- [Guiding principles](docs/specs/guiding_principles.md) - product constraints and design principles
- [GEPA optimizer surface](docs/specs/gepa_optimizer_surface.md) - GEPA-specific API and behavior notes
- [Examples](examples) - executable milestone packages
- [AIME GEPA example](examples/p8_aime_gepa/README.md) - the current public builder example
- [Testing contract](docs/testing/README.md) - verification commands and proof model

## License

MIT or Apache-2.0, at your option.
