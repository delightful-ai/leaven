---
name: leaven-docstrings
description: Use when writing or reviewing Leaven Rust docstrings, module docs, examples, intra-doc links, Errors, Panics, Safety, feature-gated docs, or dspy-rs signature field descriptions.
---

# writing great rust docstrings: a philosophy and style guide

this document is a theory of what makes rust docstrings good, derived from first principles about what documentation is FOR in a language where the type system already carries most of the contract. it's opinionated. it's meant to be used as a reference when writing or reviewing docstrings in the field.

## the fundamental claim

a rust docstring's job is to carry the parts of the contract that the type system can't reach yet. the type signature tells you WHAT the function accepts and returns. the docstring tells you WHY it exists, WHEN to use it, what INVARIANTS it maintains, and what SEMANTICS the types encode.

if you can delete a docstring and lose no information, the docstring was redundant. if you can delete the function body and reconstruct it from the docstring + type signature alone, the docstring is complete. aim for the latter.

the docstring is a type signature for the parts the type system can't express. `# Errors` and `# Panics` sections are the manually-maintained shadow of a richer effect system rust doesn't have. treat them with that level of seriousness.

## the first line

the first line is a sentence fragment that completes "this function/struct/etc...". not a full sentence starting with "This function does X."

```rust
/// Deserializes a frame from the wire format.
```

not:

```rust
/// This function deserializes a frame from the wire format.
```

rustdoc uses the first line as a summary in module-level listings. it must standalone, convey the essential purpose, and fit on one line. think of it as the type-level equivalent of a commit message subject line.

## what docstrings should carry

### the mental model, not the mechanics

for types especially, the signature already tells you WHAT. the docstring should tell you:

- what invariants the type maintains
- when you'd reach for it vs alternatives
- what "conceptual space" it occupies in the system
- what it means, not what it contains

```rust
/// A validated, non-empty sequence of pipeline stages with guaranteed type-safe
/// data flow between adjacent stages.
///
/// Use `Pipeline` when you have multiple signatures that should execute in
/// sequence with each stage's output feeding the next stage's input. For
/// independent parallel execution, use `Ensemble` instead.
///
/// The pipeline validates all inter-stage type connections at construction time,
/// so `.forward()` cannot fail due to type mismatches — only due to LM errors
/// or network failures.
pub struct Pipeline<S: Stages> { /* ... */ }
```

the struct fields already tell you the shape. the docstring tells you the contract, the design space, and the failure model.

### the WHY, not just the WHAT

when a design is surprising, non-obvious, or exists for historical/compatibility reasons, say so. a docstring that says "this exists because X" tells you more than ten lines of API description, because it tells you the code is *inhabited* — someone considered alternatives and chose this.

```rust
/// The `Handler` trait uses a generic parameter `T` rather than associated types
/// as a workaround for current limitations in Rust's type inference. This makes
/// the implementation more flexible at the cost of occasionally requiring
/// turbofish syntax at call sites.
```

this pattern — acknowledging the constraint, explaining the tradeoff — is worth more than any amount of parameter documentation. it tells the reader that the weirdness is intentional and bounded.

### the negative space

document what the thing DOESN'T do. what it deliberately chose not to handle. what's out of scope.

```rust
/// Evaluates a module against a dataset and returns aggregate metrics.
///
/// This does NOT perform optimization — it only measures. Use evaluation results
/// to decide whether optimization is needed, then pass them to an optimizer
/// separately.
///
/// Note: evaluation is not parallelized by default. For large datasets,
/// wrap calls in your own concurrency primitive or use `evaluate_parallel`.
```

the negative space is often more valuable than the positive. you can read the code to see what it does. you cannot easily read the code to see what it deliberately chose not to do.

### the "almost apologetic" precondition

when a function has a subtle precondition that the types don't capture, the docstring should sound almost apologetic about it. this honesty signals that the author KNOWS where the abstraction leaks.

```rust
/// Compiles an optimized version of the module using the provided dataset.
///
/// The dataset must contain at least 5 examples for meaningful optimization.
/// Ideally this would be enforced in the type system (e.g., `NonEmpty<Vec<Example>>`),
/// but the minimum count depends on the optimizer strategy, so it's checked
/// at runtime. Fewer than 5 examples will return `Err(TooFewExamples)`.
```

"ideally this would be in the type system" is a signal of design awareness. it tells the reader the author considered the alternative and chose this tradeoff deliberately.

## mandatory sections

### `# Panics`

if it can panic, document WHEN. this is not optional. it's a contract.

```rust
/// # Panics
///
/// Panics if `batch_size` is zero. Use `NonZeroUsize` if you want compile-time
/// enforcement; this function accepts `usize` for ergonomics at call sites
/// where zero is already ruled out by context.
```

every panic condition should be listed. vague "panics in some cases" is a documentation bug.

### `# Errors`

for `Result`-returning functions, every error variant should be documented. not "returns an error if something goes wrong" — enumerate what, when, and ideally what the caller should do about it.

```rust
/// # Errors
///
/// - [`PredictError::LmFailure`] — the language model returned a non-200 status
///   or the connection timed out. Retryable.
/// - [`PredictError::ParseFailure`] — the model's response could not be parsed
///   into the output signature type. This usually means the model ignored
///   formatting constraints. Consider adding a `Retry` wrapper or adjusting
///   field descriptions.
/// - [`PredictError::ValidationFailure`] — parsing succeeded but the output
///   violates a field constraint (e.g., an enum field returned an invalid variant).
```

note the pattern: variant name, when it occurs, and what to do about it. that last part — the remediation hint — transforms error docs from reference into guidance.

### `# Safety` (for `unsafe`)

must enumerate every precondition the caller is responsible for upholding. "the pointer must be valid" is insufficient. valid HOW? for how long? under what aliasing constraints? what alignment? what happens if violated — UB, abort, or just wrong results?

## examples

### they must compile

` ```rust ` blocks run as doctests by default. if your example doesn't compile, you've written a lie. use `no_run` or `ignore` intentionally, not because you're lazy.

```rust
/// # Examples
///
/// ```rust
/// use dspy_rs::prelude::*;
///
/// #[derive(Signature)]
/// #[instruction("classify the topic of a news headline")]
/// struct ClassifyTopic {
///     #[input(desc = "news headline text")]
///     headline: String,
///     #[output(desc = "one of: politics, technology, sports, entertainment, other")]
///     topic: Topic,
/// }
///
/// # tokio_test::block_on(async {
/// let classifier = Predict::new(&lm);
/// let result = classifier.forward(ClassifyTopicInput {
///     headline: "Congress passes new AI regulation bill".into(),
/// }).await?;
///
/// assert!(matches!(result.topic, Topic::Politics));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # });
/// ```
```

hidden setup lines (`# use ...`, `# tokio_test::block_on(...)`) keep the example focused on what matters. the reader sees the signal; the test runner sees the full program.

### show the thing that's not obvious

don't write examples that restate the signature. write examples that show the non-obvious: composition, error handling, edge cases, the "oh THAT'S how you use it" moment.

```rust
/// # Examples
///
/// Signatures compose naturally — the output of one becomes the input of the next:
///
/// ```rust
/// let claims = analyze.forward(AnalyzeInput { document }).await?;
/// let cross_ref = compare.forward(CrossRefInput {
///     claims: claims.claims,  // Vec<Claim> flows directly
///     corpus: reference_docs,
/// }).await?;
/// ```
```

the example isn't showing that `forward` exists (the signature already says that). it's showing that typed composition works and what it feels like.

## module-level docs: the mini essay

the docstring on a module (`//!` at the top of a file) should read like a two-paragraph essay: what problem this module exists to solve, what strategy it uses, and what the key types are and how they relate.

if someone has to open 15 individual type docs to reconstruct the architecture, you've failed. the module doc IS the architecture doc.

```rust
//! Optimization strategies for improving module performance against a metric.
//!
//! An optimizer takes a [`Module`], an [`Evaluator`], and a training dataset, and
//! produces a new version of the module with tuned prompts and/or few-shot examples.
//! The module's signatures (its type structure) are never mutated — only the
//! instructions, descriptions, and demonstrations that the framework generates
//! from those signatures.
//!
//! Start with [`Copro`] for instruction-only optimization, or [`MiproV2`] for
//! joint instruction + demonstration optimization. Both require a metric function
//! and at least 5 training examples.
//!
//! See the [optimization tutorial](https://docs.example.com/tutorial/optimization)
//! for a guided walkthrough.
```

this module doc tells you: what optimizers do, what they change and what they don't, which one to start with, and where to learn more. someone reading this can orient themselves before touching any individual type.

## linking

use `[`SomeType`]` intra-doc links everywhere. rustdoc resolves them automatically. there is no excuse for bare type names in prose when you could crosslink. every type mentioned in a docstring should be a link.

```rust
/// Returns the [`Trace`] for the most recent [`Module::forward`] call,
/// or [`None`] if no call has been made yet.
```

this is free. it costs nothing to write and saves significant navigation time when reading rendered docs.

## feature gates and cfg

if something is only available behind a feature flag, use `#[doc(cfg(feature = "foo"))]` so it renders with a visible badge in docs.rs. feature discoverability is a known pain point in the rust ecosystem — the docs are the primary place people discover what features exist.

## voice

have one. not jokes, not whimsy, but a human presence. when someone writes `/// This is O(n) and we're not happy about it` that tells you more than any formal complexity annotation because it tells you there's a human on the other end who CONSIDERED the alternative and decided this was the tradeoff. it tells you the code is inhabited.

the voice should be:

- precise without being pedantic
- honest about limitations without being apologetic
- concise without being cryptic
- aware of the reader without being patronizing

the test: would you want to read these docs at 2am when something is broken? if the voice is annoying or condescending or obtuse at 2am, fix it.

## the reader model

a good docstring has a theory of its reader. who are you writing for? what do they already know? what will confuse them?

for a public library, the reader model is roughly: "a rust developer who understands the language and the domain but has never seen this crate before." they know traits, generics, lifetimes. they do NOT know your internal abstractions, your naming conventions, or your design philosophy. meet them where they are.

docstrings without a reader model are noise that happens to be adjacent to code.

## the ultimate test

could someone reimplement your function from ONLY the docstring + type signature, without seeing the body, and produce something behaviorally equivalent?

if not, you're missing something. find what's missing and add it.

this test is not aspirational. it's the actual bar. a docstring that passes this test is a docstring that carries the full contract. everything else is decoration.

## field descriptions in dspy-rs signatures (domain-specific)

field descriptions in dspy-rs signatures are a special case of docstrings with an unusual property: they're read by both humans AND models. the `desc` attribute on a signature field is simultaneously documentation and steering.

this means field descriptions must be:

- precise enough that a language model produces correctly scoped output
- semantic enough that a human reading the signature understands the intent
- constrained enough that evaluation can be meaningfully applied

```rust
#[output(desc = "each distinct technical claim made in the document, \
                  stated precisely enough to be independently verified")]
claims: Vec<Claim>,
```

"each distinct technical claim" tells the model the granularity (one per claim, not one blob). "stated precisely enough to be independently verified" tells the model the quality bar. "made in the document" scopes it (don't invent claims). a human reading this knows exactly what `claims` means.

contrast with:

```rust
#[output(desc = "the claims")]
claims: Vec<Claim>,
```

this produces categorically different (worse) output. the desc isn't labeling — it's specifying.

the same principle applies to all docstrings, even outside dspy-rs: descriptions that specify semantics and constraints produce better understanding than descriptions that merely name things. but in dspy-rs, the consequence is immediate and measurable, because the model literally reads your descriptions and acts on them.

## summary of principles

1. carry what the types can't — invariants, semantics, intent, tradeoffs
2. the first line is a standalone fragment that works in listings
3. document the negative space — what it doesn't do, won't handle, deliberately excludes
4. `# Panics`, `# Errors`, `# Safety` are mandatory contracts, not optional decoration
5. examples must compile, must show the non-obvious, must be tested
6. module docs are architecture docs — the essay that makes 15 type pages cohere
7. link everything with intra-doc links
8. have a voice — precise, honest, human
9. know your reader — write for the person who's never seen this crate
10. the test: can someone reimplement from docstring + signature alone?
