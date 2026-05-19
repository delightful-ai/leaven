## Boundary
This crate owns reusable population and fitted-state implementations: keep-best, Pareto/frontier variants, tournament/Bradley-Terry, Plackett-Luce, novelty, map-elites, beam, and no-population strategies.

Population code may consume evidence and emit `PopulationEvent`s. It must not mutate `RunGraph`; optimizers decide which assessments each population observes, and `RunContext` records the resulting graph events.

## Routing
- Put single-objective best tracking in `src/keep_best.rs`.
- Put fixed-capacity scalar frontier/admission behavior in
  `src/top_k_frontier.rs`.
- Put pairwise fitted state and tournament observation in `src/tournament.rs`; raw pairwise evidence belongs in `leaven-evidence`, while stateless preference helpers belong in `leaven-preference`.
- Put casewise and partition-aware frontier behavior in `src/pareto_frontier.rs`; GEPA-specific parent selection, gates, and part selection belong in `leaven-gepa`.
- Put reusable population configuration in this crate when it can serve more than one optimizer. Put optimizer-specific strategy state in the optimizer crate.

## Current Public-Maturity Split
- Behavior-bearing today: `KeepBest`, `TopKFrontier`, `TournamentPopulation` /
  `BradleyTerryFit`, and `ParetoFrontier` / `ParetoFrontierBuilder` have focused
  tests and emit `PopulationEvent`s without writing the graph.
- Public placeholders today: `BeamPopulation`, `MapElites`,
  `NicheDescriptor`, `NoveltyPopulation`, `NoPopulation`,
  `LenientParetoFrontier`, `PlackettLuceFit`, and `TournamentConfig` are
  production-looking names with little or no behavior. Treat them as scaffold
  until laws/tests land.

## Local Helper Stack
- Use `KeepBest` for single-objective scalar P1-style flows; tie policy is "do
  not replace" unless the implementation is explicitly changed and tested.
- Use `TopKFrontier` for EvoSkill-style bounded scalar frontiers: fill open
  capacity, then admit only candidates that beat the weakest current member.
  Equal scores do not evict existing members.
- Use `TournamentPopulation` for pairwise observations; `BradleyTerryFit`
  retains ability state and starts unseen candidates at zero.
- Use `ParetoFrontier::by_case().partition_filter(...)` for sparse casewise
  scalar frontiers. Missing case scores do not dominate present scores.
- Return `PopulationEvent`s to the caller; only `RunContext`/engine records them
  into run truth.

## Local Bait
- `PopulationEvent` is an engine type, but events returned here are strategy opinions, not direct graph writes.
- `PartitionFilter` affects which observations a frontier accepts; dataset split policy still belongs in `leaven-eval`, and trust/read-scope enforcement still belongs in `leaven-engine`.
- Fitted models such as `BradleyTerryFit` live here because they retain observation state. Do not move them to stateless `leaven-preference`.
- Public unit structs in this crate are under audit pressure. Implement or
  scaffold-gate them before letting `leaven-std` or examples present them as
  standard population implementations.

## Proof Anchors
- `cargo nextest run -p leaven-population` proves keep-best, top-k, tournament,
  and Pareto/frontier population laws, including finite fitted updates and
  partition filtering.
- `cargo nextest run -p leaven-gepa --test gepa_smoke` proves GEPA consumes population state without moving GEPA selectors or gates into this crate.
- `cargo nextest run -p leaven --test scalar_keep_best --test pairwise_tournament --test gepa_parity` proves mature population implementations participate in public end-to-end workflows through the umbrella surface. It is not proof for placeholder population names.
