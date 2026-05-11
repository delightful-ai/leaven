# Review Tree Contract

Before editing or adding audit findings in this directory, read:

1. `complaints/session-user-messages-for-codex.md`
2. `auditing-conventions.md`
3. `README.md`
4. `inventory/audit-plan.md`
5. `inventory/known-findings.md`

This review tree is an audit artifact, not a fix branch. Do not change product
code from here unless the user explicitly asks for implementation.

## Audit Rules

- Findings must cite current repo paths and line numbers.
- Findings must distinguish public API promises from internal implementation
  gaps.
- Do not accept a nearby proxy as proof that the library works.
- Do not mark scaffolding as acceptable merely because tests pass.
- Do not invent compatibility plans. Leaven uses hard cutovers.
- If a crate or surface is healthy, say what was checked and why it is not a
  finding.
- If a claim depends on specs, cite the spec path and the code path.

## Layer Routing

- Layer 1 findings go under `surfaces/layer-1-user/`.
- Layer 2 findings go under `surfaces/layer-2-gepa-customizer/`.
- Layer 3 findings go under `internals/layer-3-engine-author/`.
- Cross-cutting crate graph, cache, LM, placeholder, example, and topology
  findings go under `cross-cutting/` or `inventory/`.

