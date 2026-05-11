# Map-First AGENTS Completion Audit

Status: completion audit for the active 2026-05-11 hierarchy goal.

This audit checks the goal against actual artifacts and verification evidence.
It does not treat file count, `just check`, `topology_contract`, path sanity,
or old topology-spec agreement as sufficient completion proof by themselves.

## Objective Restated

Build a map-first Leaven `AGENTS.md` hierarchy that lets future agents route
from stacked context to:

- the correct owning crate or docs subtree;
- the neighboring owner that must refuse the work;
- the proof anchor or command for the relevant claim;
- known stale or bait paths, especially live-provider proof traps and stale
  topology inventory.

## Prompt-To-Artifact Checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Use `docs/plans/2026-05-11-map-first-agents/goal-handoff.yaml` as governing handoff. | Handoff exists, parses as YAML, and now records coverage, reviewer, rubric-audit, and verification evidence. | satisfied |
| Use `docs/AGENTSMD_INFO.md` and `/Users/darin/.agents/skills/hierarchical-agents-md/SKILL.md` as hierarchy standard. | `docs/plans/2026-05-11-map-first-agents/rubric-audit.md` records the qualitative rubric pass; handoff lists both standards. | satisfied |
| Use current `Cargo.toml` denominator, not stale topology spec alone. | `docs/plans/2026-05-11-map-first-agents/workspace-coverage.md` was checked against `cargo metadata --no-deps --format-version 1` with no missing package names. | satisfied |
| Classify every workspace crate as local, parent-covered, deferred, or quarantined. | Coverage matrix lists every Cargo workspace crate and example package; `leaven-dsrs` non-workspace directory is separately quarantined. | satisfied |
| Do not rely on sibling `AGENTS.md` for stacked context. | `crates/AGENTS.md` now carries childless provider/backend leaf rules; the matrix cites stacked sources or local files for those leaves. | satisfied |
| Preserve verbatim early subagent reports for compaction-safe handoff. | `docs/plans/2026-05-11-map-first-agents/subagent-reports-verbatim.md`. | satisfied |
| Catch stale topology and orphan DSRS bait. | Root, `crates/AGENTS.md`, `docs/specs/AGENTS.md`, `crates/leaven-dsrs/AGENTS.md`, and the coverage matrix mark `crates/leaven-dsrs` as non-workspace bait. | satisfied |
| Catch live-provider proof traps. | `examples/AGENTS.md`, `examples/p5_evoskill_iteration/AGENTS.md`, `examples/p8_aime_gepa/AGENTS.md`, and `docs/testing/AGENTS.md` distinguish deterministic, live, and cached-data paths. | satisfied |
| Add high-risk crate maps where local routing changes decisions. | Local files now exist for kernel, core, surface, engine, run, umbrella, evidence, eval, preference, population, GEPA, skill artifact, std, store/file, workspace/local, LM/cache/mock/OpenAI, agent/Codex/command/agentic, derive, and quarantine DSRS. | satisfied |
| Keep deferred crates honest rather than spraying files. | Coverage matrix marks skeleton/reserved provider/backend/optimizer/domain leaves as deferred, with promotion triggers in `crates/AGENTS.md`. | satisfied |
| Avoid completion by proxy. | Handoff explicitly forbids file count, old topology agreement, `just check`, `topology_contract`, and path sanity as standalone proofs. | satisfied |
| Run adversarial blind-routing review. | Reviewer Epicurus checked workspace coverage, blind routing samples, stale paths/commands, slop, and proof traps; targeted re-review confirmed all blockers resolved. | satisfied |
| Sanity-check important commands named by new guidance. | `cargo nextest run -p leaven-engine --test graph_surface`, `cargo nextest run -p leaven-agentic --test agentic_adapters --test agentic_workload --test repairing_proposer`, `cargo nextest run -p leaven-derive`, `cargo test -p leaven --test topology_contract`, and `just --dry-run milestone-examples` passed after final guidance edits. | satisfied |
| Run broad hygiene without treating it as sufficient proof. | `RUSTUP_TOOLCHAIN=nightly just check` passed during integrated hierarchy verification with line coverage `98.56%` and branch coverage `87.97%`; later edits were documentation/AGENTS-only and got targeted command checks. | satisfied |
| Account for current working-copy reality. | `jj st` shows intended hierarchy/docs changes plus unrelated `.idea` and `Cargo.lock` changes; unrelated files were not reverted. | satisfied |

## Remaining Risks

- The hierarchy is map-first and strong, not a final "god-tier" oral-tradition
  pass for every crate. `docs/plans/2026-05-11-map-first-agents/rubric-audit.md`
  records that landmarks are useful but uneven.
- Deferred crates must receive local files when real behavior lands or when a
  local hazard appears.
- `leaven-agent-codex-app-server` feature gates still fail on vendored Codex
  protocol drift. That is deliberately marked in the crate-local AGENTS file
  and matrix rather than hidden.

## Audit Verdict

The active goal's required map-first hierarchy, denominator, stale-bait
coverage, live-provider proof-trap coverage, adversarial review, and honest
verification record are complete. The remaining risks are future enrichment
or known out-of-scope implementation drift, not blockers for this hierarchy
goal.
