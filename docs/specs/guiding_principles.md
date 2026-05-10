# optimize anything in rust — requirements

*a specification for what we are trying to build, not how to build it.*

source of truth as of 2026-05-05. supersedes the v1.0 design lock for the purpose of agreeing on the problem before re-attempting solution. solution-shape (traits, types, signatures) is deliberately out of scope here.

---

## 0. preamble

this document captures the problem we are trying to solve, the constraints that any acceptable solution must satisfy, the constraints we *expect* most acceptable solutions to satisfy, and the qualitative properties — the taste, the feel, the subtle pressures — that should shape the design. it is the input to the next attempt at a solution; it is not itself a design.

the audience for this document is competent language models and their human collaborators working on the rust port. the document optimizes for the model reading it being able to:
1. produce correct designs that satisfy the constraints
2. detect when a proposed design *fails* a constraint
3. recognize when a constraint is in tension with another and reason about the tradeoff

this is not a marketing document, a paper, or a how-to. it is the load-bearing requirements substrate.

---

## 1. the problem

### 1.1 what we are building

a rust library for optimizing arbitrary artifacts whose performance can be measured. the artifact can be text, a structured value, a directory tree, a git commit, a piece of code, an agent harness, a configuration, a specification — anything with identity and a way to produce a modified version of itself. measurement can be cardinal (scalar or multi-axis numeric), ordinal (pairwise or listwise preference), or mixed.

the library provides a generic optimization loop and a small set of composable primitives. it does not make decisions on the user's behalf about what to optimize, how to evaluate, what feedback shape is best, what the "right" search policy is, or how to render anything for any consumer. it provides the substrate; the user provides domain knowledge.

### 1.2 why a new library

the existing solution space — primarily python gepa and optimize_anything — has produced impressive results across a wide variety of domains (gskill, arc-agi agent evolution, cuda kernel generation, cloud scheduling policy discovery, prompt optimization, blackbox numerical optimization). these results validate the *core idea*: reflective evolutionary search over text artifacts with rich diagnostic feedback is a general optimization paradigm. they do not validate the *specific abstractions* python gepa happens to use. several of those abstractions are calcifications of the era they were designed in:

candidates are dict[str, str]. this is the universal interchange medium and works for any text-shaped artifact, but it requires the framework or adapter to flatten any non-text structure (filesystems, code repos, large traces) into strings, which is lossy and forces premature decisions about what to keep.
the adapter contract requires producing a Mapping[str, Sequence[Mapping[str, Any]]] reflective dataset that the proposer reads. this assumes the proposer is a one-shot LM call that reads pre-rendered context. it cannot accommodate proposers that are themselves agents and want to retrieve selectively.
evidence is scalar (or multi-axis cardinal via objective_scores). pairwise, listwise, or otherwise non-cardinal evidence shapes — increasingly important for preference learning, llm-as-judge tournaments, continual learning via comparison — must be faked through scores.
the strategy override points are real but not orthogonal. the adapter has a propose_new_texts override, the engine accepts a custom_candidate_proposer, the candidate selector is one of four hardcoded literals, frontier shape is one of four hardcoded literals. extending in a direction the maintainers did not anticipate generally means forking.
these are not unfixable bugs in python gepa. they are calcifications of one particular reading of the problem. the rust port is an opportunity to start from a sharper reading.

### 1.3 the actual class of problem

every optimizer in the literature we care about — gepa, mipro, textgrad, trace/optoprime, optimas, muf/edit, map-elites for prompts, c-evolve, moprompt, gskill, memskill, skillfoundry, evosskills, graph-of-skills, memento-skills, vista, tep, pareto-lenient consensus, combee, alphaevolve, openevolve, shinkaevolve — is the same skeleton with different choices:

loop:
  pick something to mutate from   (selection)
  produce candidate(s)             (proposal, possibly preceded by reflection)
  measure them                     (evaluation)
  decide what to keep              (acceptance / frontier / archive)
  decide whether to stop           (stopping)
the variation is entirely in the choices: what gets compared to what, what evidence the comparison produces, what the proposer sees and how, whether a frontier is maintained and what shape, what counts as "better." the skeleton is fixed. our job is to find the small set of orthogonal primitives that make every choice in the literature expressible without rewriting the engine, while remaining legible enough that a competent model can map a new paper's algorithm onto them in one shot.

---

## 2. the meta-constraint: simplicity for models

this is the load-bearing constraint and deserves to be stated first because every other constraint trades against it.

the simplicity target is **legibility to competent language models**, not human cognitive load on first read. these are different optimization functions and they pull in different directions in specific cases.

### 2.1 what simplicity-for-models implies

**explicit trait/protocol surfaces over duck typing.** a model can hold a small number of named traits with precise contracts in context and reason about which one to implement to achieve a goal. it cannot reliably guess which methods need to exist on an object based on context.
**conceptual names that map 1:1 to literature concepts.** if the literature calls something a "pareto frontier," the type should be named something like ParetoFrontier, not Archive. when a paper says "we use bradley-terry to convert preferences to scores," there should be a BradleyTerryRanker or equivalent. naming is the index into the library's capabilities.
**predictable factoring.** when a model thinks "to change GEPA parent selection, i implement the ParentSelector trait," it should be right. the override surface is enumerable and matches user intuition.
**small composable pieces over large generic ones.** seven well-factored protocols beat three god-protocols that conditionally do different things. context-fit matters.
**docstrings as contracts.** every trait method documents what it must do, what invariants it must maintain, what it can assume from callers. these are read at code-generation time.
**examples as pattern-matching fodder.** for each major optimizer in the literature, there should be a worked example showing the trait impls and configuration. a model writing a new optimizer pattern-matches against the closest worked example.
**idiomatic rust.** the constraint is "competent CS-PhD-level idiomatic rust." this is a higher bar than "accessible to junior devs" but a lower bar than "research code golf." async fn in traits, associated types, GAT-flavored generics, judicious dyn for runtime polymorphism where it actually helps — these are all on the table. unidiomatic things like macro-heavy DSLs or heavy unsafe are off the table unless explicitly justified.
### 2.2 what simplicity-for-models does NOT imply

it does *not* imply small surface area. a library with many small well-factored traits can be much simpler to a model than a library with few traits that secretly do many things.
it does *not* imply minimal generic parameters. types like Engine<A, P, S, AC, EP, ST> are fine if each type parameter has an obvious purpose and the model can fill them in from context. it is *non-obvious cognitive overhead* (e.g. lifetimes interacting with async, hidden coherence rules, surprising trait bounds) that hurts.
it does *not* imply "make it look like python." rust idioms, when they are clearer, are better even if they would be unfamiliar to a python user.
### 2.3 the success test

the operational definition of "simple enough for models":

given (a) the library's core types, traits, and a few worked examples, and (b) a natural-language description of an optimizer (or a paper PDF), a competent model should be able to produce a correct, idiomatic implementation in a single attempt without inventing new primitives, in a reasonable token budget.

if the model has to ask for new traits, the surface is missing primitives. if the model produces something that compiles but does the wrong thing, the contracts are unclear. if the model produces something verbose because composition required wiring six things together, the factoring is wrong. all three are failure modes we are trying to design against.

---

## 3. what must be true (hard requirements)

these are non-negotiable. a candidate design that fails any of these is rejected.

### 3.1 artifact-shape neutrality

the framework must not assume any specific shape for the artifact being optimized. it must accommodate at minimum:

a single string
a structured record of strings (ordered or unordered)
a directory tree (filesystem-backed, possibly content-addressed)
a git commit / tree / ref
a code module with associated tools or auxiliary files
a structured value combining any of the above (e.g. (prompt: String, skill_kit: GitRef, hyperparams: Config))
the only requirements the framework imposes on artifacts are:

**identity.** there must be some content-addressable or otherwise stable handle the framework can use to refer to a particular state of the artifact. cache keys, graph nodes, and lineage all depend on this.
**applicability.** there must be a way to take an artifact and a typed change-description and produce a new artifact (or fail explicitly).
every other property of the artifact (size, mutability, persistence, internal structure) is the user's concern and the framework treats it as opaque.

### 3.2 rendering separated from artifact

what an artifact (or trace, or lineage view, or any other piece of context) *is* must be separable from how it is *shown* to a particular consumer at a particular stage. multiple renderings of the same value must coexist.

a small string artifact rendered as inline text for a one-shot LM proposer.
a 50-file skill kit artifact rendered as a path to a workspace directory for an agentic proposer that will grep and read selectively.
a trace rendered as a summary for one consumer and as a raw json blob for another.
a candidate's lineage rendered as a list of (parent_diff, score) tuples for one proposer and as a navigable directory of past attempts for another.
the framework does not pre-render. it provides the artifact (or whatever) and a rendering mechanism; the consuming stage decides which rendering it wants. this is what unblocks both small-text cases and large-artifact-with-agentic-proposer cases through the same machinery.

### 3.3 evidence-shape neutrality

what evaluation produces is not assumed to be a number. the framework must natively express:

scalar evidence (the standard case)
multi-axis cardinal evidence (one number per named axis)
pairwise preference evidence (A beat B, possibly per-axis)
listwise ranking evidence (A > C > B > D, possibly per-axis)
mixed evidence (some axes cardinal, some pairwise)
the relation "is candidate X preferred to candidate Y" must be a *separate* concept built on top of evidence, not a property of the evidence type. cardinal pareto, bradley-terry, copeland, lexicographic, condorcet, and user-defined relations must all be expressible at the same call site where the framework asks "should X displace Y in the frontier?" or "is X improvement over Y on the minibatch?".

faking pairwise via scalar is unacceptable. a paper saying "we judge candidates by pairwise tournament" must be implementable directly, not via the user pretending tournament outcomes are numbers.

### 3.4 strategy swappability

every load-bearing decision in the loop must be a swappable trait implementation. the user must never have to fork the engine to change:

which parent (or parents) to mutate from
whether a frontier is maintained, what shape it has, and how membership is decided
what to evaluate when (full eval, minibatch, stratified, adaptive, streaming)
what counts as "improvement enough to keep" after a minibatch eval (acceptance / screening)
when to stop
how proposals are produced (one-shot LM, multi-stage pipeline, agentic, ensemble, merge, surrogate-driven, custom)
how reflection is performed, if it is performed at all
what feedback the proposer/reflector consumes (via rendering)
"no frontier" must be a valid configuration. "tournament instead of frontier" must be a valid configuration. "the user's own ParetoFrontier impl with a custom novelty bias" must be a valid configuration. the override is implementing a trait, not patching the engine.

### 3.5 mode neutrality

the three optimization modes from optimize_anything (single-task search, multi-task search, generalization) are different mental models, not just different argument shapes. all three must feel native:

**single-task search.** one hard problem, the candidate *is* the solution. no train/val. a frontier may not be wanted; "keep the best so far" is enough. sometimes there is no dataset at all, only the candidate and the evaluator.
**multi-task search.** a batch of related problems, with cross-transfer via shared state in the proposer or via shared frontiers. no held-out set; the goal is to solve the batch.
**generalization.** train/val asymmetry, the optimized artifact must work on unseen problems.
a single user-facing entry point should accommodate all three by what arguments are or aren't provided. the user should not feel like they are abusing the api when they pick the mode that fits their problem.

### 3.6 stage neutrality

a stage in the loop (evaluator, reflector, proposer) can be a one-shot LM call, a multi-stage typed pipeline, a deterministic algorithm, a full agent in a sandbox with tools, or anything the user implements. the framework must accommodate any of these at any stage, in any combination, without a special "agent mode."

the consequence: the framework cannot assume the stage is fast, deterministic, in-memory, side-effect-free, or token-bounded. it must provide bounding mechanisms (budget, timeouts, sandboxing hooks) that work uniformly.

### 3.7 expressibility of the literature

the design test is concrete: each of the following must be expressible in the library, with the implementation mapping cleanly onto user-facing primitives, without rewriting the engine:

gepa with default reflective mutation, instance-pareto frontier, frequency-weighted parent selection, minibatch acceptance, round-robin component selection
gepa+merge (with the merge proposer as a first-class optimizer, not a special engine state)
mipro / mipro v2 (bootstrap of grounded candidates, then bayesian acquisition)
textgrad (per-variable feedback aggregation, single greedy candidate)
trace / optoprime (subgraph-as-code reasoning over computation graphs)
muf/edit (typed two-stage diagnose-then-rewrite with structured claims and claim verification)
map-elites for prompts (niche-keyed frontier, uniform niche selection)
c-evolve (n-island parallel populations with periodic migration)
moprompt (multi-axis cardinal pareto)
gskill (evolution of skill files via swe-smith-generated tasks, agentic proposer in a workspace)
memskill (designer with hard-case-buffer clustering, snapshot+rollback)
pareto-lenient consensus (frontier admits temporary degraders within a budget)
alphaevolve / openevolve / shinkaevolve (single-task code evolution with island topologies)
a pairwise-tournament continual-learning optimizer where each "evaluation" is a comparison of two skill rollouts judged by an llm
a single-task search with no frontier, just keep_best
a recursive optimizer where the inner artifact is itself an optimizer config
combee (parallel scan + augmented shuffle aggregation as a custom proposer)
a confidence-aware adapter where the score is derived from logprob distributions and feedback is tiered by confidence
this list is not exhaustive of every paper, but if the design fails on any one of these the primitives are wrong.

### 3.8 honest typing

user types (artifact, evidence, trace, claims, etc) are user-defined. the framework does not attempt to infer or constrain their internal structure beyond the minimum required for the loop to function. the framework also does not lie about what it does — there is no "magic" that turns one type into another at framework boundaries. type errors occur where the type mismatch actually is.

corollary: any place the framework needs to know about user-defined structure (e.g. "what are the named components of this artifact?") goes through an explicit user-implemented trait, not reflection or convention.

### 3.9 trust separation when stages are agentic

when a proposer or evaluator is agentic and has tools, the framework must provide a mechanism to separate:

what the proposer is allowed to read (parent artifact, lineage, reflection materials, the env it's optimizing in)
what the proposer is allowed to write (the new candidate state, possibly into a sandbox)
what the proposer is *not* allowed to read or write (test data oracles, the grader, the evaluator's internal state)
this is not optional. an agentic proposer with read access to the eval set will optimize against it. the framework cannot prevent this in general — but it must provide first-class boundaries that make the right thing easy and the wrong thing visible.

### 3.10 budget bookkeeping

budget — counted in dollars, tokens, wall-clock seconds, llm calls, tool calls, or any user-defined unit — must be tracked across all stages of the loop, including stages internal to a proposer (e.g. a multi-stage proposer that itself makes llm calls). stages must be able to query and respect their remaining budget.

a stopper expressed in budget units must work at the loop level. budget bookkeeping is not a stopper; it is independent infrastructure that stoppers consume.

---

## 4. what is usually true (strong defaults)

these are not strict requirements but are properties we expect the design to support gracefully because most realistic uses want them.

### 4.1 graph as durable run state

the loop produces a graph of attempted candidates, their lineage, their evaluations, and their status in any frontiers. this graph is the durable run state. resumption, audit, reflection-over-history, and post-hoc analysis all happen against it.

the graph need not be a separate trait. it is a value type the engine maintains and passes to strategies that want to read it.

### 4.2 caching and reproducibility

content-addressable artifact identity (when available) makes idempotent eval caching trivially correct. when artifact identity is not content-addressable, caching becomes unreliable. the design should make content-addressable identity the easy path.

reproducibility — given the same seed, evaluator, and starting state, a run should produce the same trajectory — is wanted but cannot be guaranteed when stages are nondeterministic (llm sampling, agentic tool use). the design should make seeded determinism possible where the user controls all the inputs.

### 4.3 callbacks / events

research infrastructure consumers want observability. the framework should emit events at well-defined points (proposal generated, candidate evaluated, frontier updated, iteration ended, run ended, error encountered) and allow user code to subscribe.

callbacks are how integration with experiment-tracking systems (wandb, mlflow), live monitoring, and downstream tooling happens. they are first-class, not an afterthought.

### 4.4 progressive disclosure

simple cases stay simple. optimizing a single prompt with default everything should be a small number of lines, with type inference and sensible defaults doing most of the work. complex cases get complex naturally as the user opts into more parameterization. the design should not force a heavyweight setup on a lightweight problem.

### 4.5 async by default

stages involve llm calls, tool calls, network i/o, and potentially subprocess management. async is the right substrate. sync interop (block-on) is acceptable as a thin wrapper but not the default.

### 4.6 serialization

the run state (graph, frontiers, configurations) should be serializable for persistence and resumption. user types (artifact, evidence) need to be serializable for the run state to be. requiring this on user types is acceptable but the framework should not bake in a specific format — serde::Serialize + DeserializeOwned as bounds, format pluggable.

---

## 5. design forces in tension

these are the pressures that pull the design in different directions. the right design is one that makes the right tradeoffs at each tension point.

### 5.1 generality vs. ergonomics

the more abstract the primitives, the more shapes they can express, but the more code the user has to write to use them. the more ergonomic the primitives, the less abstract they are, and the more shapes they fail to express. the goal is generality at the bottom (the engine and primitive traits) and ergonomics at the top (default configurations, builder patterns, well-factored standard impls).

bias: when in doubt, prefer general primitives at the core and provide ergonomic shortcuts as separate types or builder methods. do not bake ergonomics into the primitives — that's how python gepa ended up unable to handle pairwise.

### 5.2 typed everything vs. text interchange

rust types help: they prevent whole classes of error, they make claude's job easier, they support content-addressable identity naturally. text interchange helps: it's universal, it's what llms produce, it's what most artifacts naturally are.

bias: typed everywhere the framework operates, text wherever the user wants. the artifact's internal structure is the user's choice; the framework's traits operate over typed handles.

### 5.3 small core vs. comprehensive surface

a small core is easier to maintain, easier for claude to fit in context, harder to extend in unanticipated directions. a comprehensive surface is more immediately useful, harder to keep coherent, easier for the model to misuse.

bias: small typed core, comprehensive standard library of impls in a separate module. the engine itself should be tiny — coordinate the loop, manage the graph, dispatch to strategies. everything else lives in strategies/ or proposers/ modules and is opt-in.

### 5.4 framework opinion vs. user opinion

every default is an opinion. every refused-to-default is also an opinion (it forces the user to choose). the framework should opine where there is a clear best answer and refuse to opine where there is genuine choice.

examples:
- **opine:** an artifact should have content-addressable identity if possible. callbacks emit events at fixed points. the engine loop is sequential with optional parallel proposals.
- **refuse to opine:** what counts as "improvement." what shape evidence takes. how candidates are selected. whether a frontier is maintained.

### 5.5 single-call proposer vs. agentic proposer

a one-shot LM proposer wants a pre-rendered context (text/dict). an agentic proposer wants a workspace and tools and the ability to retrieve selectively. these are very different shapes and the framework must support both at the same boundary.

bias: the boundary is the trait surface. one-shot proposers receive renderings (text, structured); agentic proposers receive workspace handles (paths, sandbox handles). the same trait is implementable both ways. the choice of how to render is the user's, not the framework's.

### 5.6 cardinal vs. ordinal evidence

cardinal evidence (numbers) composes with averaging, summing, dominance partial orders. ordinal evidence (preferences, rankings) composes with tournament-style accumulation, fitted preference models, copeland-style aggregations. the math is genuinely different.

bias: evidence is a sum type. the preference relation is a separate primitive that consumes evidence and produces a partial order. cardinal pareto is one preference relation impl. tournament-derived preference is another. mixed-mode is a third. the engine doesn't care which.

### 5.7 graph-as-truth vs. archive-as-truth

the graph records everything that happened (immutable, append-only). archives/frontiers reflect strategy opinions (mutable, may evict). the v1.0 spec correctly identified this distinction.

bias: graph is durable, append-only, framework-owned. frontiers are strategy state, queryable but not authoritative beyond their own scope.

### 5.8 model-legibility vs. type-system precision

very precise types (rich generic bounds, GATs, lifetimes interacting with async) help the compiler enforce contracts but can become unreadable to models if pushed too far. very simple types (lots of dyn, runtime polymorphism, stringly-typed dispatches) are easy to read but lose compile-time guarantees.

bias: types should be as precise as a competent CS-PhD-level rust dev would naturally write. associated types are good. heavy use of higher-kinded encodings or trait coherence tricks is suspicious. when in doubt, idiomatic > clever.

---

## 6. qualitative properties

the subtle stuff. a design can satisfy every hard requirement and still be the wrong design if it lacks these.

### 6.1 the engine is dumb, the strategies are smart

the engine's job is to coordinate the loop, manage the graph, dispatch to strategies, and bound execution. it is not where decisions live. all decisions — selection, frontier maintenance, acceptance, stopping, proposal generation, evaluation policy — live in strategy implementations. the engine should be readable in one sitting (a few hundred lines) and obvious in what it does.

this is partly for legibility (a model can read the engine and know what's happening) and partly for compositionality (strategies are the unit of variation; the engine is the invariant skeleton).

### 6.2 the user is the expert on their domain

the framework does not try to be smart about the user's domain. it does not infer artifact structure. it does not guess what evidence shape is best. it does not heuristically decide what to render. it does not pick optimization strategies based on some model of the problem. these decisions belong to the user; the framework provides the substrate.

the corollary: the user has to do meaningful work to set up an optimization. there is no optimize(artifact, eval) magic call that figures everything out. there is optimize(artifact, eval).proposer(...).selector(...).run() — explicit composition.

### 6.3 fit should be natural, not contorted

when expressing gepa in this library, the resulting code should look like gepa. when expressing textgrad, it should look like textgrad. if the user has to wrap, twist, or fake to make a paper's algorithm fit, the primitives are wrong. natural fit means the named user-facing concepts in the library line up with the named concepts in the paper, modulo translation between languages.

this is what differentiates "expressible" from "expressible naturally." both gepa and textgrad are expressible in lambda calculus. neither expresses naturally there.

### 6.4 honest about partial measurements

a candidate may be evaluated on slice A but not slice B. a slice may have observations from candidates X and Y but not Z. an evaluation may partially fail on some examples. the framework should represent these partial states honestly — Option<Observation>, per-example errors that don't invalidate the rest of the batch, slice-keyed evidence that may be sparse — rather than forcing complete measurement or pretending unmeasured == zero.

### 6.5 trace is opaque, render is the bridge

what a stage produces (trajectory, evidence, side-information) is opaque to the framework. it is whatever type the user defines. the bridge between opaque user types and the next consumer of those types is rendering — an explicit, swappable mechanism for turning an opaque value into a view consumable by a downstream stage. the framework never inspects the trace; it only carries it and renders it on demand.

### 6.6 graceful degradation, not magic

simple cases should be simple by virtue of *defaults that are individually sensible*, not by virtue of magic. when the user asks "why is this happening," the answer should be "because you used the default X, which does Y," not "the framework figured it out." this is critical for model-legibility — a model debugging a setup should be able to trace behavior to specific named components.

### 6.7 the optimizer is a value, not a class hierarchy

an optimizer instance is a configured composition of strategies — a value, not a subclass. there is no "GEPAOptimizer extends BaseOptimizer." there is Engine::new().proposer(...).selector(...).archive(...).build(), and the resulting value *is* the configured optimizer. swapping a strategy is swapping a field. this is rust-idiomatic and aligns with how models reason about composition.

### 6.8 boundary tightness when tools enter

every place the framework hands off to user code that may have side effects (file system writes, llm calls, subprocess spawns), the boundary should be explicit and the mechanism for bounding/sandboxing should be obvious. when reading the engine code, a model should be able to point at the spots where "user-controlled side effects happen here" and be confident no others exist.

### 6.9 names should explain themselves

when a model encounters a type or trait name in this library, the name should immediately suggest what role it plays. ParentSelector is good for GEPA parent choice; ChoicePolicy is bad (could be many things). PreferenceRelation is good; Comparator is bad (collision with std). Evidence is good; Feedback is mediocre (overloaded). this isn't pedantry — it's the index into the library.

### 6.10 the loop is observable end-to-end

at any point in a run, a user (or a model debugging the run) should be able to inspect: which parents were selected and why, what was proposed, what evidence was produced, whether the proposal was accepted into a frontier and why, what the frontier currently looks like. this is the audit story but it's also the debugging story — opacity here is the difference between "i can fix this" and "i don't know what happened."

### 6.11 honest about cost

every stage that costs something — money, time, tokens, compute — should report its costs explicitly through the budget mechanism. the engine should never absorb costs silently. this matters most for agentic stages where a single proposer call could spend $5 and run for 10 minutes; the framework should make this visible, not hide it inside an opaque proposer abstraction.

---

## 7. the expressibility test (operationalized)

the test of whether the design is right is whether competent models can express the optimizers in §3.7 in the library, given the library plus a reasonable docstring/examples surface, in one shot.

### 7.1 procedure

for each optimizer in the test set:
1. give a model the library's core types, traits, and a small number of worked examples (maybe gepa, textgrad, and one custom).
2. give the model a description of the target optimizer (paper or prose).
3. ask for an implementation as user code (trait impls + composition).
4. compile, run on a toy problem, check that behavior matches the description.

### 7.2 pass conditions

compiles without modification of the library
runs without panic
behavior matches the optimizer's description (e.g. gepa actually maintains an instance pareto front, mipro actually bootstraps then transitions to bayesian acquisition)
the implementation is short and readable — comparable in size to the python equivalent
### 7.3 failure modes (and what they mean)

**model asks for a new trait.** primitive missing. design gap.
**model produces something that compiles but does the wrong thing.** trait contract unclear. docstring or naming gap.
**implementation is several times the size of the python equivalent.** factoring is wrong; composition requires too much wiring.
**model expresses the optimizer but the result doesn't *look* like the literature description.** fit isn't natural; primitives map to wrong concepts.
### 7.4 stretch goal: novel optimizer expressibility

beyond reproducing the literature, the design should make it easy to express *new* optimizers that don't yet exist. the test: given a paper published after the design freeze, can a model implement its optimizer in the library? this is the long-term value proposition — research infrastructure that doesn't go stale.

---

## 8. non-goals

these are explicitly out of scope. designing for them dilutes the focus.

a user-facing CLI. this is a library, called from rust code or potentially from python via PyO3 later.
a built-in distributed engine. parallel proposals on a single machine: yes. distributed sharding: out of v1, possibly later.
a managed service or hosting story.
baked-in observability backends. callbacks emit events; users wire wandb/mlflow themselves (with provided helper crates if useful).
a specific LLM SDK as a dependency. the framework depends on a small Lm trait; concrete adapters (anthropic, openai, local) live downstream.
automatic detection of artifact structure or evidence shape. user declares both via types.
domain-specific helpers (no built-in optimize_my_dspy_program shortcut). domain helpers live in adapter crates.
a "skill marketplace" or shared run registry. the design should make this *possible* (content-addressable artifacts, serializable run state) but not build it.
backward compatibility with python gepa's adapter contract. inspiration, yes; api compatibility, no.
support for online learning at framework level beyond what falls out of Slice::Recent-like primitives. production streaming is its own thing.
---

## 9. open questions

things we know we don't know. honest acknowledgment that solution-shape can't be fully determined from the requirements alone.

### 9.1 the right factoring of preference relations

evidence shape and preference relation are separate concepts. but at what point are they separate? options:
- evidence is a sum type (scalar / multi-axis / pairwise / listwise), and the preference relation consumes the sum type.
- evidence is generic in its shape, and the preference relation is generic in evidence.
- evidence is a value type, and the preference relation is a trait that knows how to read it.

each has tradeoffs around extensibility, ergonomics, and type-inference behavior. resolving this requires writing several preference relation impls and seeing which factoring composes cleanly.

### 9.2 the right surface for "rendering"

rendering is a primitive but its trait surface is unclear. options:
- a single Render<T, Ctx> trait that takes any value and produces a generic RenderedView.
- per-target renderings (RenderForReflection, RenderForProposal, etc) reflecting the consumer's needs.
- consumer-specified rendering (the proposer trait says what kind of rendering it accepts).

probably the third, but the type-system mechanics need to be worked out.

### 9.3 frontier as one trait or many

a single Frontier trait parameterized on cell type and preference relation may cover instance-pareto, objective-pareto, map-elites niche, beam, and tournament. or these may genuinely need different traits. the expressibility test will surface this.

### 9.4 stage composition

a "stage" (eval / reflect / propose) might want to be a single trait with three variants, or three separate traits, or two (eval and propose, with reflect either fused into propose or expressed as a sub-call). the right factoring depends on how often stages share infrastructure (sandboxing, budget, rendering) and how often they don't.

### 9.5 what the engine actually owns

clearly: the loop, the graph, dispatching to strategies, budget, callbacks. not clearly: the frontier(s) (or are they strategy-owned?), the cache (or is it strategy-owned?), the rng (per-strategy or shared?), the slices (engine-defined or evaluator-defined?).

bias toward minimal engine ownership; but cross-strategy coordination (e.g. selector reading what archive built) requires *some* shared state, and where that lives is open.

### 9.6 how to express trust separation between proposer and evaluator

mentioned in §3.9 as a hard requirement. the *mechanism* — sandbox handle types, capability tokens, separate engine entry points — is open. likely involves wrapping handles in newtypes that restrict what they can do. needs design work.

### 9.7 the budget abstraction

scalar (one number) is too restrictive. structured ({ usd, tokens, seconds, calls }) is more accurate but more annoying. user-extensible (trait BudgetUnit) is most general but most complex. the right answer is probably structured-with-user-extensible-axes, but the surface needs sketching.

### 9.8 caching policy

content-addressed identity makes idempotent eval caching trivial when the evaluator is deterministic. when the evaluator is nondeterministic (agent w/ tools, llm sampling), caching is wrong unless the user opts in. and even then, the cache key needs to include the eval's "version" (model, prompt, tools available). how the framework expresses this is open.

### 9.9 the right defaults for the standard library

we will ship some number of standard strategy impls (parent selectors, frontiers, eval policies, acceptance criteria, stoppers, proposers). the right *list* is open. too few and users have to write everything; too many and the surface bloats. likely: ship the impls necessary to reproduce the literature in §3.7 and stop there until concrete demand emerges.

---

## 10. appendix: literature targets

these are the optimizers we want to express. each is a touchstone for a particular design pressure. in alphabetical order; this is not a priority list.

**alphaevolve / openevolve / shinkaevolve** — single-task code evolution; tests workspace-shaped artifacts with island topologies and prompt co-evolution.
**arc-agi agent architecture evolution (optimize_anything)** — evolving an entire agent system as a single text artifact; tests scale, agentic evaluator, multi-aspect dataset.
**c-evolve** — n-island parallel populations; tests parallel proposers and migration.
**combee** — parallel-scan + augmented-shuffle aggregation; tests custom proposers that operate over many traces.
**confidence adapter** — logprob-derived continuous scores with tiered feedback; tests custom evaluator producing structured evidence.
**gepa (paper)** — instance-pareto frontier, frequency-weighted parent selection, minibatch acceptance, round-robin component selection. the canonical reference.
**gepa+merge** — adds the merge proposer; tests proposer arity / multi-parent.
**gskill** — skill files evolved via swe-smith tasks; tests filesystem-backed artifacts and agentic evaluators.
**map-elites for prompts** — niche-keyed frontier with uniform niche selection; tests behavioral-niche frontier shape.
**memskill** — designer with hard-case-buffer clustering and snapshot+rollback; tests history-aware proposers.
**mipro / mipro v2** — bootstrap of grounded candidates then bayesian acquisition; tests two-phase proposers and surrogate models.
**moprompt** — multi-axis cardinal pareto; tests multi-objective preference relation.
**muf/edit** — typed two-stage diagnose-then-rewrite with structured claims and claim verification; tests user-typed claims, multi-stage typed proposers, and claim-aware acceptance.
**optimas** — globally aligned local rewards per module; tests per-component evidence attribution.
**pareto-lenient consensus** — frontier admits temporary degraders within budget; tests non-strict frontier policies.
**single-task search with no frontier** — keep-best-only; tests "no frontier" as a valid configuration.
**pairwise tournament continual learning** — preference learning via pairwise rollout judging; tests pairwise evidence and tournament-derived preference relations.
**recursive optimizer (memento-skills)** — inner artifact is itself an optimizer config, evaluated by running an inner optimization; tests artifact composability.
**tep (textual equilibrium propagation)** — per-component local critics with two-phase free/nudged updates; tests per-component proposers.
**textgrad** — per-variable feedback aggregation, single greedy candidate; tests per-component evidence attribution and aggregation.
**trace / optoprime** — subgraph-as-code reasoning over computation graphs; tests structured trace consumption.
**vista** — diagnose-then-rewrite (independently of muf/edit); tests two-stage proposer pattern.
each of these needs a worked example post-implementation. failure to express any one of them naturally is a design defect.

---

## 11. closing

this document does not specify the solution. it specifies what the solution must satisfy and what shapes the design space. the next step is solution design, against this substrate, with periodic checks back to this doc when a tradeoff is unclear.

the doc is meant to be edited as our understanding sharpens. open questions in §9 are the most likely candidates for revision; design forces in §5 are stable; hard requirements in §3 should only change with strong reason.

simplicity-for-models in §2 is the meta-constraint. when in doubt, choose the option that makes a competent model's job easier, even at some cost to elegance or human-first ergonomics. that is the point.

