# Leaven Python SDK + ACP Transport Research

Status: active research, pre-spec.
Updated: 2026-05-24.

## Authority

This note is subordinate to:

- `docs/specs/public-seam-v1/` (the locked V1 public seam — plan IR, capability
  tokens, result receipts, stage payloads, evidence envelopes, ACP profile,
  JSON schemas).
- `docs/specs/public-seam-v1-lock-draft.archived/COMPREHENSIVE_DESIGN_PASS_NOTES.md`
  (archived but load-bearing design rationale for the Python authoring surface,
  the explicit pyo3 rejection at line 29, the "200-line Python evaluator"
  target at line 21, and the host-language-interior / typed-boundary split at
  lines 33 / 69-71).
- `crates/AGENTS.md:20-24` and `crates/AGENTS.md:48-53` (the new `leaven-acp`
  crate topology committed during this conversation).
- `AGENTS.md:40` (the new `leaven-acp` map entry in the repo-wide routing).
- `docs/plans/2026-05-24-public-seam-v1-acp-transport-route.md` (the original
  ACP transport planning note from 2026-05-24, predating the topology lock).
- `docs/specs/public-seam-v1/conformance-matrix.yaml` (the row-level proof
  denominator; 32/39 rows proven, 3 blocked on ACP transport, 4 pending on
  runtime row work).

It records the pre-spec research phase for the Python SDK + ACP transport
package and is not proof of any implementation.

## Conversation Synthesis

A multi-turn design conversation on 2026-05-24 produced the following live
shape, which this research phase is meant to validate before any spec/plan
write:

**One product, many consumers.** The user's product question is "how do we
make EvoSkill-shaped paper repros 200 lines of Python glue instead of 8000
lines of Rust glue, without losing capability tokens, data classes, receipts,
or replay?" The answer architecture is:

- `leaven` binary (built by `leaven-cli`) is one binary with subcommands.
  `leaven serve --stdio` hosts an ACP-shaped JSON-RPC engine; `leaven query
  lineage`, `leaven runs list`, `leaven artifact show`, `leaven optimize` are
  CLI subcommands consumed by humans, agents-in-workspaces, and shell scripts.
- `leaven-acp` is a new crate that owns the hot stdio process/session
  transport, implementing the locked Leaven ACP profile in-house (Path B) and
  delegating Leaven method/result truth to `leaven-public-seam`. It is not
  dependent on the third-party `agent-client-protocol` SDK in this slice. The
  AGENTS.md hedge allows a later migration after external-dependency approval.
- `leaven-types` is a Python package code-generated from
  `docs/specs/public-seam-v1/schemas/*.schema.json` (typed records for plan
  IR, capability, plan result, stage payloads, evaluator job, evidence
  envelope). Schemas are pure JSON Schema 2020-12, kind-discriminated, no
  advanced features; codegen is mechanical (~1-2 days).
- `leaven` Python package is the SDK. It is the Python equivalent of
  `leaven-run`'s product-builder API — the way a Python user sets up,
  drives, and inspects an entire Leaven optimization run, not a stage
  authoring kit. It spawns the `leaven` binary as a child process and
  speaks ACP JSON-RPC over stdio via `leaven-acp`-compatible transport.
  The full surface covers:
  - **artifact definition** — typed Python representations of what's being
    optimized (prompt, skill bank, git repo, agent harness)
  - **case definition** — train/val/test splits, schemas, loaders (JSONL,
    parquet via `leaven-eval-parquet`, custom)
  - **optimizer configuration** — pick from registry (`lv.optimizers.gepa(...)`,
    `lv.optimizers.mipro(...)`, etc.), set knobs (population, parent
    selector, frontier capacity, reflection LM)
  - **environment configuration** — workspace backend, LM providers, agent
    runtime, sandbox policy, trust profile, budget, evidence store, cache
  - **stage authoring** — `@lv.evaluator`, `@lv.reflector`, `@lv.proposer`,
    `@lv.runner`, `@lv.scorer`, `@lv.judge` decorators (one component of
    the surface, not its whole shape)
  - **query/effect builders** — `cx.case`, `cx.workspace`, `cx.lm`,
    `cx.agent`, `cx.sandbox`, `cx.assessments`, `cx.proposals`, `cx.batch()`
  - **run composition + execution** — `lv.optimize(...).run()` returning
    typed `Optimized[Artifact]` with reports, lineage, receipts
  - **inspection + replay** — query the RunGraph from Python (`run.lineage()`,
    `run.evidence()`, `run.assessments()`), replay deterministically, fork
  - **observability** — events, progress, cost streams during the run

  Authoring new optimizers stays Rust-side (engine state binding makes
  Python optimizer authoring incoherent); composing/configuring existing
  optimizers, environments, artifacts, and stages is Python. Extending the
  system means writing a Rust crate; using the system means writing
  Python.
- `x.dspy.*` adapter namespace owns DSPy drop-in (`dspy.configure(lm=...)`)
  per `COMPREHENSIVE_DESIGN_PASS_NOTES.md:735`.
- The Python wheel ships the `leaven` Rust binary bundled (the `ruff` / `uv`
  pattern), so `pip install leaven` works without a Rust toolchain on the
  user's machine.

**pyo3 is rejected.** `COMPREHENSIVE_DESIGN_PASS_NOTES.md:29` lists the
reasons (manylinux wheel matrix, Python ABI versioning, GIL + Tokio
integration hell, locks Leaven to Python forever). An earlier sketch of an
in-process pyo3 transport in the conversation was withdrawn after the
archived design notes were surfaced.

**Why Path B (own the transport) over Path A (depend on the SDK):** the user
explicitly does not want to be coupled to the external SDK's versioning,
unstable feature gates, or future direction. The Leaven ACP profile is locked
in `docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md`;
implementing it in-house preserves wire compatibility with the spec without
the upstream coupling. The first agent's SDK audit found the load-bearing
substrate is small (stdio framing, JSON-RPC dispatch, session lifecycle,
cancellation, bounded queues), and most of the ceremony is already in
Leaven's spec layer.

## Open Research Questions

The conversation surfaced four questions that the spec write should not
guess on. Each has a dedicated research file:

1. **EvoSkill glue-code reality check.** What is actually painful in the
   current Rust EvoSkill replication path? Which lines / shapes of glue
   would a 200-line Python evaluator eliminate, and which would it not
   touch? See
   `docs/working-memory/leaven-py-research/2026-05-24-evoskill-glue-survey.md`.

2. **ACP SDK code inventory for Path B.** Concrete LOC count of the modules
   `leaven-acp` would need to reimplement (stdio, JSON-RPC dispatch, session
   lifecycle, cancellation, bounded progress). Validates or refutes the
   5-7 day estimate. See
   `docs/working-memory/leaven-py-research/2026-05-24-acp-sdk-code-inventory.md`.

3. **Multi-language future-proofing.** If we own the wire transport, what
   does the TypeScript / Go / shell-worker story look like? Are we
   accidentally locking Leaven into Python by going Path B, or does Path B
   actually make TS/Go/etc. simpler than Path A would have? See
   `docs/working-memory/leaven-py-research/2026-05-24-multi-language-future-proofing.md`.

4. **External-language-worker prior art.** How do other optimizer engines
   (Optuna, Ray Tune, Google Vizier, OpenAI evals, DSPy) expose
   external-language workers, if at all? What patterns are worth stealing
   and what failure modes are worth avoiding? See
   `docs/working-memory/leaven-py-research/2026-05-24-external-worker-prior-art.md`.

## Decision Log

### 2026-05-24 — Python SDK is the whole product surface, not just stages

Drift caught during research phase: the conversation's "decorator + builder"
framing implied the Python SDK was a stage-authoring kit. The user pushed
back: Python should be the way a user sets up, drives, and inspects a
whole Leaven optimization run end-to-end. The decorator surface is one
component, not the shape of the product.

What this changes:

- The leaven-py architecture section above was rewritten to enumerate the
  full surface (artifact / cases / optimizer / environment / stages /
  composition / inspection / observability), not just decorators.
- EvoSkill open question 1 ("does `@lv.optimizer` exist?") gets a no-but
  answer: Python picks from a registry of Rust optimizers and configures
  them; new optimizer authoring stays Rust-side because optimizer-strategy
  state binds tightly to engine state.
- `leaven-types` codegen scope expands beyond public-seam wire records to
  include typed handles for artifact, optimizer config, environment config,
  and run-result types.
- The ACP method surface must support inspection/replay (`graph.query`,
  `lineage.fetch`) as first-class consumer paths, not only as stage-callback
  internals.

200-line target reframed: 25 lines of `lv.optimize(...).run()` composition
plus user stage bodies, not 200 lines of evaluator function. The
composition glue is what disappears.

### 2026-05-24 — Synthesis across the four research files

Sources:

- `leaven-py-research/2026-05-24-evoskill-glue-survey.md` (EvoSkill glue,
  219 lines, agent ID `a46eb86f14e57550f`).
- `leaven-py-research/2026-05-24-acp-sdk-code-inventory.md` (ACP inventory,
  598 lines, agent ID `ae1aa8661f80e217d`).
- `leaven-py-research/2026-05-24-multi-language-future-proofing.md`
  (multi-language, 600 lines, agent ID `a19ab52f902dab766`).
- `leaven-py-research/2026-05-24-external-worker-prior-art.md` (prior art,
  403 lines, agent ID `ace0dcc89e4a3a59e`).

Where the four reports agree (validated by parallax, not by a single
agent's argument):

- **Path B (own `leaven-acp`) is correct.** Inventory report risk #4: SDK
  vendor-and-prune isn't viable because the actor design assumes
  RoleId/HasPeer/ProtocolCompat. Multi-language report: Path B keeps
  schema-codegen uniform across languages instead of inheriting upstream
  SDK quirks. Prior-art report: MCP/LSP/ACP SDK churn is a documented
  adoption-pain pattern; the schema-locked seam inverts it. Three reports
  reach Path B from independent angles.
- **Schema-locked wire is the load-bearing safety net.** Multi-language
  agent: the wire uses pure JSON Schema 2020-12 in the boring subset
  (kind-discriminated oneOfs, no advanced features) — codegen-friendly,
  multi-language-safe. Prior-art agent: this directly inverts MCP's
  schemaless-drift failure mode (the most-cited MCP pain in their
  research). Inventory agent: the seam owns wire validation, so transport
  layer can stay thin.
- **The DSPy drop-in shape is achievable in ~30 lines** if `leaven-lm`
  carries both `prompt`-or-`messages`, full kwarg pass-through, multi-modal
  content, and an OpenAI-chat-shaped response. Prior-art agent enumerated
  exactly what to accommodate (section 6 of their report).

Where the reports diverge or surface tension:

- **Scope of Python SDK value.** EvoSkill agent: SDK helps evaluator/
  proposer authoring (~1,500-2,000 of 2,400 lines in `p5_evoskill_iteration`
  disappear) but does NOT touch the 9,351-line paper-close bring-up harness
  (`p5_skill_paper_reproductions/src/evoskill.rs`). The user's correction
  above broadens scope to the whole product surface, which puts some of
  that paper-close bring-up code in scope (manifest construction, source
  pinning) but the 280 internal validators are still domain logic that
  has to live somewhere. **The Python SDK does not eliminate paper-close
  bring-up; it relocates the parts that benefit from typed records and
  ergonomic composition, and leaves the parts that are inherently domain
  work where they live.**
- **`@lv.optimizer` doesn't exist; `lv.optimizers.gepa(...)` does.** Per
  the user's correction. Honest stance: extend by writing a Rust optimizer
  crate, configure by writing Python. EvoSkill agent flagged this as open;
  the user's correction closed it. Carry into the spec.
- **Bidirectional spawning is implicit but unverified.** Inventory agent
  surveyed `leaven-acp` as "engine spawns worker for a stage" — but the
  Python SDK is the other direction: Python spawns engine. Same machinery
  should serve both (the spawned side just hosts the same JSON-RPC
  dispatch), but no report explicitly tested this.

LOC discrepancy worth flagging:

- Inventory agent reported 327 LOC src + 943 LOC tests for the existing
  `crates/leaven-acp` blocking prototype.
- Filesystem read on 2026-05-24 showed 450 LOC src + 1,171 LOC tests.
- ~350 LOC delta. Cause unclear (counting method, file growth between
  reads, or measurement error on one side). The architectural conclusion
  ("substantial existing scaffolding") is unaffected; the day-count
  estimate (6-8 calendar days) does not move with this delta but the
  number is therefore not down to the last day.

The "ground I'm standing on" check, per UNDERSTANDING.md:

The synthesis above describes an architecture (one binary + leaven-acp
crate + Python SDK shipping the binary + schema-codegen for typed records
+ adapter namespaces for DSPy) that has been validated by four
independent research angles. None of the angles reported "this won't
work." But all four are research reports, not implementation proofs.
The actual ground here is:

- Locked spec at `docs/specs/public-seam-v1/` (real).
- Existing `crates/leaven-acp` blocking prototype with passing tests
  (real, verified by filesystem read).
- Existing `crates/leaven-public-seam` with 32/39 conformance rows proven
  (real, verified by conformance-matrix.yaml).
- AGENTS.md topology committed during this conversation (real).

What is NOT ground:

- The Python SDK ergonomic surface (sketched in this note, not implemented).
- The schema codegen pipeline (sketched, not implemented).
- The 6-8 day timeline for the async ACP rewrite (estimate, not measured).
- The "200-line Python paper repro" target (target, not demonstrated).

Does the ground serve what this is for? Yes — the alignment checkpoint
and handoff artifact will be a goal against the implementation work that
sits on top of the proven ground, and the proven ground is sufficient
to set such a goal. The synthesis is not claiming the architecture works;
it's claiming the architecture is worth building from where we are.

What the alignment checkpoint should surface to the user:

1. Confirm scope: Python SDK = whole product surface (already corrected,
   needs re-confirmation in artifact form).
2. Confirm `@lv.optimizer` is out of scope (configure existing Rust
   optimizers, don't author new ones from Python).
3. Confirm the first external-language worker fixture choice (bash vs
   Rust test binary vs Python actual SDK). Recommend: Python, because
   that's the proof-of-life that matters; defer bash/Rust fixtures to
   internal testing.
4. Confirm Path B is the right call for `leaven-acp` (write in-house,
   no upstream SDK dependency).
5. Confirm the 6-8 calendar day estimate is honest and the work order
   (async dispatch → cancellation → bounded queue → state machine →
   authenticate → tests) is sensible.
6. Surface the LOC discrepancy and what it means for trust in the
   estimate.

## Next Actions

1. Wait for the four research files to land under
   `docs/working-memory/leaven-py-research/`.
2. Incorporate findings into a "Decisions" section of this note.
3. Run the goal-handoff alignment checkpoint with the user (per
   `leaven-goal-handoff` skill).
4. Write the implementation plan to `docs/plans/2026-05-24-leaven-py-and-acp-transport.md`.
5. Set the durable goal with the handoff artifact attached.

## Conversation Provenance

This research note was created during a 2026-05-24 design conversation that
walked through the public seam v1 maturity (32/39 conformance rows proven),
the blocked ACP transport rows, the comprehensive design pass rejection of
pyo3, the bridge architecture that landed on Path B (own-the-transport), and
the AGENTS.md updates that committed `leaven-acp` to the topology. The
conversation also surfaced that stage authoring (not just evaluator
authoring) is first-class for Python users per the locked stage payloads
spec (`docs/specs/public-seam-v1/04_stage_payloads_spec_v0.3.md`).
