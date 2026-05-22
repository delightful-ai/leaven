# EvoSkill Paper-Close Goalcraft

Ready-to-paste goal:

```text
/goal Destination: Get EvoSkill in /Users/darin/src/personal/leaven to paper-close, not product-proof. End state: a Rust/Leaven replica harness can run paper-close OfficeQA/SealQA with declared source universe, train/validation/test split manifest, paper scorer, multi-iteration git-program/frontier loop, checkpoint/resume, and a report that labels exactness gaps.

Context: Use docs/working-memory/skill-paper-replication.md, docs/working-memory/evoskill-replication.md, docs/specs/agentic_skill_optimization_primitives.md, docs/specs/agentic_task_execution_substrate.md, and docs/plans/2026-05-22-evoskill-paper-close/goal-handoff.yaml. Treat Git materialization/readback, trust benchmark, TopKFrontier/TopKParentSelector, eval split builders, and P5 fixtures as substrate, not completion.

Scope: Reconcile current code/docs/data. Build no-spend first: source manifest, OfficeQA/SealQA loaders or explicit blockers, exact or paper-close split materialization, paper scorer with tolerance/failure threshold, and fake-runtime full loop. Run a small live agent only if runtime/credentials exist and spend is approved. Wire real Leaven primitives: leaven-eval SourceRowManifest/split builders/CategoryRoundRobinSampler, leaven-artifact-git, leaven-workspace-git, agentic Git bridge, leaven-population, evidence/checkpoint/report surfaces. Keep paper-specific code thin/local unless a generic primitive is missing.

Preserve: No fake proofs. Do not claim paper-close from P5 fixture, Git bench, topology/just check, fake runtime, single sample inspect, missing exact splits, Python reference script, or live smoke alone. No Leaven-owned replication logic in Python. Do not expose hidden targets/categories to runners or bake EvoSkill into engine. Hard-cut APIs; no shims.

Verify: For each slice, run focused tests and update handoff evidence. Before any paper-close claim require manifest hash/split fingerprint report, loader/scorer law tests, fake-runtime multi-iteration checkpoint/resume test, git child lineage/admission test, and final report with baseline/optimized train/validation/test plus exactness classification. Run relevant cargo tests and final ulimit -n 8192 && CARGO_BUILD_JOBS=8 just check unless blocked.

Done/stop: Done only when the handoff says every paper-close acceptance item is proven or source-blocked and closeout rejects proxy artifacts. Stop for destructive dataset/source rewrites, unclear paper-release revision, missing private/gated data that changes denominator, provider spend/credential ambiguity, or pressure to call paper-close paper-exact.
```

Objective length: 2575 characters.
