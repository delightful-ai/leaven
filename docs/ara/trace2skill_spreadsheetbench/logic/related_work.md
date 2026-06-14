# Related Work

## RW01: Anthropic Skills
- **DOI**: Not specified in paper.
- **Type**: imports.
- **Delta**: Trace2Skill targets Anthropic-style skill directories but automates creation/deepening from trajectories.
- **Claims affected**: C01, C03.

## RW02: SkillsBench
- **DOI**: Not specified in paper.
- **Type**: bounds.
- **Delta**: Provides evidence that curated skills help while self-generated skills can be weak; Trace2Skill addresses trajectory-grounded improvement.
- **Claims affected**: C01.

## RW03: SWE-Skills-Bench
- **DOI**: Not specified in paper.
- **Type**: motivates.
- **Delta**: Context mismatch can make skills harmful; Trace2Skill emphasizes generalizable patterns rather than per-instance overfitting.
- **Claims affected**: C01, C03.

## RW04: ReasoningBank
- **DOI**: Not specified in paper.
- **Type**: baseline.
- **Delta**: Stores and retrieves reasoning memories; Trace2Skill distills the same kind of trajectory evidence into a portable skill.
- **Claims affected**: C03.

## RW05: EvoSkill
- **DOI**: Not specified in paper.
- **Type**: closest neighbor.
- **Delta**: Iteratively diagnoses failures and validates skill updates; Trace2Skill emphasizes many-to-one consolidation over many independent patches.
- **Claims affected**: C01, C02, C04.

## RW06: Memento-Skills
- **DOI**: Not specified in paper.
- **Type**: neighboring system.
- **Delta**: Uses stateful markdown skills updated incrementally through a read-write loop; Trace2Skill emphasizes holistic parallel consolidation.
- **Claims affected**: C02.

## RW07: SkillWeaver, AutoSkill, XSkill
- **DOI**: Not specified in paper.
- **Type**: related skill-evolution family.
- **Delta**: These systems evolve or organize skills/memories; Trace2Skill claims comprehensive declarative skill directories with no test-time retrieval.
- **Claims affected**: C03, C04.
