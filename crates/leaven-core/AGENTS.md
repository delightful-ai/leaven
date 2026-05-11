## Boundary
This crate owns the cold optimizer algebra: artifact and problem traits,
proposal/evaluation/evidence/preference vocabulary, and the type-level shape of
what a run records.

It describes states and facts. It does not run stages, mutate graphs, project
artifact parts, persist bytes, materialize workspaces, call providers, or hold
optimizer strategy state.

## Route Here
- Artifact contracts: `Artifact`, `ContentAddressed`, `ArtifactIdentity`, and
  `CacheIdentity` belong here when the rule is intrinsic to the optimized value.
- Proposal shape: `Proposal`, `ProposalEffect`, `CausalInputs`, `InfoRef`,
  provenance, batches, and batch semantics belong here.
- Evaluation vocabulary: request/set expressions, resolved request data,
  assessment shapes, partition/tag/window names, granularity, purpose, and pair
  order belong here.
- Cold markers and result enums: `Evidence`, `Preference`, and
  `OptimizationProblem` associated-type wiring belong here.

## Local Helper Stack
- Use the `Proposal::{create, mutate, merge, aggregate}` builders instead of
  constructing provenance by hand. They pre-fill the causal lineage shape that
  engine graph application expects.
- Use `ProposalProvenance::informed_by` / `InfoRef` for bibliographic reads and
  `CausalInputs` only for content lineage. Reflection reading an assessment is
  usually informational; creating a child from candidate content is causal.
- Use `EvaluationRequest` for unresolved user intent and
  `ResolvedEvaluationRequest` only after a run context has frozen the set
  against a `CaseSetVersion`.
- Put semantic proposer payloads in `P::ProposalAnnotations`; keep
  `MetadataBag` for operational breadcrumbs such as worker, prompt blob, or
  timing.

## Route Away
- Candidate storage, lineage indices, graph views, proposal application,
  evaluation resolution, trust, cache, budgets, stage traits, and events belong
  in `leaven-engine`.
- Parts, addresses, selections, views, and edit projections belong in
  `leaven-surface`; decomposition is a surface choice, not an artifact law.
- Reusable evidence, preference relations, populations, renderers, artifacts,
  and optimizers belong in their standard or optimizer crates.
- Store/workspace/LM/agent/provider vocabulary stays outside cold core. If a
  proposed API needs those words, this is the wrong crate.

## Decision Cards
- when: adding a new evaluation request shape or purpose
  do: add the cold vocabulary here, then make engine resolution/cache/trust prove
    the runtime semantics separately
  preserve: unresolved `EvaluationSet` vs resolved case IDs as distinct facts
  avoid: smuggling case lookup, dataset mutation, or trust filtering into core
  verify: run `cargo nextest run -p leaven-core` and
    `cargo test -p leaven --test topology_contract`

- when: adding artifact identity or cache behavior
  do: keep `Artifact::identity` and `Artifact::cache_identity` separate unless
    the artifact is genuinely content-addressed
  preserve: external identity not being automatically cache-safe
  avoid: adding renderer, workspace, or surface facts to artifact identity
  verify: add/update the lowest core contract test plus the downstream cache
    test that consumes the new identity promise

## Proof Anchors
- `src/lib.rs` documents the negative space for cold core and is the public
  export map only.
- `tests/proposal_contract.rs` proves create/change/merge/aggregate effects,
  causal vs informational lineage, batch cloning, and problem trait wiring.
- `crates/leaven/tests/topology_contract.rs` proves `leaven-core` depends only
  on `leaven-kernel` and checks for common projection/engine leaks.
- `cargo nextest run -p leaven-core` proves local algebra contracts.
- `cargo test -p leaven --test topology_contract` proves dependency and
  cold-core leak boundaries when the public surface or manifest changes.

## Local Bait
- `ProposalEffect::Change` is the cold shape of a change; applying it and
  recording success/failure belongs in `RunContext` and `RunGraph`.
- `EvaluationSet` is an expression. Case lookup, partition resolution, and
  unsupported-set errors belong in `leaven-engine::CaseSet`.
- `Preference` is a result value. `PreferenceRelation`, fitted models, archive
  state, and population update policies belong in `leaven-engine`,
  `leaven-population`, or `leaven-preference` as appropriate.
- `EvaluationSet::Unscoped` is the cold single-task expression. Product-builder
  ergonomics and no-dataset lowering belong in `leaven-run`; do not solve that
  Layer 1 gap by adding runner concepts here.
- Do not make fields public to inspect private invariants in tests. Add or
  update a public contract test at this layer, or test private invariants in a
  crate-local `#[cfg(test)]` module.
