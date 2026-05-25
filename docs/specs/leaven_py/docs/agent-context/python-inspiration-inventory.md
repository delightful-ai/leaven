# Python Inspiration Inventory — Vendoring Decisions

**Date**: 2026-05-24  
**Purpose**: Scoped vendor list for key upstream Python libraries that shaped the Leaven Python scaffold's design.

The Leaven Python surface is a ~200-line composition API with decorator-based stage registration, typed async context objects (`RunContext`, `StageContext`, `EvalContext`), builder namespaces for case/workspace/LM/agent/sandbox access, and wire-shaped result types. The inspirations below informed specific patterns; this document decides which to vendor for agent context and which to reference via web-only.

---

## 1. DSPy

**Upstream**: https://github.com/stanfordnlp/dspy  
**Default branch**: main  
**Latest tag**: 3.2.1 (at writing)  
**Approximate size**: ~3–5 MB clone; ~50–100k LOC (estimated from 4,526 commits)  
**Type**: Single-package Python framework  

### What shaped our design
- `dspy.BaseLM` + `dspy.configure(lm=...)` pattern — influenced `lv.lm.configure()` and the LM provider registry abstraction
- Decorator + module introspection for program composition — echoed in `@lv.runner`, `@lv.scorer`, `@lv.evaluator` decorators
- Prompt + artifact manipulation as first-class types — `PromptArtifact` templates follow DSPy's material-artifact model

### Worth reading for our scaffold
- `dspy/clients/lm.py` — BaseLM interface contract and provider registration
- `dspy/programs/` — decorator registration and module traversal patterns
- `dspy/examples/` — composition and invocation idiomatic shapes

### Vendor recommendation
**`full`** — DSPy is compact, well-scoped to a single Python package, and the decorator + LM-provider patterns are load-bearing for our context builders. Small enough that full vendoring is justified; no monorepo overhead.

### Justification
DSPy's BaseLM interface and decorator registration model directly inform `lv.lm.LmBuilder` and stage decorators. Full vendoring lets agents read real working examples of LM routing, prompt templates, and composition glue without hunting web docs.

---

## 2. Inspect AI

**Upstream**: https://github.com/UKGovernmentBEIS/inspect_ai  
**Default branch**: main  
**Commits**: ~700+ (estimated from GitHub activity)  
**Approximate size**: ~15–25 MB clone; Python 99.9%  
**Type**: Single-package evaluation framework  

### What shaped our design
- `@solver`, `@scorer`, `@task` decorator patterns — direct influence on Leaven's `@runner`, `@scorer`, `@evaluator` stage roles
- `TaskState` + context object injection — echoed in `RunContext`, `StageContext`, `EvalContext`
- Async evaluation pipeline with state threading — foundational to our stage call model

### Worth reading for our scaffold
- `src/inspect_ai/solver/` — solver decorator and composition
- `src/inspect_ai/scorer/` — scorer registration and evaluation
- `src/inspect_ai/task/` — task state and context injection patterns
- `src/inspect_ai/_eval/` — evaluation loop and async state threading

### Vendor recommendation
**`full`** — Inspect AI is small (< 25 MB), cohesive, and its decorator + context patterns are nearly 1:1 with Leaven's stage model. No monorepo bloat; pure signal.

### Justification
Inspect's `@solver`, `@scorer` decorators and `TaskState` are the most direct design ancestors of Leaven's stage context system. Full vendoring allows agents to see the real evaluation loop, context injection, and async threading that our surface extends. This is critical for implementing stage decorators correctly.

---

## 3. Optuna

**Upstream**: https://github.com/optuna/optuna  
**Default branch**: master  
**Latest release**: 4.8.0 (March 2026)  
**Approximate size**: ~100–200 MB clone; 20,945 commits; Python 100%  
**Type**: Single-package hyperparameter optimization framework  

### What shaped our design
- Distributed optimization with gRPC storage backend — informs `lv.environment.Environment` and budget/worker model
- Study/trial/sampler abstraction — partial echo in optimizer config registry (though Leaven's optimizers are Rust-native)
- Worker/server split for distributed evaluation — relates to Leaven's engine/SDK separation philosophy

### Worth reading for our scaffold
- `optuna/study/study.py` — study and trial lifecycle
- `optuna/samplers/` — sampler interface (not reused, but shows distributed algorithm patterns)
- `optuna/storages/rpc_storage.py` — gRPC client pattern for distributed state
- `optuna/trial/` — trial state and context threading

### Vendor recommendation
**`subset:optuna/{samplers,study,trial,storages/rpc_storage.py}`** — Vendor only the core abstractions, not the full sampler/pruner implementations or database drivers. Excludes dashboard, examples, tests.

### Justification
Optuna is too large for full vendoring (~200 MB) and most of the sampler/pruner ecosystem is not applicable to Leaven (our optimizers are Rust-native). But the Study/Trial/Sampler interface contract and the gRPC distributed-storage pattern are worth reading for agents implementing `lv.environment` and worker coordination. Subset vendoring keeps repo size reasonable.

---

## 4. Ray Tune

**Upstream**: https://github.com/ray-project/ray  
**Default branch**: master  
**Approximate size**: 2+ GB clone (monorepo containing Ray core, Tune, Serve, AIR, etc.); Python 76.7%, C++ 17.2%  
**Type**: Monorepo (Ray ecosystem)  

### What shaped our design
- `Trainable` API with `train()` + `save_checkpoint()` — loosely parallels `@lv.runner` + artifact materialization
- `tune.report()` callback for incremental progress — echoes in our proposal/assessment submission model
- Distributed training + tuning composition — relates to `lv.environment` worker/server model

### Worth reading for our scaffold
- `python/ray/tune/trainable/trainable.py` — Trainable API contract
- `python/ray/tune/stopper/` — stopping criteria patterns (relates to budget/termination)
- `python/ray/tune/result_grid.py` — result aggregation and inspection

### Vendor recommendation
**`skip`** — Ray is 2+ GB for a monorepo; `python/ray/tune/` alone is likely 50–100 MB. The Trainable API is simpler than Leaven's stage model and the sampler integration is Ray-specific. Web-only reference is sufficient; agents can read Ray Tune docs on demand.

### Justification
Too large and too tangentially related. The `Trainable` API and `tune.report()` patterns are straightforward; web search suffices. Vendoring Ray just for Tune APIs is not worth 100+ MB of disk. The gRPC patterns matter more (Optuna covers this better at smaller scale).

---

## 5. OSS Vizier

**Upstream**: https://github.com/google/vizier  
**Default branch**: main  
**Commits**: 1,230  
**Approximate size**: ~20–40 MB clone; Python 99.8%  
**Type**: Single-package optimization service  

### What shaped our design
- `Policy.suggest()` interface for algorithm abstraction — conceptual parallel to optimizer config plugability (though Leaven optimizers are Rust)
- `Designer` API for black-box optimization — relates to stage composition and artifact handling
- gRPC-based client/server split — reinforces wire/engine separation philosophy

### Worth reading for our scaffold
- `vizier/service/` — gRPC service contract and policy interface
- `vizier/algorithms/` — algorithm abstraction (JAX-based; Bayesian optimization patterns)
- `vizier/client/` — client-side API and session management

### Vendor recommendation
**`full`** — OSS Vizier is compact (~30 MB), Python-focused, and the Policy/Designer abstraction + gRPC wire patterns are directly relevant to `lv.optimize()` composition and the ACP wire model. Small enough to justify full vendoring.

### Justification
OSS Vizier's Policy/Designer interface and gRPC client/server separation are architectural cousins to Leaven's optimizer config + wire model. Full vendoring lets agents understand Google's scale-out optimization patterns without hitting the web. Size is reasonable (< 40 MB).

---

## 6. OpenAI Evals

**Upstream**: https://github.com/openai/evals  
**Default branch**: main  
**Commits**: 691  
**Approximate size**: ~10–20 MB clone (Python 89.4%, includes YAML + JSON data); Git-LFS for evaluation data  
**Type**: Single-package evaluation registry + templates  

### What shaped our design
- Eval registry pattern (Python + YAML, single-process) — directly parallels `lv.optimize(...).run()` registry model
- Template-based eval definition (no-code YAML configs) — relates to artifact templates and case materialization
- Model-graded evaluation patterns — echoes in `@lv.judge` and assessment submission

### Worth reading for our scaffold
- `evals/registry/` — registry structure and lookup
- `evals/base.py` — eval and completion classes
- `evals/solvers/` — solver patterns (simple evaluator implementations)
- `evals/templates/` — YAML config patterns for evaluations

### Vendor recommendation
**`full`** — OpenAI Evals is small (< 20 MB code, excludes Git-LFS data), tightly scoped, and the eval registry + template patterns are foundational to how `lv.optimize()` and `lv.evaluator` work. Single-process registry model is directly applicable.

### Justification
The registry pattern and YAML+Python split are load-bearing for `lv.cases.from_yaml()` and stage decorator lookup. Full vendoring removes web dependency for understanding how eval configs compose. Git-LFS data is optional; we vendor only the code layer.

---

## 7. MCP Python SDK

**Upstream**: https://github.com/modelcontextprotocol/python-sdk  
**Default branch**: main  
**Stars**: 23.1k; Forks**: 3.5k  
**Approximate size**: ~5–15 MB clone; Python primary; v2 in development  
**Type**: Single-package SDK for wire protocols  

### What shaped our design
- Stdio JSON-RPC wire transport — foundational to Leaven ACP profile (though Leaven owns the schema)
- FastMCP decorator framework — parallels stage decorator + router model
- Tool/resource/prompt request/response abstraction — relates to builder namespaces and capability tokens

### Worth reading for our scaffold
- `src/mcp/server/session.py` — session lifecycle and transport routing
- `src/mcp/server/fastmcp.py` — FastMCP decorators and registration
- `src/mcp/shared/json_rpc.py` — JSON-RPC wire protocol
- `src/mcp/client/` — client-side transport and session management

### Vendor recommendation
**`full`** — MCP SDK is small (< 15 MB), focused on wire protocol patterns, and the stdio JSON-RPC transport + decorator model are directly relevant to how Leaven's stage calls route through the ACP wire. No monorepo bloat.

### Justification
MCP's stdio JSON-RPC transport and FastMCP decorator framework inform how Leaven's context builders route requests to the engine. Although Leaven owns its ACP schema (not MCP-compatible), understanding the reference implementation prevents agents from making costly wire-level mistakes. Known issue (CRLF bug python-sdk#2433) is worth reading to avoid.

---

## 8. LangGraph

**Upstream**: https://github.com/langchain-ai/langgraph  
**Default branch**: main  
**Commits**: 6,880  
**Approximate size**: ~50–100 MB clone; Python 99.5%; Stars: 32.8k  
**Type**: Single-package graph orchestration framework  

### What shaped our design
- `StateGraph.add_node(name, func)` pattern — parallels `lv.optimize(...).runner(func)` + `lv.optimize(...).scorer(func)` registration
- `RunnableConfig` injection for context — echoes in `RunContext`, `StageContext` parameter injection
- State threading + durability — relates to assessment/proposal submission and state reconstruction

### Worth reading for our scaffold
- `langgraph/graph/` — StateGraph and node registration
- `langgraph/types.py` — RunnableConfig and context threading
- `langgraph/pregel/` — execution and state threading logic
- `langgraph/checkpoint/` — durability and resumption patterns

### Vendor recommendation
**`full`** — LangGraph is medium-sized (~75 MB), well-organized, and the StateGraph + RunnableConfig patterns are load-bearing for how Leaven's context and stage composition work. No monorepo overhead; clean single-package boundary.

### Justification
LangGraph's StateGraph.add_node() and RunnableConfig injection are direct architectural parallels to `lv.optimize(...).runner(...).scorer(...)` composition and context parameter threading. Full vendoring lets agents read the durability and state-reconstruction patterns that inform how Leaven handles resume and artifact materialization.

---

## 9. Modal

**Upstream**: https://github.com/modal-labs/modal-client  
**Default branch**: main  
**Commits**: 8,536  
**Approximate size**: ~100–200 MB clone (Python 74.7%, Go 12.9%, TypeScript 12.3%); Stars: 474  
**Type**: Monorepo (Python, Go, TypeScript SDKs + proto definitions)  

### What shaped our design
- Python decorator deployment ergonomics (`@app.function`, `@app.cls`) — influenced `@lv.runner`, `@lv.scorer` decorator design
- App definition + composition model — echoes in `lv.optimize(...)` builder pattern
- Async function serialization + remote execution — parallels context object injection and remote stage dispatch

### Worth reading for our scaffold
- `modal_proto/` — protocol buffer definitions for RPC (relevant to wire patterns)
- `py/modal/` — Python SDK decorators and app composition
- `py/modal/functions.py` — function decoration and parameter serialization

### Vendor recommendation
**`subset:py/modal/`** — Vendor only the Python SDK (`py/modal/`), exclude `modal_proto`, JS, and Go SDKs. Keeps scope reasonable (~50 MB vs 150+ MB for full monorepo).

### Justification
Modal's Python decorator model and remote-function serialization directly influenced our stage decorator design. However, the full monorepo (150+ MB) is oversized; vendoring just `py/modal/` (~50 MB) captures the decorator patterns without Go/TypeScript/proto overhead. Agents need to see how Modal handles async function serialization and remote dispatch, which is a close analogue to Leaven's stage-call serialization.

---

## 10. CrewAI

**Upstream**: https://github.com/joaomdmoura/crewai  
**Default branch**: main  
**Commits**: 2,433  
**Approximate size**: ~20–30 MB clone; Python 98.7%; lean codebase  
**Type**: Single-package agent composition framework  

### What shaped our design
- `@agent`, `@task`, `@crew` decorator stacking — parallels multi-stage composition (e.g., `@lv.evaluator` + `@lv.proposer`)
- YAML + Python config split — echoes in case/artifact template patterns
- Role-based agent composition — relates to `lv.agent.AgentBuilder` and agent instructions

### Worth reading for our scaffold
- `src/crewai/agent/agent.py` — agent composition and execution
- `src/crewai/tasks/task.py` — task definition and execution
- `src/crewai/crews/crew.py` — crew composition and role assignment
- `src/crewai/tools/` — tool/capability abstraction (parallels `lv.sandbox.exec()`, `lv.workspace.materialize()`)

### Vendor recommendation
**`full`** — CrewAI is small (< 30 MB), focused, and its decorator stacking + role-based composition patterns are relevant to how agents use Leaven's multi-stage API. No monorepo overhead; clean package boundary.

### Justification
CrewAI's role-based agent composition and decorator stacking are direct ancestors to how Leaven agents compose `@reflector` + `@proposer` + `@judge` stages. Full vendoring lets agents understand role assignment, task chaining, and tool/capability delegation without web lookups. Load-bearing for agentic reflection surface (Stage 2+).

---

## 11. Pydantic

**Upstream**: https://github.com/pydantic/pydantic  
**Default branch**: main  
**Commits**: 5,518  
**Approximate size**: ~100–150 MB clone; Python 83.3%, Rust 16.6% (pydantic-core); Stars: 27.8k  
**Type**: Single-package validation library (with Rust accelerator)  

### What shaped our design
- Type-hint-driven validation + data-class generation — foundational to `lv.data_class.DataClass` and artifact type projection
- Serialization/deserialization + field metadata — informs `lv.output_record.OutputRecord` and assessment records
- Config + override patterns — echoes in `lv.environment.Cache` and stage-config registration

### Worth reading for our scaffold
- `pydantic/main.py` — BaseModel type contract and validation
- `pydantic/fields.py` — field metadata and constraints
- `pydantic/json_schema.py` — JSON Schema codegen (relevant to wire payloads)
- `pydantic/config.py` — config and override model

### Vendor recommendation
**`note-only`** — Pydantic is already a runtime dependency of the Leaven Python scaffold (`pyproject.toml` pins it). Vendoring would create a local copy while the live version is imported, causing confusion. Instead, agents should reference the live installed Pydantic and web docs. Document the version pinning and reference strategy in scaffold docs.

### Justification
Pydantic is already vendored by virtue of being a direct transitive dependency. Creating a redundant `repos/pydantic/` copy invites version skew bugs. Better to document the pinned version in `pyproject.toml` and link to Pydantic's docs/source where needed. Agents can `import pydantic` and read live source within the venv if deep inspection is needed.

---

## 12. Verifiers (Prime Intellect)

**Upstream**: https://github.com/PrimeIntellect-ai/verifiers  
**Default branch**: main  
**Approximate size**: ~20–35 MB clone; Python 97%  
**Type**: RL/eval environment library + Prime CLI integration  

### What shaped our design
- `load_environment(config)` module entrypoint — self-contained eval packages users can bring unchanged
- Taskset rows + `@vf.reward` rubrics — dataset loading and scorer semantics
- v1 Taskset/Harness split — parallels Leaven runner/scorer/case separation
- `HarborTaskset` bridge — shows how Harbor tasks lower into row dicts

### Worth reading for our scaffold
- `docs/overview.md` — environment module contract
- `verifiers/v1/taskset.py`, `verifiers/v1/env.py` — row → rollout pipeline
- `verifiers/rubrics/rubric.py`, `verifiers/decorators.py` — reward/score functions
- `verifiers/v1/packages/tasksets/harbor.py` — Harbor interop

### Vendor recommendation
**`full`** — Vendored 2026-05-24 as Phase 2. See `docs/agent-context/patterns/verifiers-patterns.md`.

---

## 13. Harbor (agent eval harness)

**Upstream**: https://github.com/harbor-framework/harbor  
**Default branch**: main  
**Approximate size**: ~43–49 MB clone; Python 92%  
**Type**: Agent evaluation CLI + task/dataset registry (Terminal-Bench official harness)  

**Not this repo:** `meridianlabs-ai/inspect_harbor` (Inspect AI registry adapter only).

### What shaped our design
- Task directory layout (`instruction.md`, `task.toml`, `tests/`, `environment/`) — informs how Leaven could import containerized benchmark cases into `lv.cases.*`
- Trial/job orchestration (`harbor run`) — parallels `lv.optimize(...).run()` with concurrency and durable job dirs
- `BaseAgent` / verifier scripts — runner vs scorer separation with sandbox boundary
- Dataset manifest digest pinning — aligns with Leaven case identity / cache/resume obligations

### Worth reading for our scaffold
- `src/harbor/models/task/task.py` — task filesystem contract
- `src/harbor/models/dataset/manifest.py` — dataset.toml pinned refs
- `src/harbor/agents/base.py` — agent lifecycle
- `src/harbor/verifier/verifier.py` — reward extraction from test scripts
- `examples/tasks/hello-world/` — minimal task skeleton

### Vendor recommendation
**`full`** — Harbor is medium-sized (~43 MB), cohesive, and the task + verifier layout is load-bearing for agentic eval compatibility. Vendored 2026-05-24 as Phase 2.

### Justification
Harbor is the de facto harness for Terminal-Bench and many registry benchmarks. Vendoring clarifies task semantics vs Prime Verifiers vs Inspect AI (`inspect_harbor`). Leaven adapters should read real task layout here, not infer from Inspect registry strings alone.

---

## Recommended Vendor Order

### **Phase 1 — Load-bearing patterns (add first)**
1. **DSPy** — LM provider registry + decorator patterns (core to context builders)
2. **Inspect AI** — Stage decorator + context injection model (direct design ancestor)
3. **MCP Python SDK** — Wire protocol + JSON-RPC patterns (foundation for ACP routing)

### **Phase 2 — Composition & execution (add after Phase 1)** ✅ partial (2026-05-24)
4. **Verifiers** — `load_environment` + Taskset/Rubric + `@vf.reward` (vendored)
5. **Harbor** — task directories + verifier scripts + dataset registry (vendored)
6. **LangGraph** — StateGraph + RunnableConfig + durability patterns (skipped earlier round; revisit if needed)
7. **OpenAI Evals** — Registry + template patterns (skipped earlier round)
8. **OSS Vizier** — Policy/Designer + gRPC client patterns (skipped earlier round)

### **Phase 3 — Agentic reflection (add for Stage 2+ work)**
9. **CrewAI** — Role-based composition and multi-stage orchestration
10. **Modal** (subset: `py/modal/`) — Decorator + remote dispatch patterns

### **Skip entirely**
11. **Ray Tune** — Too large (2+ GB monorepo); web-only reference sufficient
12. **Pydantic** — Already a live dependency; use installed version + web docs
13. **Optuna** — Distributed optimization patterns less critical than OSS Vizier; decide based on schedule.

### **Total estimated vendored size**
- Phase 1: ~35 MB (DSPy + Inspect + MCP)
- Phase 2 (eval frameworks): ~63 MB (Verifiers 20 MB + Harbor 43 MB) — **vendored 2026-05-24**
- Round 4 references: ~150 MB (BAML, pydantic-ai, Temporal, Marvin, Anthropic SDK, etc.)
- **Current total**: ~289 MB (12 subtree vendors + informal Weave copy)

---

## Implementation Notes

1. **Git subtree command**: Use `git subtree add --squash https://github.com/<owner>/<repo>.git docs/specs/leaven_py/repos/<slug>/` from the `leaven_py/` directory.

2. **Subset vendoring** (e.g., Modal, Optuna): After subtree add, delete unwanted directories locally, then `git subtree split` to create a clean subset branch before final commit.

3. **Known issues to document**:
   - MCP Python SDK issue #2433 (CRLF in stdio) — note in agent context docs
   - Inspect AI requires `uv` or pip with dev extras; document setup in scaffold README

4. **Version pinning**: Add a `vendors.lock` file to `docs/agent-context/` recording each vendored repo's branch/tag at vendoring time, for reproducibility.

5. **Agent context discovery**: Add `docs/agent-context/README.md` explaining the vendor directory structure and how to search it (e.g., `grep -r "decorator"` to find decorator patterns across all repos).

