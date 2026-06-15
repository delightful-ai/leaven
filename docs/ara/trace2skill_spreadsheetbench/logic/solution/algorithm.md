# Algorithm

## Objective

Given an initial skill `S0`, fixed model agent `pi_theta`, evolving set
`D_evolve`, and held-out set `D_test`, construct an evolved skill `S*` from
trajectories over `D_evolve` such that held-out performance improves over `S0`.

The paper formalizes success rate as the average task correctness over a dataset
and defines skill evolution as constructing `S* = E(S0, D_evolve; pi_theta)`
without updating model parameters.

## Pseudocode

```text
input:
  initial skill S0
  model-backed agent pi_theta
  evolving set D_evolve
  held-out set D_test
  merge batch size B_merge
  analyst worker count W

stage 1:
  T = parallel_map(D_evolve, task -> run_agent(pi_theta, S0, task))
  T_minus = failures(T)
  T_plus = successes(T)

stage 2:
  P_minus = parallel_map(T_minus, tau -> error_analyst(pi_theta, S0, tau))
  P_plus = parallel_map(T_plus, tau -> success_analyst(pi_theta, S0, tau))
  P = valid(P_minus union P_plus)

stage 3:
  level = P
  while size(level) > 1:
      groups = batch(level, B_merge)
      level = parallel_map(groups, group -> merge_operator(pi_theta, S0, group))
      level = valid_non_conflicting(level)
  p_star = only(level)
  S_star = apply_patch(S0, p_star)

evaluation:
  score S0 and S_star on the selected validation/test/OOD denominator
```

## Complexity

The paper analysis states that with `W=128` workers and about `N=70` error
lessons, analysts execute in a single parallel round and the hierarchical merge
adds about `ceil(log2 N) ~= 7` further sequential rounds. The reported practical
runtime comparison is about 3 minutes for parallel consolidation versus about
60 minutes for Seq-B=1 and about 15 minutes for Seq-B=4.

## Leaven Stub

The typed stub in `src/execution/trace2skill_pipeline.py` captures the stage
shape and result-denominator records. It is not an executable paper reproduction
until connected to real run/eval/model artifacts.

The upstream executable entrypoints used by the generated full-denominator
runbook are pinned in `src/execution/upstream_code_manifest.json` by
repo-relative path, role, byte count, line count, and SHA-256. That manifest is
source identity evidence only; it does not prove the entrypoints have been run.
