# Frustrations

This is intentionally not a polished design note. It is the complaint log. The
point is to name the painful shit directly so we do not accidentally smooth it
over into "looks fine" prose.

## The Biggest Lie Waiting To Happen

The worst failure mode is calling the P5 live proof a paper reproduction.

It is not. It is a real product-path proof. It is a real Leaven agentic skill
mutation. It is not a faithful EvoSkill reproduction. If we let ourselves blur
that line, the whole project gets intellectually mushy.

The live gate proves:

- Codex can run through the Leaven agent runtime path.
- A skill folder can be generated, parsed, validated, proposed, applied,
  evaluated, stored, checkpointed, and resumed.
- The graph path is real enough that the mutation is not just a script.

It does not prove:

- OfficeQA behavior.
- SealQA behavior.
- paper train/validation/test splits.
- paper frontier schedule.
- paper history accumulation.
- paper ablations.
- paper graders.
- paper tool environment.
- paper metrics.

That distinction needs to stay screamingly visible. Otherwise we will end up
with a library that can demo itself but cannot reproduce the science that is
supposed to pressure-test it.

## Codex App-Server Is Still Too Awkward

The current Codex app-server path worked, but it did not feel like a clean
primitive yet.

The biggest irritation: the shell/tool path was not reliable enough for the P5
proof, so the example uses a no-shell final-message JSON contract. That was the
right engineering call for the slice, but it is also a big flashing sign that
we have not proven the full agentic execution path.

For actual skill papers, the agent needs to do real workspace work:

- inspect files;
- run commands;
- write files;
- maybe use scripts from skills;
- produce transcript evidence;
- fail in debuggable ways;
- keep enough session state that repair is not a weird detached ritual.

Right now the final-message JSON path proves typed output. It does not prove
toolful agency. It lets us avoid the worst provider weirdness, but the papers
do not get to avoid it. They are about agents doing work.

The stdio app-server connector also feels fragile as hell for containers. Stdio
is convenient locally, but the moment the runtime and workspace are not in the
same host process world, all the annoying questions come back:

- Where does Codex run?
- Where does the workspace mount?
- Does the app-server see the same filesystem as the sandbox?
- Are commands running in the right namespace?
- Are paths host paths, guest paths, or protocol paths?
- How do we capture command output without leaking backend-specific details?

This should not infect `leaven-agent`, but we need a concrete product answer in
the Codex app-server crate or a sibling connector. Otherwise every paper
example will carry its own duct tape.

## Provider-Neutral Runtime Is Correct, But It Is Barely Proven

The boundary is right:

```text
AgentRuntime runs one session in an already-materialized world.
It does not know candidates, proposals, evidence, SkillBank, or RunGraph.
```

But right now the real proof is narrow:

- fake runtime covers the generic laws;
- Codex app-server covers a live final-message path;
- toolful Codex execution is not yet proven in the Leaven path;
- non-local workspace semantics are not proven;
- cost accounting is not yet something I would trust for paper budgets.

So the trait design is good. The implementation proof is still thin.

The irritating thing is that agent runtimes are where all the messy truth goes:

- cancellation;
- timeout;
- token accounting;
- provider model config;
- tool policy;
- transcript shape;
- output contracts;
- workspace attachment;
- developer instructions;
- session reuse;
- retry behavior;
- provider protocol bugs.

The trait can stay small, but the product surface around it cannot be vague.
If we leave this underspecified, every backend crate will invent its own little
runtime religion.

## Materialization Is Doing Too Much Conceptual Work

The artifact/materializer/runtime split is still right, but materialization is
where the pressure collects.

The artifact says "this is the skill bank." The materializer says "here is how
an agent sees it." That is clean in a diagram. In a real agent run, the
materializer also becomes the owner of a pile of ABI decisions:

- `.agents/skills/<name>/SKILL.md` versus some provider-native path;
- whether skills are read-only or mutable;
- whether task input lives under `task/`, root, or a harness-specific path;
- whether output lives under `output/`;
- whether scripts need executable bits;
- whether references are copied, linked, or filtered;
- whether hidden validation material is withheld;
- whether the executor and proposer see the same world shape.

That is fine if we admit it. It is bad if we pretend materializers are boring
file-copy helpers.

Materializers are the executable ABI for artifacts. They need laws, fixtures,
and maybe named layouts. Otherwise "same artifact" will mean five subtly
different things across examples.

## Workspace Semantics Still Have Teeth

The local workspace path is usable now. The backend-neutral story is still not
proved hard enough.

The whole point of `Workspace` is that examples should not reach for host
`std::fs` and silently break E2B/Docker/K8s/Firecracker. We fixed obvious local
path bullshit, but there are still plenty of traps:

- read/write/list/run must all route through the backend;
- local mount must be optional;
- command cwd must be workspace-relative;
- file paths must not escape;
- cleanup must happen on error;
- cleanup failure must not erase the primary stage failure;
- backend unsupported operations must fail explicitly;
- provider runtimes must not smuggle host paths into transcripts as contract.

This is exactly the kind of substrate where one lazy example can poison the
API. We need to keep slapping examples when they use host paths.

## Skill Validation Is Necessary But Also Annoyingly Weak

The current validation is the right minimum:

- mandatory `SKILL.md`;
- valid name;
- description;
- non-empty body;
- metadata bag;
- path safety;
- executable bit preserved but not semantically magical.

But this only proves "valid package", not "good skill."

That distinction is annoying but important. A skill can be perfectly valid and
still useless, misleading, unretrievable, too broad, too narrow, unsafe,
overfit, or impossible for the agent to apply.

So parse validation is necessary and not nearly enough.

We still need product patterns for:

- semantic validation;
- behavioral validation;
- retrieval validation;
- "would this skill have helped here?" counterfactual checks;
- over-broad description detection;
- stale references;
- script sanity;
- body/reference consistency;
- whether the description actually names the use conditions.

The library should not hardcode those judgments, but it does need the generic
places to put them. Right now that is still underdeveloped.

## The Repair Loop Is The Right One, But Still Too Narrow

I like the one proposer-owned repair loop. It is the right shape:

```text
agent proposes mutation
parser/validator rejects it
same proposer gets repair feedback
bounded retry
valid proposal enters graph
```

But right now it is mostly parse/shape repair. That is not enough for real
skill optimization.

The useful repair loop will often be:

- generated invalid `SKILL.md`;
- generated valid but empty-body nonsense;
- generated a skill whose name does not match the directory;
- generated a skill that references missing files;
- generated a script but forgot executable/dependency notes;
- generated a skill that fails the validation task;
- generated a skill with an over-broad description that routes everywhere;
- generated a skill that helps train but hurts validation.

Some of those are parser errors. Some are evaluator evidence. Some are
admission failures. Some are retrieval failures. If we collapse all of that
into "repair failed", the optimizer will be blind.

The repair primitive needs typed failure evidence, not just strings.

## The Example-Owned Checkpoint Enum Is A Smell

P5 has a checkpoint enum because it needed to be resumable now. That was fine.
But if every serious optimizer example invents its own checkpoint story, we are
going to hate ourselves.

Long-running agentic optimization needs resumable state for:

- run graph identity;
- seed artifact;
- current artifact bank;
- pending proposal;
- accepted proposal;
- rejected proposal;
- evidence refs;
- population/frontier state;
- feedback history;
- private optimizer strategy state;
- random seeds;
- budget ledger;
- provider session IDs if reuse matters;
- version/fingerprint of prompts, scorers, datasets, and runtime config.

`FileCheckpointStore` is useful, but the library story is not done. We need a
standard pattern for optimizer-owned resumable state that does not shove
everything into engine internals and does not require every paper example to
invent the same machinery badly.

## Evidence Is Still Too Blob-Like

Typed evidence exists. File evidence exists. Good.

But the evidence we need for skill optimization is richer than "score and some
notes."

We need to answer:

- What task was attempted?
- What skills were visible?
- Which skill descriptions were visible at routing time?
- Which skill was loaded?
- Which referenced files were read?
- Which scripts were executed?
- Did the agent ignore the right skill?
- Did the agent use the wrong skill?
- Did the skill help or hurt?
- Was the failure caused by missing skill, bad skill body, bad retrieval, bad
  harness, bad tool call, or bad base model behavior?
- What feedback was shown to the proposer?
- What prior feedback history was shown?
- What exact candidate state was evaluated?

If that all lives in opaque transcript JSON, selectors and optimizers cannot
use it. If the library over-structures it, it will become a paper-specific
monstrosity.

We need a middle path: optional typed evidence capabilities for trajectories,
skill activations, retrieval decisions, scorer output, and attributions. Not a
mandatory mega-evidence type. But not "lol parse the transcript yourself"
either.

## Retrieval Is The Real Scientific Mess

Skill optimization is not just "write better skills." It is "write skills that
get retrieved at the right time and help when used."

That is much harder.

A skill can fail because:

- it was never retrieved;
- it was retrieved too often;
- it was retrieved for the wrong task;
- its description is missing the trigger words;
- its description is too broad;
- its description is correct but the agent ignores it;
- the body is good but too long;
- the body buries the useful bit;
- the script works but the instructions do not tell the agent when to run it;
- the skill helps only after another skill is loaded;
- two skills conflict;
- the skill was generated from one failure and overfits that failure.

This is where the papers are actually interesting. If Leaven cannot preserve
retrieval/routing evidence, we will not be able to reproduce the important part
of the field.

The thing I want to know for every evaluation is not just "score improved." It
is:

```text
Was a relevant skill available?
Was it shown?
Was it loaded?
Was it used?
Did use causally help?
Would it have helped if shown?
Did the description cause the right retrieval decision?
```

That is the heart of the damn problem.

## Candidate Selection Must Stay Swappable

The earlier temptation to make candidate selection feel like GEPA-only was
wrong. Skill papers make this obvious.

Different papers need different rhythms:

- one failure to one skill;
- many failures consolidated into one skill;
- persistent library update;
- dual-granularity skill banks;
- skill compression and validation;
- utility-weighted pruning;
- beam/frontier admission;
- Pareto over cases;
- pairwise preference;
- random exploration;
- round-robin target selection.

If candidate selection gets baked into GEPA, the library gets stupid fast.
Selection, admission, population, and optimizer rhythm have to stay separable.

The current direction is better, but the pressure is not fully paid down.
Paper examples will expose whether the abstractions are actually reusable or
just named reusable.

## The P5 Example Is Too Big

`examples/p5_evoskill_iteration/src/main.rs` is doing a lot. It is paper
specific, so some of that is fine. But it is also a warning.

Large examples are dangerous because they hide missing library primitives.

When an example grows, ask:

- Is this paper-specific harness logic?
- Or is this generic Leaven product machinery hiding in the example?

The current P5 crate owns:

- tiny dataset;
- scorer;
- EvoSkill role prompt wrapping;
- phase checkpoint enum;
- proposal/build JSON schemas;
- live loop wiring.

Some of that should stay there. Some of it might be a symptom that we need
better reusable stage adapters for:

- role/session evidence recording;
- typed final-message JSON parsing;
- common checkpoint phase patterns;
- live/resume runner ergonomics;
- "evaluate baseline, collect failures, propose, build, validate, admit" loops.

I do not want a magic EvoSkill optimizer in core. But I also do not want every
paper reproduction to be a 1300-line `main.rs` shrine.

## The Naming Is Still Not Fully Settled

"Finalize" was bad. "Import" was also not quite right. The underlying concept
is still awkward:

```text
workspace changed
parser reads workspace
validated artifact change is produced
proposal enters graph
candidate is created
```

The important thing is that this is not merely file import. It is not merely
finalization. It is a boundary crossing from side-effectful workspace world into
typed Leaven artifact/proposal world.

I do not love the vocabulary yet.

If the name is wrong, downstream users will misunderstand the boundary. They
will think the runtime mutates candidates directly. It does not. The runtime
mutates a workspace. The parser lifts that into typed changes. The graph records
truth through `RunContext`.

That boundary needs a name that makes the ownership clear.

## Git/JJ Artifact Support Is Still Future Pain

We correctly did not make git the default skill substrate. Good.

But agentic optimization over codebases is going to hit VCS semantics
immediately:

- fork candidate;
- run agent in a checkout;
- collect diff;
- preserve executable bit;
- record commit or tree identity;
- rewind/retry;
- branch multiple children;
- compare candidates;
- materialize into containers;
- avoid worktree hell;
- maybe use git bundles or snapshots.

This is not required for the current SkillBank path, but it is absolutely part
of the agentic optimization product surface.

The pain point: git is too useful to ignore and too opinionated to make the
default. That means Leaven needs first-class git artifact support without
letting git leak into generic artifact semantics.

## Cost Accounting Is Not Yet Good Enough

For sample-efficiency papers, cost and rollout count are not decoration. They
are the argument.

Right now I would not be comfortable saying Leaven can reproduce paper cost
claims. We can store costs structurally, but live provider accounting still
needs to be much tighter:

- model input tokens;
- model output tokens;
- cached tokens;
- tool call cost;
- wall-clock time;
- command/runtime cost;
- failed attempts;
- repair attempts;
- validation rollouts versus training rollouts;
- replay/resume avoiding duplicate spend.

If the paper says "35x fewer rollouts" or "budget B", the library needs to
make those counters boringly auditable. Currently we are not there.

## Cache Identity Is Still A Landmine

Separating cache identity was the right call. But every agentic artifact makes
cache identity harder:

- skill folder content;
- executable bit;
- hidden task data;
- runtime config;
- model config;
- tool policy;
- materializer layout;
- scorer version;
- dataset version;
- prompt wrapper version;
- environment image;
- nondeterministic tools;
- network access.

If any of those are missing from the cache key, evaluation caching becomes a
lying machine.

I am glad we did not paper over this. I am also annoyed because full correctness
requires a lot of fingerprints.

## Tests Are Good, But The SLA Is Brittle Around Cold Builds

The tests themselves passed. The cold `just check` failed the SLA because it
included compile time after coverage/build churn. Warm `just check` passed.

That is not a correctness failure, but it is annoying operator friction. The
SLA is supposed to protect test runtime, not punish a cold dependency compile
caused by a feature-heavy slice.

We should keep the SLA. But we should be honest that cold-build behavior can
make the signal noisy. If this becomes common, the script should distinguish
compile time from test execution time more explicitly.

## Coverage Pressure Can Push Weird Tests

Coverage is useful. Branch coverage is especially useful. But when we are close
to the floor, it starts tempting stupid tests.

I hit one example: trying to create a non-UTF-8 filename to force a file-store
branch. macOS rejected the filename before Leaven could see it. That was a bad
test target because it depended on filesystem behavior outside the library's
contract.

The lesson: coverage gaps should be patched only with real contract tests or
private invariant tests. Otherwise coverage turns into nonsense theater.

## Specs Are Better, But Still Lag The Real Pain

The specs now say much more of the right thing. Still, implementation exposed
details that are easy to under-document:

- live proof versus full paper reproduction;
- source-prompt fidelity versus runtime wrapper constraints;
- final-message output as a stage choice;
- provider leaf dependency boundaries;
- repair loop ownership;
- evidence/checkpoint product requirements;
- skill retrieval instrumentation.

If these distinctions are not in the specs, future agents will re-litigate
them or overclaim. Specs need to keep the embarrassment visible.

## The Five-Paper Pressure Set Is Still Mostly Pressure, Not Proof

Only EvoSkill has a live first gate. The rest are still unimplemented pressure
tests.

The missing examples matter because each one attacks a different weak spot:

- Trace2Skill attacks batch consolidation.
- Memento-Skills attacks persistent library update and routing.
- D2Skill attacks utility attribution and dual granularity.
- SkillReducer attacks compression, progressive disclosure, and faithfulness.
- EvoSkill attacks create/edit skill generation from failures.

If we only finish EvoSkill, Leaven may become an EvoSkill-shaped library while
pretending to be general. The other examples are how we keep ourselves honest.

## The Most Important Missing Primitive Is Probably Retrieval Events

If I had to pick the single biggest underbaked area, it is retrieval/routing
events.

Skill optimization without retrieval evidence is half blind.

We need to preserve:

- all available skill descriptors;
- descriptors shown to the model;
- skill activation decision;
- activation rationale if available;
- loaded files;
- retrieval score/ranker output;
- task context used for retrieval;
- missed-skill counterfactuals when possible;
- downstream outcome.

This should probably be optional evidence capability, not mandatory core. But
it needs to be first-class enough that optimizers can depend on it.

Otherwise we will keep optimizing skill bodies while the real bug is the
description/routing layer.

## The Second Most Important Missing Primitive Is Optimizer State Resume

File checkpointing works. Product-grade optimizer resume is not done.

Long-running skill optimization needs resume semantics that are not just
"deserialize whatever this example enum says."

The library needs a pattern for:

- restoring graph;
- restoring population;
- restoring pending phase;
- restoring evidence refs;
- restoring random seeds;
- detecting completed work;
- refusing stale checkpoints when prompt/scorer/runtime fingerprints changed;
- letting optimizers serialize explicit private state without making that state
engine-owned.

This is a real library feature, not an example nicety.

## The Third Most Important Missing Primitive Is Toolful Session Evidence

For agent papers, transcripts cannot just be final answer text.

We need:

- tool calls;
- command records;
- file mutations;
- stdout/stderr;
- exit status;
- duration;
- working directory;
- maybe redaction/trust boundaries;
- references to output files;
- provider-native session IDs.

The current generic transcript pieces point in the right direction. The live
Codex proof did not fully exercise them. That gap has to close before claiming
real paper reproduction.

## The Thing That Still Feels Most Structurally Risky

The most structurally risky thing is that "agentic optimization" can become a
bag of adapters instead of a coherent substrate.

The shape must stay:

```text
Artifact state
Surface targets
Materialized workspace
Runtime session
Parsed proposal/evidence
RunContext graph mutation
Population/admission
Checkpoint/evidence persistence
```

If any example skips around that, it is probably cheating.

The temptation will be huge:

- let the agent patch the artifact directly;
- let the evaluator mutate graph state;
- put skill semantics into the runtime;
- put provider protocol into generic crates;
- store transcripts as untyped blobs forever;
- make git the substrate for everything;
- hardcode paper loops into Leaven core;
- call a demo a reproduction.

All of that would make the library worse.

## What I Am Actually Mad About

The annoying part is that the architecture is mostly right, which means the
remaining work is not glamorous. It is the hard boring shit:

- exact task loaders;
- exact scorers;
- exact prompts;
- exact runtime behavior;
- exact evidence records;
- exact resume behavior;
- exact cache fingerprints;
- exact retrieval traces;
- exact failure attribution;
- exact paper loops.

There is no clever abstraction that makes those disappear. The library can make
them composable and auditable. It cannot skip them.

So the real frustration is this:

Leaven is now close enough that fake proofs are easy.

That is dangerous as hell.

The next phase has to be hostile to fake proof. Every paper example should make
us answer:

- Is this the real paper setup or a toy?
- What is generic Leaven machinery?
- What is paper-specific harness code?
- What evidence proves the agent actually ran?
- What evidence proves the skill was actually used?
- What evidence proves resume did not rerun spend?
- What evidence proves the candidate entered through `RunContext`?
- What evidence proves retrieval worked or failed?
- What evidence proves the result is not hardcoded?

Until those answers are boring, the library is not done.

