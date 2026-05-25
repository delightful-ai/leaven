# Leaven Python Scaffold + Vendor Session — 2026-05-24

Status: session status note.
Updated: 2026-05-24 PM.

## Authority

This is a session status note covering autonomous work done while the user
was away at a dentist appointment. It is not product law. Specs/code/tests
win when this note disagrees with them.

Subordinate to:

- `docs/specs/leaven_python.md` (governing product spec)
- `docs/specs/leaven_py/` (the scaffold instance)
- `docs/working-memory/leaven-py-and-acp-transport.md` (research + decisions ledger)
- `docs/working-memory/leaven-py-and-acp-transport-handoff.yaml` (goal handoff artifact)

## What landed in this session

### Spec

- `docs/specs/leaven_python.md` — added "Public API discipline" section.
  Two tiers (public in `__all__` + no underscore vs private otherwise),
  one rule ("if it isn't in `__all__` and unprefixed, it doesn't exist"),
  three sub-rules (every public module ships `__all__`; namespace
  submodules vs leak submodules; no backdoor exports), deprecation
  policy for V1.

### Scaffold (docs/specs/leaven_py/)

- 73 modules: types + signatures + docstrings, `__all__` everywhere,
  pydantic v2 frozen + `extra="forbid"`, full type hints. Imports
  cleanly; `uv run ty` passes; `uv run ruff` passes.
- Decorators (`@lv.evaluator`/`@lv.reflector`/`@lv.proposer`/`@lv.runner`/`@lv.scorer`/`@lv.judge`)
  return real `RegisteredStage` values at decoration time. Only
  `lv.optimize(...).run()` and `lv.serve_stage(...)` raise
  `NotImplementedError` at the engine boundary. Composition fires taste;
  execution waits for the engine.
- 8 runnable example scripts under `examples/`:
  - `01_environment.py` — full `lv.environment(...)` composition
  - `02_cases_and_artifacts.py` — `PromptArtifact`, `SkillBank`, case loader shape
  - `03_prompt_optimize.py` — canonical minimal sketch (~25 lines)
  - `04_evoskill_skill_bank.py` — canonical big sketch (~80 lines)
  - `05_evaluator_with_judge.py` — rich `@lv.evaluator` body
  - `06_reflect_propose_custom.py` — custom GEPA stage override
  - `07_serve_stage_worker.py` — standalone Python worker
  - `08_dspy_dropin.py` — DSPy integration
- `examples/run_all.py` tour runner; `examples/README.md` per-example
  table; `examples/fixtures/arithmetic.jsonl` 8-case fixture.
- `justfile` recipes: `just sync`, `just examples`, `just example N`,
  `just check`, `just compile-examples`, `just all`.
- `pyproject.toml`: pydantic + typing-extensions runtime, dspy-ai
  optional extra, pytest + ruff + ty dev group, ruff lint config with
  `repos/**` exempted from linting.

### AGENTS.md (docs/specs/leaven_py/)

- Added "Public API discipline" section mirroring the spec.
- Added "Vendored Repositories" section per the
  `vendor-key-dependency` skill template.

### Vendored repos (docs/specs/leaven_py/repos/)

Phase 1 of 3 (per
`docs/specs/leaven_py/docs/agent-context/python-inspiration-inventory.md`):

| repo | upstream | size | added for |
|------|----------|------|-----------|
| `dspy/` | stanfordnlp/dspy@main | ~23 MB | `BaseLM` adapter + decorator patterns |
| `inspect_ai/` | UKGovernmentBEIS/inspect_ai@main | ~38 MB | `@solver`/`@scorer`/`@task` + `TaskState` context |
| `mcp-python-sdk/` | modelcontextprotocol/python-sdk@main | ~4 MB | stdio JSON-RPC + FastMCP idioms + #2433 failure mode |

Inventory + add/update commands + per-repo "read first" hints in
`docs/specs/leaven_py/docs/agent-context/vendored-repositories.md`.

Per-repo pattern observations (what to steal / avoid / surprising)
queued — see `docs/specs/leaven_py/docs/agent-context/patterns/`. If
the three pattern agents that were dispatched complete, you'll find:

- `patterns/dspy-patterns.md`
- `patterns/inspect-patterns.md`
- `patterns/mcp-patterns.md`

If any of those is missing when you read this, the dispatched agent may
have crashed; the README in `docs/agent-context/` describes the layout.

### Workflow skills (~/plans/knowledge/)

Extracted from this session's work pattern (parallel research agents
writing durable files, fork-and-review for design-altitude work):

- `parallel-write-then-converge/SKILL.md` (145 LOC, restructured by
  Fork C after Fork B's adversarial review)
- `parallel-write-then-converge/REVIEW-2026-05-24.md` (Fork B's review,
  kept alongside for future iteration)
- `forked-self-agents/SKILL.md` (240 LOC, Fork D writing from scratch)

### Commit history (jj)

Three commits on top of `towylwut public-seam: harden ACP launch and scope negatives`:

1. `tests: parallelize libtest binaries with RUST_TEST_THREADS=1` — your
   tangential test-SLA work that I split into its own commit.
2. `leaven-py: spec, scaffold, examples, research, vendor prep` — the
   spec + scaffold + examples + research notes + skills, in one
   coherent change.
3. `leaven-py: vendor Phase 1 + agent context` (this commit) — the
   three subtree-add merges + vendored-repositories.md + AGENTS.md
   patches + pattern observation files (when those agents complete).

## What changed about the broader plan

You mentioned in passing: "we've also implemented!! all of the ACP shit
so we're READY ready!!!" — if the Rust-side ACP transport is in fact done
and the three blocked conformance rows
(`ps1.acp.transport_profile`, `ps1.acp.extension_results`,
`ps1.acp.lifecycle_backpressure`) are now provable, the implementation
plan in
`docs/working-memory/leaven-py-and-acp-transport-handoff.yaml`
needs updating. I did NOT update that yet because I wanted to verify the
claim with you (and the matrix) before promoting rows.

Suggested verification command:

```bash
# From the leaven workspace root
grep -A 1 "status: blocked" docs/specs/public-seam-v1/conformance-matrix.yaml
```

If the three ACP rows are now `proven`, the handoff artifact's
`tied_to_p5_timeline` decision still holds (the closeout gate is the
P5-shaped GEPA+agentic-reflector live-LM run), but the
`acp_transport_async_rewrite` acceptance row can shift to `proven` and
the timeline estimate collapses.

## Taste calls awaiting you

In order of how load-bearing they are:

1. **Public/private API discipline review.** The spec section + AGENTS.md
   patch describe the rule. The scaffold has reasonable underscore
   conventions (`_handles.py`, `_receipts.py`, `_EnvironmentBuilder`,
   `_CacheNamespace`) but I haven't done a full audit. When you have
   time, walk the `lv.*` top-level surface in
   `python -c "import leaven; print(sorted(n for n in dir(leaven) if not n.startswith('_')))"`
   and flag any name that should be private but isn't.

2. **The 8 examples.** You said the earlier batch was "really bad" so
   these are written fresh. They span environment / cases / minimal-opt
   / EvoSkill-big / rich-evaluator / reflect-propose-custom /
   serve-stage / DSPy. Each runs end-to-end (`NotImplementedError` at
   engine boundaries is caught as expected). If any feels wrong, the
   shape is what fires taste — adjust the source and `just check` will
   catch type breaks.

3. **Vendored repo selection.** Phase 1 is in. Phase 2 (LangGraph,
   OpenAI Evals, OSS Vizier) and Phase 3 (CrewAI, Modal) are deferred
   to when scaffold ergonomics stabilize. Skipped explicitly: Ray Tune,
   Pydantic (already a runtime dep), Optuna. Inventory + reasoning in
   `docs/specs/leaven_py/docs/agent-context/python-inspiration-inventory.md`.

4. **Open API questions surfaced by the sketches.** The Python surface
   sketches file
   (`docs/working-memory/leaven-py-research/2026-05-24-python-surface-sketches.md`)
   §7 has 7 specific open questions (`cx.batch()` placeholder geometry,
   artifact class location, benchmark catalog policy, `lv.serve_stage()`
   launch contract details, result type variance, `lv.scoring.*`
   placement). The scaffold made choices on most of these (described in
   each module's docstring); you may want different.

5. **Whether to publish.** The README / pyproject / examples are clean
   enough that you could `uv publish` this to TestPyPI as a "shape
   preview" if you wanted a public sniff test. Probably not worth it
   until the engine wires up.

## What's queued

Per task list at end of session:

- All Phase 1 vendor + docs + AGENTS.md work: done
- Pattern observation files: in flight (3 parallel agents)
- This status note: in flight (you're reading it)

What I did NOT do (deliberate):
- Update the handoff artifact's blocked rows based on the "ACP done"
  claim (waiting on your verification)
- Vendor Phase 2 (explicit user decision: defer)
- Refactor scaffold based on what I'd see in vendored sources (that's
  your taste call when you return)
- Anything in the `leaven` Rust workspace beyond reading state — the
  scaffold is fully self-contained under `docs/specs/leaven_py/`

## Recommended /goal framing

When you set the goal, the existing handoff artifact at
`docs/working-memory/leaven-py-and-acp-transport-handoff.yaml` is the
operational package. The proof denominator + non-goals + acceptance
gates are durable.

The two updates I'd suggest before activating the goal:

1. If the Rust ACP transport is in fact landed: mark
   `acp_transport_async_rewrite` acceptance row `proven` with links to
   the now-passing conformance rows. The closeout gate
   (`end_to_end_proof_of_life`) remains tied to the P5-shaped
   GEPA+agentic-reflector live run.

2. Add a "scaffold instance ready" note: the leaven_py scaffold at
   `docs/specs/leaven_py/` is the entry point for Python-side
   implementation. Future work that touches the Python surface should
   keep the scaffold and the governing spec in sync.

Then `/goal` against the handoff artifact, citing the spec, with the
scaffold as the implementation starting point and the per-tranche tests
from the conformance matrix as the proof denominator.

## Verification when you return

From `docs/specs/leaven_py/`:

```bash
just all
```

Runs `uv sync`, lint, type-check, and example-compile. If anything is
broken from the autonomous session, that's where it shows.

From the workspace root:

```bash
jj log -r 'mine() & trunk()..' --limit 5
```

Shows the three commits this session produced.

```bash
git -C . log --oneline --grep "subtree" -5
```

Shows the three subtree-add merges.
