# Eval Nomenclature And Layers

Status: planning vocabulary note.
Date: 2026-05-10.

This doc records the naming correction for Leaven's optimizer/eval surface. The
authoritative public/private contract is
`docs/specs/gepa_public_private_surface.md`; the lowered type-level contract is
`docs/specs/eval_lowering_detail.md`.

## 1. Correction

The earlier naming pass was still too implementation-shaped. Renaming
`EvalPlan` to `EvaluationSpec` does not solve the product problem if ordinary
users still have to learn a new "spec" object before running GEPA.

The public surface should start from familiar user actions:

```text
optimize
train
validation
test
score
evaluate
reflect
budget
report
```

The lowered surface can use precise types, but those types must not become the
front-door explanation.

## 2. Public Words

Use these in Layer 1 docs and examples:

```text
candidate / program / artifact
train
validation / dev
test
case / example / task
score / metric
feedback / trace
reflection
budget
best
report
```

The user-facing explanation should be:

```text
Give Leaven a candidate, work to train on, a scorer, an optimizer, and a budget.
Optionally give it validation/test work. GEPA searches for a better candidate
and reports what happened.
```

## 3. GEPA Customizer Words

Use these for Layer 2:

```text
surface
parent selector
part selector
batch sampler
reflector / proposer
acceptance
population / frontier
validation cadence
merge
stopper
```

The parent/part distinction is:

```text
parent selector = which candidate to mutate next
part selector   = where inside that candidate to edit
```

Prefer `parent_selector` in GEPA-facing APIs. Use `candidate_selector` only for
general optimizer internals where the selected candidate is not necessarily a
proposal parent.

## 4. Lowered/Internal Words

These can be public Rust APIs, but they are not Layer 1 concepts:

```text
Dataset
DatasetSplits
SplitRole
SplitUse
EvaluationPlan
EvaluationRequestTemplate
EvaluationReport
EvaluationSet
EvaluationRequest
ResolvedEvaluationRequest
TrustPolicy
ReadScope
RunGraph
RunContext
Assessment
Evidence
EvidenceStore
```

The lowered eval crate should prefer:

| Avoid | Prefer | Why |
| --- | --- | --- |
| `EvalPlan` | `EvaluationPlan` | Lowered declaration; not public front door. |
| `EvaluationSpec` | avoid for now | Sounds like a new concept users must learn. |
| `EvalProtocol` | avoid | Suggests runtime protocol or extra crate split. |
| `CaseCatalog` | `Dataset` | ML-native; clear enough. |
| `EvalCase` | `Case` | Short, familiar, domain adapters can specialize. |
| `SplitManifest` | `DatasetSplits` | Names the concept, not storage. |
| `SplitPermissions` | `SplitUse` | Says how the split may be used; not ACL-coded. |
| `EvalUse` | `EvaluationUse` | Clearer and less abbreviated. |
| `LeakagePolicy` | engine `TrustPolicy` or builder lowering | Leakage is a failure mode, not a product object. |
| `EvalRunReport` | `EvaluationReport` | Familiar and direct. |
| `ReportAxis` | `MetricAxis` | ML-native. |

## 5. Words To Keep Out Of Layer 1

Avoid these in ordinary GEPA docs/examples:

```text
actor
visibility policy
trust scope
read scope
split permissions
request template
resolved set
graph mutation
evidence store
substrate
Harbor/AISI-like
```

They may appear in optimizer-author, engine, or agentic security docs. They
should not be necessary to understand how to run GEPA.

## 6. Crate Naming Implication

The naming split maps to crate ownership:

```text
leaven-run     public builder verbs: optimize/train/validation/test/score/run
leaven-gepa    GEPA strategy vocabulary: parent selector, part selector, etc.
leaven-eval    lowered dataset/split/report vocabulary
leaven-engine  execution vocabulary: requests, graph, trust, budget, callbacks
domain crates  artifact/evaluator/surface adapters
```

This keeps user-facing names intuitive while preserving the precise machinery
power users need.
