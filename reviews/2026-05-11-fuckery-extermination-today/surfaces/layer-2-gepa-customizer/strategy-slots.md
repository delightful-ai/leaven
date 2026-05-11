# Layer 2 GEPA Strategy Slots

Status: active findings recorded.

Layer 2 covers power users who want to swap GEPA strategy pieces without
forking the optimizer or losing access to the context those pieces need.

## Findings

### L2-001: The GEPA builder does not expose the promised strategy slots

- severity: high
- evidence: `docs/specs/gepa_optimizer_surface.md:273`,
  `crates/leaven-gepa/src/optimizer.rs:249-258`,
  `crates/leaven-gepa/src/optimizer.rs:663-713`,
  `crates/leaven-gepa/src/optimizer.rs:716-722`
- promised behavior: power users can swap parent selection, part selection,
  batch sampling, reflection/proposal, acceptance, validation policy,
  population/frontier, merge, and stopping behavior without forking GEPA.
- actual behavior: the public builder only supports `.surface(...)`,
  `.population(...)`, and `.reflector(...)`. More slots exist only through the
  low-level generic constructor, and config / merge types are placeholders.
- why it matters: Level 2 users have to drop into constructor/type gymnastics
  or cannot configure advertised slots at all.
- correction direction: either expose real builder methods for each promised
  slot or remove the advertised slot until implemented. The builder should be
  the Layer 2 customization surface, not an incomplete convenience.

### L2-002: Parent selection naming and behavior diverge

- severity: medium
- evidence: `docs/specs/eval_nomenclature.md:87`,
  `docs/specs/initial_library.md:3367`,
  `crates/leaven-gepa/src/selector.rs:34-96`
- promised behavior: GEPA-facing vocabulary calls this parent selection, and
  the default should match the paper-style frontier/frequency behavior or be
  named honestly.
- actual behavior: code exposes `CandidateSelector`. The default-looking
  `ParetoFrequencyWeighted` returns the population best deterministically.
- why it matters: users think they are customizing paper GEPA parent selection
  but get a differently named, simplified policy.
- correction direction: rename the public slot to `ParentSelector` and either
  implement the advertised policy or rename the current policy to describe its
  actual deterministic best-candidate behavior.

### L2-003: Acceptance is scalar-average only

- severity: high
- evidence: `docs/specs/gepa_public_private_surface.md:523`,
  `docs/specs/initial_library.md:1382`,
  `crates/leaven-gepa/src/gate.rs:23`,
  `crates/leaven-gepa/src/optimizer.rs:406`,
  `crates/leaven-preference/src/lib.rs:7`
- promised behavior: acceptance can be swapped over evidence/preference
  semantics, metric axes, parent/child summaries, validation policy, and
  comparable assessment context.
- actual behavior: `Gate` compares two `f64` average scores. Preference types
  are mostly public marker structs with no behavior.
- why it matters: pairwise, listwise, multi-axis, validation-aware,
  trace-aware, or claim-aware acceptance requires replacing the GEPA loop.
- correction direction: replace `Gate` with an `Acceptance` slot over
  candidate IDs, assessment/evidence refs, selected split purpose, and a
  preference relation.

### L2-004: Population strategy is tied to scalar casewise evidence

- severity: high
- evidence: `docs/specs/guiding_principles.md:127`,
  `crates/leaven-gepa/src/optimizer.rs:57-78`,
  `crates/leaven-population/src/tournament.rs:115`
- promised behavior: no-frontier, tournament, and custom frontier
  configurations are swappable without forking; evidence shape stays neutral.
- actual behavior: `GepaPopulation` observes only
  `CasewiseEvidence<ScalarEvidence>`. Richer population crate behavior cannot
  plug into GEPA as-is.
- why it matters: the population crate advertises broader concepts, but GEPA
  narrows them before users can use them.
- correction direction: separate population observation from scalar projection
  and route comparison through the preference/evidence contracts.
