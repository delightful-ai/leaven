# Agentic Optimization SOTA, 2026

My read: agentic optimization has shifted from "make the prompt better" to
"build an external learning substrate around the agent." The interesting unit is
increasingly not a single prompt, but a library of skills, memories, heuristics,
execution traces, tests, and routing/admission policies.

  The strongest pattern across MemSkill, Memento-Skills, EvoSkill, ProcMEM, ERL,
  ALMA, SkillRouter, SkillReducer, and the April skill papers is this:

  Agents learn when experience is externalized into reusable artifacts, but reuse
  is the actual hard problem.

  Not generation. Not reflection alone. Reuse.

  A rough field map:

  1. Experience -> abstraction
     Papers are converging on the idea that raw trajectories are too bulky and too
     situation-specific. Useful systems distill them into something like
     heuristics, skills, procedures, tips, memory designs, or structured knowledge.
     ERL (arx_2603.24639) is very clear here: heuristics beat raw trajectories.
     Trajectory-informed memory (arx_2603.10600) says the same with more taxonomy:
     strategy, recovery, optimization tips.
  2. Abstraction -> selection
     The next bottleneck is selecting the right reusable thing at the right time.
     SkillRouter (arx_2603.22455), MemSkill (arx_2602.02474), ERL, Graph of Skills
     (arx_2604.05333), and SkillX (arx_2604.04804) all point here. A giant skill/
     memory library is not intelligence. Without strong routing, it becomes context
     bloat or wrong-context poisoning.
  3. Selection -> admission/maintenance
     The field is realizing libraries rot. Skills conflict, duplicate, overfit,
     drift, or become obsolete. MemSkill has rollback, ProcMEM (arx_2602.01869) has
     score-based pruning, A-MAC (arx_2603.04549) makes memory admission explicit,
     SkillReducer (arx_2603.29919) gates compression with tests. The scientific
     shape is closer to database/index maintenance plus evolutionary search than to
     one-shot prompting.
  4. Learning target is expanding
     ALMA (arx_2602.07755) is especially interesting because it does not optimize
     memories. It optimizes memory designs as code: schema, update, retrieve,
     submodules. That is a jump. It says the "learning system" itself can be an
     artifact under search. SkillFoundry (arx_2604.03964) points similarly for
     scientific skills: not just instructions, but contracts, environment
     assumptions, provenance, tests.
  5. Failure is now a first-class signal
     A lot of these systems learn more from failures than successes. ERL found
     failure heuristics better for Search, success heuristics better for Execution.
     Memento-Skills and EvoSkill use failure attribution to decide what to patch.
     This feels scientifically real: failures localize missing constraints;
     successes mostly reinforce procedures.
  6. Self-feedback is not enough
     The newer benchmark notes are sobering. SkillLearnBench (arx_2604.20087) says
     self-feedback alone can drift. ERL's iterative variant overfit source
     trajectories and generalized worse. This suggests "closed-loop self-
     improvement" needs external anchors: tests, held-out cases, human/teacher
     feedback, deterministic verifiers, or strong environment rewards.
  7. Compression is not one thing
     There are two conflicting results that are both useful. Memory Probe
     (arx_2603.02473) says lossy memory writes can underperform raw chunks when
     retrieval is good. SkillReducer says structure-aware skill compression can
     improve performance by removing distractors. So "compress" is the wrong
     scientific category. The real distinction is lossy summarization vs structure-
     preserving refactoring with behavioral gates.
  8. Security is structural, not a scanner problem
     SkillInject (arx_2602.20156) makes this pretty clear. Skill files are
     instructions, so "separate instructions from data" does not solve it. The
     right frame is authorization: what can this skill cause the agent to do, with
     what data, under what policy? Skills are becoming a supply chain.
  9. The field lacks good long-horizon evaluation
     The April benchmarks are all trying to fix this: realistic retrieval,
     continual repair, library maintenance, wild skill usage. The core problem is
     that short tasks make skills look better than they are. Long-horizon,
     sequential, held-out, changing distributions expose drift and routing
     failures.

  My higher-level take: this area is becoming agentic continual learning without
  weight updates. The learned state lives outside the model, and the hard science
  is how to turn experience into durable, retrieveable, inspectable, and safe
  operational knowledge.

  The wonky part: many papers still conflate several different things under "skill"
  or "memory." A skill can be a prompt snippet, a procedure, a script folder, a
  retrievable document, a policy, a tool wrapper, or a mini-agent. A memory can be
  a trace, fact, heuristic, learned retrieval program, or full memory architecture.
  That ambiguity is probably the biggest conceptual drag on the field right now.

  The most interesting frontier, to me, is not "better reflective prompting." It
  is:

  - learned routing over large evolving libraries
  - admission/rollback under noisy rewards
  - failure attribution to the right reusable unit
  - structured skill representations beyond markdown
  - secure capability binding for third-party skills
  - external feedback and tests to prevent recursive drift
  - open-ended search over the learning substrate itself

  For Leaven-adjacent purposes, the punchline is: do not over-center GEPA, but also
  do not throw away GEPA-like evolutionary structure. The frontier looks like many
  GEPA-ish loops wrapped around richer artifacts: skill folders, memory programs,
  retrieval policies, compressed skill variants, and agent harnesses.

## Five skill-optimization reproductions to pressure-test Leaven

Pick papers that make different parts of the skill-learning loop load-bearing,
not just five variants of reflective mutation:

1. EvoSkill (`arx_2603.02766`) - skill folders, failure analysis,
   create-vs-edit, held-out validation, frontier admission.
2. Trace2Skill (`arx_2603.25158`) - many-trace analysis, parallel lesson
   extraction, hierarchical consolidation into a transferable skill directory.
3. Memento-Skills (`arx_2603.18743`) - continual read/write skill library,
   stateful prompts, skill routing, failure attribution, test-gated rollback.
4. D2Skill (`arx_2603.28716`) - task-level and step-level skill banks, paired
   baseline vs skill-injected rollouts, utility-aware update/retrieval/pruning.
5. SkillReducer (`arx_2603.29919`) - structure-preserving skill compression,
   progressive disclosure, routing-description minimization, behavioral gates.

Use SkillRouter (`arx_2603.22455`) and SkillInject (`arx_2602.20156`) as shared
assays across these reproductions rather than as the first five optimizer
targets: retrieval quality and capability/security boundaries should be tested
against every learned skill library.
