# GEPA Slot Contract

Status: integrated refinement pass.

This doc turns the GEPA customizer audit into a slot-by-slot contract. The
purpose is to keep GEPA swappable without making GEPA the whole library and
without leaking engine internals into Layer 1.

## Contract Table

| GEPA aspect | Layer 1 visible surface | Layer 2 customizer API | Lowered/private contract | Current state | Correction |
| --- | --- | --- | --- | --- | --- |
| Editable view | usually hidden by default or domain adapter | `.surface(surface)` | `EditSurface`, parts, part views, surface fingerprint | directionally healthy in `leaven-surface`; GEPA does not pass part view to reflector | pass owned selected part view into reflection request |
| Parent selection | hidden default by mode | `.parent_selector(...)` | selection context over population/frontier and graph view | public code says `CandidateSelector`; default name does not match behavior | public GEPA name is `ParentSelector`; implement or honestly rename default |
| Part selection | hidden default | `.part_selector(...)` | surface-scoped part choice with evidence/attribution view | placeholder worst-evidence behavior; no feedback context reaches selector | selector input must include selected evidence/trace view or precomputed attribution |
| Batch sampling | hidden default | `.batch_sampler(...)` | train/search minibatch selection with stable case IDs | listed in specs, mostly absent in public builder | add slot and typed request/output; reject empty required split |
| Reflection/proposal | `.with_reflection_lm(lm)` | `.reflector(...)` / `.proposer(...)` | async GEPA mutation request, renderer, LM/agent runtime, proposal finalization | `ReflectiveMutation` is a fixed edit fixture; local `SurfaceProposer` too narrow | reserve `ReflectiveMutation` for real evidence-aware reflection; quarantine fixtures |
| Acceptance | hidden default | `.acceptance(...)` | evidence/preference-aware admission decision | scalar `Gate(f64, f64)` | replace with `Acceptance` over assessments/evidence/preference summaries |
| Validation | `.validation(cases)` | `.validation(policy)` | held-out policy with explicit final/in-loop use | validation listed but under-audited | policy must say when validation can influence admission and report that use |
| Population/frontier | hidden default by mode | `.population(...)` | optimizer-private live state observing assessments/candidates | GEPA population tied to scalar casewise evidence | make observation evidence-shape-neutral; scalar is one adapter |
| Merge/crossover | no ordinary surface unless enabled | `.merge(...)` | scheduled proposer over two parents through same surface | placeholder public names | merge is another proposer schedule with explicit request, provenance, cost |
| Stopping/config | `.budget(...)` and maybe callbacks | `.stopper(...)` / `.config(...)` | budget, iteration, validation cadence, callback stop | config placeholder; stopper not clearly slotted | add typed config and stopper contract or remove names |

## Nomenclature Classification

### Keep As Public GEPA Names

- `ParentSelector`
- `PartSelector`
- `BatchSampler`
- `ReflectiveMutation`
- `Acceptance`
- `ValidationPolicy`
- `Population` / `ParetoFrontier`
- `Merge`

These match the original spec or optimizer literature closely enough to be
teachable.

### Accept Internally, Not As Public GEPA Names

- `CandidateSelector`: acceptable for generic lower-level candidate choice, but
  not for GEPA-facing parent selection.
- `Gate`: acceptable as an internal boolean helper name, but too narrow for the
  public acceptance/admission slot.
- `CachedLm`: acceptable as an advanced wrapper type, not as the ordinary
  runtime/cache story.

### Quarantine Or Remove Until Real

- fixed-edit `ReflectiveMutation`;
- `GepaConfig` placeholder;
- `MergeScheduler` placeholder;
- `SystemAwareMerge` placeholder;
- renderer names with no behavior.

## Private-State Discipline For Every Slot

Every swappable GEPA slot needs:

- request type;
- output type;
- context/capability boundary;
- budget/cost behavior;
- evidence/provenance behavior;
- event/report behavior where relevant;
- checkpoint/private-state story if it accumulates state;
- explicit `must not` rules for hidden split content and graph mutation.

Without these, a slot name is only a hole in the public API.
