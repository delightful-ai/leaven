# Layer 3 Engine And Optimizer-Author Surface Audit Seed

Layer 3 users are optimizer authors. They should be able to use `RunContext`,
`RunGraphView`, `EvaluationRequest`, evidence, budget, trust scopes, events,
and proposal machinery directly.

## Canonical Docs

- `root-cause-map.md`: Layer 3 engine/eval root causes, with current code and
  spec evidence.
- `fix-priority-map.md`: ordered hard-cutover fixes and proof gates for
  engine/eval/optimizer-author readiness.
- `vision-comparison.md`: original optimizer-author/engine/eval vision versus
  current repository reality.
- `surface-requirements.md`: exact public/private contract for `RunContext`,
  stage contexts, eval lowering, trust, cache, budget, evidence, errors, and
  tests.

## Already Found Problems

### The Real Engine Proposer Surface Exists

The engine has:

- `Proposer<P>` with an associated request type and async `propose(...)`;
- `ProposalContext<'_, P>` with graph, read scope, budget, render context, and
  materialize context;
- `RunContext::propose(...)` that dispatches a proposer and records the
  proposal batch.

This is the machinery GEPA should probably be using or mirroring honestly.

### Evidence Payload Access Is Not In `ProposalContext`

The graph view can expose assessment records and evidence refs, but direct
typed payload retrieval currently lives on `RunContext::assessment_evidence`.

That may be correct if optimizers are supposed to lower evidence into explicit
proposer requests. It is not correct if proposer stages are supposed to be able
to inspect trace/evidence themselves.

The broader audit needs to decide the intended contract and make the API
enforce it.

### `RunContext` Can Do The Right Thing, But Bypasses Are Too Easy

GEPA currently calls `record_proposal_batch(...)` directly after doing its own
local proposal logic. That API is necessary for optimizer authors, but it also
means optimizers can skip `Proposer<P>` entirely.

The question is whether this is intended power-user flexibility or whether
default optimizer implementations should be forced through engine proposer
stages for traceability and consistency.

## Layer 3 Audit Questions For The Broader Pass

- Does `ProposalContext` need scoped evidence-store access?
- Are read scopes actually enforced across graph views and evidence payloads?
- Are graph mutation paths narrow enough, or do optimizer implementations get
  too many ways to partially reimplement stage behavior?
- Are stage errors typed enough for optimizer authors to make decisions?
- Are budget charges attached to the right stage for LM/agent reflection?
- Does checkpoint state include every private strategy decision that changes
  future behavior?
