# Eval Nomenclature And Presentation

Status: planning vocabulary note.
Date: 2026-05-10.

This doc is a naming and presentation pass for Leaven's optimizer/eval surface.
It does not replace `docs/specs/eval_protocol_detail.md`; it explains how the
concepts should be named and taught before we revise implementation specs.

The problem: the current eval detail spec contains useful boundaries, but the
names drift too far into implementation plumbing. To name a concept is to know
it. If the names feel wrong, future code will scatter.

## 1. What Should Feel Familiar

Leaven is an optimizer library. Its public language should be legible to people
coming from GEPA, DSPy, ML evaluation, benchmark suites, and prompt/program
optimization.

Use familiar words when they are honest:

```text
evaluation
dataset
split
train
validation / dev
test
metric
score
trace
report
budget
```

Do not invent new names for concepts ML-land already owns. Leaven should differ
from GEPA in type safety, provenance, durability, surfaces, and generality, not
in gratuitous vocabulary.

## 2. What GEPA Names Well

GEPA's public shape is useful because the concepts are recognizable:

```text
candidate
task / examples
train batch
validation
score
trace
feedback
reflection
budget
```

That is the right user-facing gravity. Leaven should preserve that feel where
possible.

Where Leaven must diverge:

| GEPA/Python shape | Leaven shape | Why |
| --- | --- | --- |
| `str` or `dict[str, str]` candidate | typed `Artifact` | Leaven optimizes more than prompt strings. |
| prompt component key | `EditSurface` part | Editable structure depends on artifact/domain. |
| adapter-returned scores/traces | graph-backed assessments/evidence | Runs must be resumable, auditable, budgeted, and trust-scoped. |
| dataset arrays only | optional dataset plus evaluation spec | Some evals are live, online, pairwise, or environment-backed. |
| local optimizer state only | graph + optimizer state | Leaven needs durable provenance across optimizers. |

The difference should feel like:

```text
GEPA, but typed and durable.
```

Not:

```text
GEPA, but with a new private language.
```

## 3. Core Distinctions

These distinctions are real and should be preserved:

```text
Evaluation spec  = what is measured and how measurements are allowed to count.
Dataset          = optional examples/tasks/prompts/fixtures.
Dataset split    = named partition of dataset cases.
Environment      = optional execution substrate used by an evaluator.
Evaluator        = executable capability that produces assessments.
Evidence         = durable measured output.
Report           = readable summary over graph truth.
Optimizer        = search rhythm and strategy state.
```

Important:

- evaluations are not always datasets;
- datasets are not always environments;
- environments are not evals;
- evaluators execute; specs do not;
- reports summarize; reports do not become truth.

## 4. Preferred Names

Current internal-ish names and preferred public names:

| Current / proposed | Prefer | Why |
| --- | --- | --- |
| `EvalPlan` | `EvaluationSpec` | ML-native; says declaration, not execution. |
| `EvalProtocol` | avoid as type name | Sounds like runtime protocol or separate crate. |
| `CaseCatalog` | `Dataset` or `CaseDataset` | Familiar; optionality is modeled by `Option`. |
| `EvalCase` | `Example` or `Case` | Simpler; domain adapters can use richer case types. |
| `SplitManifest` | `DatasetSplits` | Names the concept, not the storage artifact. |
| `SplitRole` | `Split` or `SplitRole` | Keep only one split-role vocabulary. |
| `SplitPermissions` | `SplitUsage` | Less security-coded; says how split is used. |
| `EvalUse` | `EvaluationUse` | Clearer and less abbreviated. |
| `LeakagePolicy` | `VisibilityPolicy` | Names intended access rule, not failure mode. |
| `EvalRunReport` | `EvaluationReport` | Familiar and direct. |
| `ReportAxis` | `Metric` or `MetricAxis` | ML-native. |
| `ScoreSummary` | `ScoreSummary` | Fine; it is a report projection. |

Best current surface vocabulary:

```rust
pub struct EvaluationSpec { ... }
pub struct EvaluationSuite<C = Case> { ... }
pub struct Dataset<C = Case> { ... }
pub struct DatasetSplits { ... }
pub struct SplitUsage { ... }
pub enum EvaluationUse { ... }
pub struct VisibilityPolicy { ... }
pub struct EvaluationReport { ... }
```

`EvaluationSuite` should mean:

```text
EvaluationSpec + optional Dataset + optional DatasetSplits
```

It should not mean "runnable evaluator".

## 5. Names To Avoid In Public Concepts

Avoid these as public concept names unless a narrower doc gives a strong reason:

```text
protocol
manifest
permissions
leakage
catalog
influence
scheme
substrate
Harbor/AISI-like
```

Why:

- `protocol` suggests runtime handshakes or a separate protocol crate;
- `manifest` suggests a file/storage detail;
- `permissions` suggests security ACLs rather than optimizer usage;
- `leakage` names a failure mode instead of the access rule;
- `catalog` is abstract and less ML-native than dataset;
- `influence` is conceptually useful but too foreign for users;
- `scheme` is vague;
- `substrate` is internal architecture language;
- `Harbor/AISI-like` should remain inspiration/source material, not Leaven
  product vocabulary.

## 6. Public Explanation

The user-facing explanation should be:

```text
Leaven runs optimizers over typed artifacts.

An evaluator measures candidates.
An evaluation spec says which measurements are allowed to affect optimization.
A dataset is optional; when present it can have train/validation/test splits.
A visibility policy prevents hidden split content from reaching proposers.
The engine records assessments, evidence, cost, and provenance in the graph.
The optimizer uses visible evidence according to the evaluation spec.
```

This keeps familiar ML concepts while preserving Leaven's stronger graph and
type boundaries.

## 7. Budget Naming

Budget should remain engine-shaped, not GEPA-shaped.

Preferred concepts:

```text
Cost             one charged event or aggregate amount
Budget           consumed resources / ledger view
BudgetLimit      hard cap
BudgetPolicy     what to do when a limit is near or exceeded
```

GEPA builder ergonomics may expose:

```text
max_iterations
max_metric_calls
max_lm_calls
max_cost
max_wall_time
```

But those lower into shared engine budget limits. GEPA does not own budget
semantics.

Why budget is shaped this way:

- evaluators can cost money/time/tokens;
- reflection LMs can cost tokens/money;
- agentic tasks can cost runtime/workspace time;
- pairwise judges and human review can have their own units;
- multiple optimizers need the same accounting story.

So budget is a cross-cutting run constraint, not a GEPA policy.

## 8. Recommended Revision Direction

Do not delete the detailed spec. Instead, revise it to present concepts first
and implementation scaffolding second.

Suggested order for the next revision:

1. Rename public concept language:
   - `EvalPlan` -> `EvaluationSpec`
   - `CaseCatalog` -> `Dataset`
   - `SplitManifest` -> `DatasetSplits`
   - `SplitPermissions` -> `SplitUsage`
   - `EvalUse` -> `EvaluationUse`
   - `LeakagePolicy` -> `VisibilityPolicy`
   - `EvalRunReport` -> `EvaluationReport`
2. Keep exact crate-boundary decisions:
   - existing core/engine/evidence/agentic types do not move;
   - `leaven-eval` does not depend on `leaven-engine`;
   - engine adapters stay outside first `leaven-eval`.
3. Move file/module graph lower in the doc or into an implementation section.
4. Replace implementation-flavored prose with concept laws:
   - datasets are optional;
   - splits are named dataset partitions;
   - split usage controls optimizer access;
   - visibility controls actor access;
   - reports cite graph IDs, not copied truth.
5. Keep the explicit-case leakage warning, but explain it as a visibility
   invariant rather than an implementation wart.

## 9. One-Screen Vocabulary

```text
Artifact          thing being optimized
Surface           typed editable view over the artifact
Evaluator         executable measurement capability
EvaluationSpec    rules for how measurements count
Dataset           optional cases/examples/tasks
DatasetSplits     train/validation/test partitioning
SplitUsage        which split can affect which optimizer decisions
VisibilityPolicy  which actors can see which split/evidence
Assessment        graph-recorded measurement result
Evidence          measured details behind an assessment
EvaluationReport  graph-backed summary of what was measured and used
Budget            shared run resource accounting
Optimizer         search rhythm
```

This is the vocabulary that should guide the next pass over the detailed spec.
