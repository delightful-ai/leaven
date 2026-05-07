# Agentic Optimization Literature Journal

Date: 2026-05-07

## Objective

Use recent literature to pressure-test the Leaven v0.2.2 spec before cutting
options. The question is not "can we make the surface smaller?" in the abstract.
It is "which seams are genuinely load-bearing for agentic optimizers that learn
skills, memories, and reusable behavior?"

## Working Mode

This journal is the primary coherence artifact. `tmp/` is scratch for paperclip
search dumps, paper summaries, and possible Flywheel payloads. Flywheel should
mirror or organize the map once the research has enough structure; it should not
replace the running notes too early.

## Local Constraints Read First

- `docs/specs/guiding_principles.md` frames Leaven as a Rust library for
  optimizing arbitrary artifacts, including agent harnesses, skill kits,
  directories, git commits, and structured records.
- The meta-constraint is model-legibility: named traits should map directly to
  literature concepts. If a model thinks "selection is a `CandidateSelector`,"
  that should be true.
- Hard requirements include artifact-shape neutrality, render/materialize
  separation, evidence-shape neutrality, strategy swappability, stage neutrality,
  trust separation for agentic stages, and explicit budget bookkeeping.
- The current v0.2.2 direction already says: selection policy is swappable,
  population/archive state is separate, and workspace materialization bridges
  typed skill artifacts to agent runtimes.

## Search Plan

Focus recent papers after October 2025, with extra weight on February-May 2026:

- agentic optimization / self-improving agents
- skill learning, skill libraries, skill routing, hard-case selection
- procedural memory and memory editing for LLM agents
- meta-learning or recursive optimization for agents
- ablations that say which components matter

For each paper, capture:

- what state is learned
- what selection/routing/admission mechanism exists
- whether workspace/filesystem/materialization is needed
- whether evidence is scalar, casewise, pairwise/listwise, or attributable
- what Leaven primitive it validates or contradicts

## Running Notes

- Initial local read pushes against cutting selection. The product constraints
  name selection as one of the fixed loop slots, and recent skill-memory papers
  are likely to make routing/admission/target selection even more important,
  not less.
- Flywheel is useful for the final literature graph, but the journal is better
  for maintaining research coherence while discovery is still fluid.

## Paperclip Search Log

- `s_c2f3f0bb`: broad abstract search for "LLM agent skill learning skill
  library self improving agents" over six months. Too noisy; mostly domain
  "agentic AI" papers and surveys.
- `s_d143f190`: broad memory search. Some useful landmarks, especially
  lifelong-learning roadmap, memory fabric, structured memory eval, and
  A-MAC-style memory admission, but many false positives about hardware or
  human procedural memory.
- `s_f3c7f972`: agentic optimization/meta-controller search. Useful for survey
  context, less directly useful for skill-learning mechanisms.
- `s_01f59321`: hard-case/skill/memory search. Still noisy, but surfaces the
  same reliable landmarks.
- `s_8cecf749`: named EvoSkill search. High signal: EvoSkills, EvoSkill,
  SkillRouter, Memento-Skills, skills-in-the-wild benchmark, MemSkill, SKILL0,
  SoK Agentic Skills, SkillReducer, skill-file attacks.
- `s_2bf99a41`: named MemSkill search. High signal: MemSkill, Externalization
  review, Memento-Skills, SkillRouter, skills-in-the-wild, StructMemEval, SoK,
  SkillReducer, A-MAC, skill-file attacks.
- `s_416d09ed`: named Memento/skills search. High signal: Memento-Skills,
  CUA-Skill, skills-in-the-wild, SoK, SkillRouter, Graph of Skills,
  SkillFoundry, SkillInject, large-library skill-selection failure, SkillX.
- `s_d825392e`: automatic skill-learning search. Best current working set:
  SkillX, Memento-Skills, SkillFoundry, SkillRL, CUA-Skill, skills-in-the-wild,
  SoK, SkillRouter, Graph of Skills, MemSkill, SkillInject, large-library
  selection failure, SKILL0, SkillReducer, EffiSkill.

## Current Paper Set

High-priority mechanism papers:

- EvoSkills / EvoSkill
- MemSkill
- Memento-Skills
- SkillRL
- SkillX
- SkillFoundry
- SkillRouter
- Graph of Skills
- How Well Do Agentic Skills Work in the Wild
- SkillReducer
- Adaptive Memory Admission Control
- CUA-Skill
- SkillInject / When Skills Lie

Initial read: this cluster does not support cutting selector/admission seams.
It suggests the opposite: the agentic frontier is increasingly about routing,
retrieval, verification, compression, admission, and rollback around an external
skill/memory substrate.

## Mechanism Notes

### MemSkill — memory operations as evolvable skills

Paperclip ID: `arx_2602.02474`

Facts:

- Learned state is split into two stores: trace-specific memory banks and a
  shared skill bank of reusable memory-construction skills.
- Skill selection is a learned controller: given a current text span and
  retrieved memories, it selects an ordered Top-K subset from a variable-size
  skill bank.
- Skill evolution is designer-driven: recent hard cases are stored in a sliding
  buffer, clustered for diversity, ranked by difficulty, and used to refine
  existing skills or add new ones.
- Admission/rollback exists: MemSkill keeps best-performing skill-bank snapshots
  and rolls back if a designer update degrades stabilized reward.
- Ablations are directly relevant: random skill selection and disabling the
  designer both degrade performance. That is the strongest evidence so far that
  selector and updater/admission seams are not optional.

Leaven implication:

- `SkillRouter`/selector, hard-case selector, skill target selector, and
  admission/rollback policy are real seams for skill-memory systems. They do
  not belong in cold core today, but the current `CandidateSelector` /
  `Population` split is aligned with the literature.

### Memento-Skills — executable skill folders as external memory

Paperclip ID: `arx_2603.18743`

Facts:

- Learned state is an external skill memory: structured markdown files plus
  prompts/code/helper artefacts, not model weights.
- Read phase uses a behavior-trained skill router. Write phase updates and
  expands the skill library from experience.
- Failed attempts use a failure-attribution selector over full execution traces
  and judge rationale to identify the skill most responsible for an error.
- In-place patch vs new-skill discovery is governed by utility thresholds and
  sample counts; mutations are guarded by a unit-test gate with rollback.
- The paper explicitly contrasts behavior-aligned routing with semantic
  similarity; retrieval quality is not just embedding search.

Leaven implication:

- `Materializer` is non-negotiable for agentic skill learning: the optimized
  unit is a folder with multiple files, not a rendered string. `AttributableEvidence`
  over skill IDs is also justified by the failure-attribution step.

### A-MAC — memory admission as a structured control problem

Paperclip ID: `arx_2603.04549`

Facts:

- Memory admission is treated as a first-class decision: admit, update, or
  reject candidate memories before they enter long-term storage.
- The policy scores candidate memories on utility, confidence, novelty, recency,
  and type prior, then applies learned weights and a threshold.
- Main result claims better precision-recall and lower latency than LLM-native
  memory systems; ablations identify content type prior as the strongest factor.
- Important design point: admission is deliberately inspectable and auditable,
  not an opaque side-effect of a memory generation step.

Leaven implication:

- For Leaven, memory/skill admission should be modeled as a strategy/policy
  around an archive/store, not buried inside the evaluator or agent runtime.
  This supports keeping archive/population state separate from policy.

### EvoSkill — failure-driven evolution of filesystem skills

Paperclip ID: `arx_2603.02766`

Facts:

- Learned state is a git-backed frontier of "agent programs", where the delta
  between programs is skill folders plus agent metadata.
- Proposal is split across an executor, proposer, and skill-builder. The
  proposer analyzes failed cases and decides create-vs-edit; the skill-builder
  materializes a concrete skill folder.
- Parent selection is simple round-robin over a fixed-capacity frontier, but it
  is still explicit. Admission compares held-out validation score against the
  weakest frontier member.
- The paper uses train/validation/test separation: training failures produce
  learning signal, validation drives frontier selection, and test is held out.
- Environment representation is explicitly git/workspace shaped: each program
  is a branch/config snapshot, frontier members are tagged, and losing children
  may be discarded to avoid repo bloat.
- Skill-merge across independent runs outperformed individual runs, suggesting
  complementary failure-mode coverage and some need for merge/admission
  semantics beyond single-lineage mutation.

Leaven implication:

- This is almost a direct validation of `Materializer`, `ProposalEffect::Change`
  over skill-directory artifacts, validation-scoped `Population`, and explicit
  candidate selection. It also argues for preserving merge/crossover as a
  proposer pattern, not an engine special case.

### SkillRouter — retrieval as a high-leverage skill-library bottleneck

Paperclip ID: `arx_2603.22455`

Facts:

- The paper frames large-scale skill routing as an upstream bottleneck: if the
  wrong skill shortlist is surfaced, downstream planning often cannot recover.
- It specifically tests large, overlapping registries: roughly 80K skills, many
  topically plausible distractors, and both single-skill and multi-skill cases.
- Full skill body access is central for routing; hiding implementation bodies
  caused large Hit@1 drops across sparse, dense, and reranking baselines.
- The proposed route is a two-stage full-text retrieve-and-rerank pipeline. The
  important algorithmic pieces are false-negative filtering for near-duplicate
  skills and listwise reranking for fine-grained candidate competition.
- End-to-end agent task success improves when routing improves, but top-10
  gains are not the same as full gold-set recovery.

Leaven implication:

- Retrieval itself is not necessarily Leaven core, but the framework cannot
  assume skills are represented only by short metadata. Agent-facing
  materialization and routing-facing rendering may need different views of the
  same skill artifact. This reinforces renderer/materializer split and argues
  against hiding full artifacts behind a thin description-only API.

### Retrieval/compression/security lane — useful, but probably not optimizer core

Paperclip searches:

- `s_c019a9e7`: Graph of Skills, skills-in-the-wild, SkillTester, secure skill
  architecture, SkillProbe, SkillRouter, SkillInject, SkillReducer.
- `s_b7c8016d`: SkillInject, malicious skills in the wild, hidden-comment
  injection, repository-context classification, SkillSieve, SoK, SkillProbe.

Current judgment:

- These papers matter for Leaven, but mostly as constraints on artifact
  representation, trust boundaries, and admission/audit hooks.
- I do **not** yet see evidence that skill retrieval, skill compression, or
  skill security should become optimizer-core traits. They look like
  `leaven-agentic`/`leaven-skill` utilities or policies around an agent skill
  library.
- The exception: if a skill optimizer mutates compressed skills or security
  labels directly, then compression/security becomes a domain-specific
  evaluator/admission policy, not a cold-core concept.

### ProcMEM — procedural memory as executable skill pool

Paperclip ID: `arx_2602.01869`

Facts:

- Learned state is a pool of procedural skills, not conversational facts.
  Skills are extracted from trajectories and represented as executable
  procedures with activation, execution, and termination structure.
- The paper frames the optimization problem as a Skill-MDP with a dynamic skill
  pool. It keeps skill selection fixed and focuses the contribution on evolving
  the skill pool itself.
- Updates use "semantic gradients" from hindsight attribution: failed or
  low-return trajectories are reflected into skill edits, then verified through
  a PPO-like gate before admission.
- Pool maintenance is score-based. Skills with non-positive or redundant
  contribution are pruned; FIFO maintenance is an explicit ablation and
  performs badly in long-horizon settings.
- Ablations remove the skill representation, semantic gradient, PPO gate, and
  score-based maintenance; all matter. This is strong evidence that admission
  and maintenance policy are not incidental.

Leaven implication:

- This supports `AttributableEvidence<S::PartId>`/semantic-gradient-style
  routing, `Gate`/validation as a real optimizer policy, and score-aware
  `Population` maintenance. It does **not** push pairwise evidence into P1; the
  evidence shape here is mostly scalar return plus trace-derived attribution.

### ALMA — memory design itself as optimizable code

Paperclip ID: `arx_2602.07755`

Facts:

- Learned state is a memory design expressed as executable code: storage
  representation, retrieval, update, optional databases, and composition among
  submodules.
- The core interfaces exposed to the agentic system are `general_update()` and
  `general_retrieve()`, but internally the learned memory can contain multiple
  modules and databases.
- The meta-agent samples previous designs from an archive, reflects on code and
  evaluation logs, proposes a plan, implements and debugs code, evaluates the
  candidate in an agentic system, then appends the result and logs back into
  the archive.
- Sampling is archive-based and exploration-weighted: probability is roughly
  proportional to success and inversely proportional to sample count, with
  non-zero probability for all designs.
- Ablation against greedy search favors open-ended exploration. Moderate
  stepping-stone designs can become ancestors of the best memory design.
- Safety is not optional: generated memory code is validated and run in isolated
  sandboxes, with access confined to the sandbox and human inspection for
  harmful behavior.

Leaven implication:

- This is the strongest reason not to bake a fixed "memory trait" into cold
  core. Memory architecture can itself be the artifact. The Leaven substrate
  should optimize arbitrary memory-design code via workspace materialization,
  evaluator logs, archive/population policy, and trust-limited execution.

### Trajectory-Informed Memory — structured tip generation from traces

Paperclip ID: `arx_2603.10600`

Facts:

- Learned state is structured memory tips extracted from agent trajectories:
  strategy tips, recovery tips, and optimization tips.
- The system explicitly performs trajectory intelligence extraction, decision
  attribution, contextual tip generation, storage/consolidation, and runtime
  retrieval.
- Tips carry provenance: source trajectory ID, source outcome, category,
  priority, trigger condition, implementation steps, and optional negative
  example.
- Extraction granularity matters. Subtask-level tips improve task goal
  completion, while LLM-guided retrieval improves scenario consistency.
- Memory management includes generalization, semantic clustering, and LLM-based
  consolidation to avoid unbounded duplicate tips.
- The retrieval strategy itself is an experimental axis: cosine retrieval is
  cheap, LLM-guided selection is more expensive but better at consistency.

Leaven implication:

- This validates evidence splitting and provenance-heavy evidence. A useful
  agentic evidence type probably needs both raw trajectory handles and derived
  structured attribution/tips. It also argues for typed retrieval/admission
  policies outside optimizer core, because the right retrieval strategy is
  workload-dependent.

### Memory Probe — retrieval often beats write-time sophistication

Paperclip ID: `arx_2603.02473`

Facts:

- Controlled 3x3 study crosses three write strategies (raw chunks, fact
  extraction, summarization) with three retrieval strategies (cosine, BM25,
  hybrid reranking).
- Retrieval method dominates in their LoCoMo setup: accuracy varies by about
  20 points across retrieval methods but only 3-8 points across write
  strategies.
- Raw chunk storage matches or beats lossy fact extraction/summarization under
  stronger retrieval, suggesting compression can throw away useful context.
- Failure analysis attributes most errors to retrieval-stage failure; utilization
  failures stay relatively stable when relevant memory is surfaced.
- Limitation: one benchmark, one model, fixed retrieval budget; the result is
  a warning against overgeneralizing, not a universal theorem.

Leaven implication:

- Do **not** centralize compression as a Leaven core primitive. Preserve raw
  evidence/artifact access, allow multiple render/retrieval views, and make
  compression an evaluated, gated transformation. Retrieval policy is more
  likely to be the product-facing seam than a generic "compress" trait.

### ERL — heuristic memory and selective retrieval

Paperclip ID: `arx_2603.24639`

Facts:

- Learned state is a persistent pool of structured heuristics generated from
  single-attempt trajectories plus binary outcome feedback.
- Runtime uses selective retrieval: an LLM scores stored heuristics for task
  relevance, diversity, and informativeness, then injects the top-k into the
  agent prompt.
- Heuristics outperform raw few-shot trajectories under controlled token
  budgets, and LLM retrieval beats both random selection and embedding-only
  retrieval.
- The number of heuristics is non-monotonic: more context eventually hurts, so
  selection quality matters more than stuffing the context.
- Source outcome matters. Failure-derived heuristics help Search more; success
  heuristics help Execution more. A practical system probably wants both and
  lets task/context selection decide.
- Iterative online ERL improved source performance but hurt held-out
  generalization, likely because guided trajectories narrow the failure modes
  seen during learning.

Leaven implication:

- This is another direct argument for swappable `CandidateSelector`/retrieval
  policy and separate admission/maintenance. It also suggests evidence should
  preserve source-outcome labels and routeable categories, not only aggregate
  scores.

### SkillInject — skill security is a trust/authorization boundary

Paperclip ID: `arx_2602.20156`

Facts:

- Skill files create an instruction supply-chain attack surface: the file is
  made of instructions, so standard "instruction vs data" defenses do not fit
  cleanly.
- The benchmark covers skill-file injections across document, ML, payment, and
  healthcare skills, with obvious and contextual attacks.
- Contextual security is the hard case: the same action can be legitimate or
  malicious depending on task, data sensitivity, destination, and policy.
- Safety-policy prompting helps but is insufficient. LLM skill screening detects
  many injected skills but struggles with contextually authorized cases.
- Script-based attacks are worse than direct text injections because agents
  often trust auxiliary scripts without inspecting them.
- The recommendation is explicit policy, least-privilege capability binding,
  and context-aware authorization for external side effects.

Leaven implication:

- Security should not be represented as a boolean marker trait like
  `SafeSkill`. The useful boundary is capability/trust policy at workspace and
  materialization time: what files/scripts/tools/network destinations can this
  candidate access, and which actions require authorization or evaluator review?

### SkillReducer — compression as evaluated restructuring

Paperclip ID: `arx_2603.29919`

Facts:

- Compression target is the skill artifact itself: route description, main
  body, references, and progressive-disclosure structure.
- The system compresses routing descriptions via delta debugging plus actual
  agent-trigger validation, then restructures bodies into always-loaded core
  rules and on-demand reference modules.
- Functional retention is gated. Faithfulness checks catch missing operational
  concepts, and task-based evaluation promotes mistakenly-deferred content back
  into the core.
- This is not generic prompt compression. It depends on skill structure:
  routing layer vs body, core rule vs background/example/template, and
  read-file access to deferred references.
- The reported failure mode is important: examples can act as specification.
  Compressing or deferring examples can silently remove behavior unless tests
  catch it.

Leaven implication:

- Compression belongs as a materializer/renderer/evaluator workflow around a
  concrete skill artifact, not as a cold-core trait. If Leaven supports this
  well, a SkillReducer-like optimizer is just another optimizer over
  `SkillDir` with validation gates and workspace execution.

## Current Design Pressure

Things that look more important after the literature pass:

- Keep candidate selection swappable. GEPA, MemSkill, ALMA, ERL, and
  SkillRouter-style systems all make selection/retrieval/routing load-bearing.
- Keep archive/population state separate from policy. Papers repeatedly separate
  "what is stored" from "which item is sampled, retrieved, admitted, pruned, or
  rolled back."
- Treat `Materializer` as central for agentic use. Skills and memory designs
  are folders/code/config, not just strings.
- Keep raw artifacts/evidence accessible. Compression and summarization are
  lossy until proven otherwise.
- Model retrieval/compression/security as agentic-library policies and
  evaluator workflows first, not optimizer-core traits.
- Add explicit trust/capability language around skill materialization: skill
  files and scripts are untrusted until bound to a policy.
- Add a first-class "provenance + derived evidence" pattern. Many systems need
  raw traces plus extracted attributions/tips/heuristics with source case,
  source outcome, trigger, priority, and confidence.

Things that still look underspecified for implementation:

- A reusable `SkillDir` artifact shape and `SkillDirSurface`/path surface:
  enough for examples, but not necessarily in cold core.
- A materialization API that copies/renders typed artifacts into local and
  remote workspaces without assuming `Path`.
- A trust table that says which stage can read raw train data, validation data,
  hidden partitions, skill files, scripts, network, and workspace outputs.
- Evidence split: raw evaluation record vs derived evidence. The current
  `AttributableEvidence<K>` direction is good, but examples need a concrete
  evidence shape that implements multiple K views without clone-heavy APIs.
- Skill/memory admission policies: add/update/reject/rollback are common enough
  to deserve standard vocabulary, even if not cold core.
- Retrieval policy vocabulary: not a core trait, but `leaven-agentic` probably
  wants standard interfaces for route descriptions, full bodies, candidate
  pools, and rerank evidence.
- Compression should be specified as an optimizer/evaluator pattern with gates,
  not a blanket `Compressible` marker.
- Security status should not be a marker trait. It needs provenance, trust
  source, capability set, sandbox/network policy, and audit/evaluator outcome.

## Tentative Design Bets To Test

- Keep `CandidateSelector` swappable.
- Keep `Population`/archive state separate from selection policy.
- Keep `Materializer` in scope for agentic stages; do not collapse it back into
  text rendering.
- Avoid adding cold-core skill traits until a concrete `leaven-skill` or
  agentic optimizer crate needs them.
- Look for evidence that pairwise/listwise/fitted preference machinery is real
  implementation pressure rather than speculative generality.

## Open Questions

- Do recent skill-library systems fit naturally as GEPA strategy variants, or do
  they need a separate optimizer rhythm?
- Are hard-case buffers and skill admission policies population concerns,
  selector concerns, or their own strategy slots?
- Does procedural memory require a distinct artifact/evidence shape, or is it
  just another workspace-materialized artifact with attribution?
- Which of the current expressibility targets can be moved out of P1 without
  losing the ability to implement agentic skill learning?
