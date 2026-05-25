## Boundary
This crate owns reusable population and fitted-state implementations: keep-best, Pareto/frontier variants, tournament/Bradley-Terry, and skill utility state.

Population code may consume evidence and emit `PopulationEvent`s. It must not mutate `RunGraph`; optimizers decide which assessments each population observes, and `RunContext` records the resulting graph events.

## Routing
- Put single-objective best tracking in `src/keep_best.rs`.
- Put fixed-capacity scalar frontier/admission behavior in
  `src/top_k_frontier.rs`.
- Put pairwise fitted state and tournament observation in `src/tournament.rs`; raw pairwise evidence belongs in `leaven-evidence`, while stateless preference helpers belong in `leaven-preference`.
- Put casewise and partition-aware frontier behavior in `src/pareto_frontier.rs`; GEPA-specific parent selection, gates, and part selection belong in `leaven-gepa`.
- Put optimizer-owned skill utility/use bookkeeping in `src/skill_utility.rs`.
  This module may know `leaven-artifact-skill::SkillName` because utility is
  keyed by validated skill identity, but it must not parse skill files,
  materialize workspaces, or decide paper-specific retrieval policy. It may
  rank caller-supplied relevance scores with utility and exploration state,
  plan D2Skill-style two-stage retrieval over caller-supplied route registries
  and similarity scores, and map paired rollout reward gaps onto skill utility
  credits. It may also plan utility-guided pruning for a caller-supplied active
  skill pool, and it may consume generic `SkillTrajectoryUseEvidence` once a
  runner/parser has already emitted it. Embeddings, route-key extraction,
  trajectory parsing, artifact mutation, and router training stay outside it.
- Put reusable population configuration in this crate when it can serve more than one optimizer. Put optimizer-specific strategy state in the optimizer crate.
- Reintroduce beam, no-population, novelty, map-elites, lenient Pareto,
  Plackett-Luce, or tournament-config names only with behavior-bearing laws and
  tests. The old placeholder unit structs were removed instead of exported
  through `leaven-std`.

## Current Public-Maturity Split
- Behavior-bearing today: `KeepBest`, `TopKFrontier`, `TopKParentSelector`,
  `TournamentPopulation` / `BradleyTerryFit`, `ParetoFrontier` /
  `ParetoFrontierBuilder`, and `SkillUtilityState` have focused tests and emit
  `PopulationEvent`s without writing the graph where applicable.
- The old public placeholders `BeamPopulation`, `MapElites`,
  `NicheDescriptor`, `NoveltyPopulation`, `NoPopulation`,
  `LenientParetoFrontier`, `PlackettLuceFit`, and `TournamentConfig` were
  deleted. They must not return as ordinary public exports until laws/tests
  land.

## Local Helper Stack
- Use `KeepBest` for single-objective scalar P1-style flows; tie policy is "do
  not replace" unless the implementation is explicitly changed and tested.
- Use `TopKFrontier` for EvoSkill-style bounded scalar frontiers: fill open
  capacity, then admit only candidates that beat the weakest current member.
  Equal scores do not evict existing members.
- Use `TopKParentSelector` for checkpointable deterministic parent selection
  over a `TopKFrontier`. `Best` reads the current score-ordered head without
  advancing cursor state; `RoundRobin` cycles across the current score order
  and stores the next index cursor. Random selection needs explicit RNG-state
  ownership before it becomes a reusable primitive.
- Use `TournamentPopulation` for pairwise observations; `BradleyTerryFit`
  retains ability state and starts unseen candidates at zero.
- Use `SkillUtilityState` for checkpointable skill utility bookkeeping:
  retrieval counts, trigger counts, EMA utility updates, explicit rename
  transfer, and explicit removal. Use `SkillUtilityRanker` when a caller
  already has finite relevance scores and needs utility/UCB-aware deterministic
  top-k ranking. Use `SkillTwoStageRetriever` when a caller has a
  `SkillRouteRegistry`, active route pool, caller-computed similarity scores,
  and D2Skill-style top-m/top-k retrieval configuration. Use
  `SkillPairedRolloutUtilityInput` when a caller has `PairedRolloutEvidence`,
  retrieved task skills, and either trajectory-level step credits or
  runner-provided `SkillStepTrajectoryOutcome`s / `SkillTrajectoryUseEvidence`
  and needs the D2Skill-style task gap and `Y_i - baseline_mean` step credit
  equation applied through
  `SkillUtilityState`. Use `SkillUtilityPruner` when a caller has one active
  skill pool, paper-provided capacity/current-step/protected-window values, and
  needs D2Skill-style eviction scores over stored utility and retrieval stats.
  Retrieval embeddings, paper route keys, transcript parsing, skill-bank
  artifact mutation, injection formatting, and paper-specific cadence/threshold
  selection remain outside this module.
- Use `ParetoFrontier::by_case().partition_filter(...)` for sparse casewise
  scalar frontiers. Missing case scores do not dominate present scores.
- Return `PopulationEvent`s to the caller; only `RunContext`/engine records them
  into run truth.

## Local Bait
- `PopulationEvent` is an engine type, but events returned here are strategy opinions, not direct graph writes.
- `PartitionFilter` affects which observations a frontier accepts; dataset split policy still belongs in `leaven-eval`, and trust/read-scope enforcement still belongs in `leaven-engine`.
- Fitted models such as `BradleyTerryFit` live here because they retain observation state. Do not move them to stateless `leaven-preference`.
- `SkillUtilityState` is optimizer/population state, not `SkillBank` artifact
  truth. If a future paper materializes utility into a candidate-visible
  registry, that registry needs artifact/cache identity outside this state.
- `SkillUtilityRanker` does not decide what a query means. It combines
  caller-provided relevance with utility and exploration state only.
- `SkillTwoStageRetriever` does not embed route keys or normalize similarity.
  It consumes a route registry plus caller-supplied finite similarities, applies
  threshold/top-m/top-k mechanics, and records selected retrievals only when the
  caller asks it to mutate `SkillUtilityState`.
- `SkillPairedRolloutUtilityInput` does not inspect transcripts or infer which
  step skills fired. The paper/runner supplies retrieved task skills and either
  step trajectory outcomes, generic skill-use trajectory evidence, or
  trajectory-level step credits; this crate only applies validated skill
  identities, finite reward gaps, smoothing, and checkpointable utility stats.
- `SkillUtilityPruner` plans removals but does not remove files, mutate a
  `SkillBank`, or decide which task/step pool is active. The caller supplies one
  pool at a time and applies the returned plan at the artifact/optimizer layer.
- New public population names are under audit pressure. Implement or
  scaffold-gate them before letting `leaven-std` or examples present them as
  standard population implementations.

## Proof Anchors
- `cargo nextest run -p leaven-population` proves keep-best, top-k frontier,
  top-k parent selector, tournament, skill utility state, paired rollout skill
  credit application, step-trajectory and skill-use-evidence credit extraction,
  two-stage routed skill retrieval, utility-guided skill pruning plans, and
  Pareto/frontier population laws, including finite fitted updates and partition
  filtering. It does not prove future beam, novelty, no-population, map-elites,
  lenient Pareto, Plackett-Luce, or tournament-config names.
- `cargo nextest run -p leaven-gepa --test gepa_smoke` proves GEPA consumes population state without moving GEPA selectors or gates into this crate.
- `cargo nextest run -p leaven --test scalar_keep_best --test pairwise_tournament --test gepa_parity` proves mature population implementations participate in public end-to-end workflows through the umbrella surface. It is not proof for placeholder population names.
