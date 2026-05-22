## Boundary
This package is the no-spend skill-paper replication denominator surface. It is
not a live EvoSkill proof and must not be cited as paper-close by itself.

It owns paper-specific replica manifests, source-artifact probes, exactness-gap
classification, and small deterministic law tests for the five skill-paper
replication lane. Reusable split builders, samplers, artifact vocabulary,
frontier policy, workspace materialization, agent runtimes, and evidence types
belong in their owning crates.

## Local Rules
- Keep default commands no-spend and deterministic. Live provider/runtime work
  belongs behind explicit opt-in in the owning reproduction package.
- Manifest output is denominator evidence: source pins, artifact hashes,
  split/scorer/model/frontier config, and blockers. It is not score evidence.
- Do not use a manifest, a single sample probe, a fake runtime, or `just check`
  as completion evidence for paper-close replication.
- Keep Leaven-owned replication logic in Rust. Python scripts may remain source
  references only.
- If a generic primitive is needed, implement it in the owning crate before
  wiring it here.

## Proof
- `cargo test -p p5_skill_paper_reproductions --test evoskill_manifest`
  proves the EvoSkill replica manifest preserves the paper-close denominator
  and rejects proxy completion claims. It also proves the schema v7
  `source_universe` links datasets to source artifact ids, available source
  revision ids, materialized row counts, source-row fingerprints, split ids,
  split exactness labels, source revision status fields, and blockers without
  treating local source identity or substitute splits as exact paper evidence.
  The manifest also records OfficeQA baseline and skill-merge paper result
  targets, including the 67.9 versus 68.1 exact-match ambiguity, as typed
  `paper_result_targets` rather than a generic blocker.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_scorer`
  proves the paper-specific Rust scorer laws for weighted multi-tolerance
  scoring, failure-threshold classification, unit normalization, incidental
  year filtering, hybrid text+number answers, multi-number answers, and
  normalized text containment. It also proves the SealQA paper auto-grader
  placeholder is pinned as a no-spend judge request template with a stable
  fingerprint. It does not prove OfficeQA/SealQA split materialization, live
  judge execution, or live agent performance.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_feedback`
  proves scorer-visible attempts turn into ordered failure-only feedback rows
  with source identity, expected/actual values, weighted score, and proposer
  feedback text. It does not prove a live proposer consumes that feedback.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_materialization`
  proves the OfficeQA CSV lowers through `leaven-eval::SourceRowManifest` into
  row-stable Leaven cases, records deterministic difficulty-stratified
  train/validation/test substitute split fingerprints and role-level source-id
  membership manifests, lowers SealQA Parquet rows through
  `leaven-eval-parquet` into row-stable cases, and records a row-order 10
  percent train / held-out substitute split with role-level source-id membership
  manifests. SealQA row identity is physical row order, because the observed
  `canary` column is not unique. It keeps missing paper category/exact split
  artifacts as blockers and does not prove exact OfficeQA/SealQA paper
  membership, SealQA judge scores, or live scores.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_loop_mechanics`
  proves a no-spend multi-iteration mechanics loop over the OfficeQA substitute
  split: train sampling, failure feedback history, round-robin parent
  selection, agentic Git materialize/readback of child proposals, typed
  `GitProgramArtifact` child lineage, top-k admission/ignore/replacement, and
  checkpoint/resume. It does not prove live provider behavior or paper scores.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_final_report`
  proves the current no-spend final report truth surface writes manifest and
  report artifacts, carries baseline/optimized train/validation/held-out score
  slots, exposes zero spend, blockers, ablation statuses, and exactness gaps.
  Materialized score slots carry split exactness, split fingerprint, and
  role-level source-id fingerprint directly. The report also carries a
  `live_run_gate` that blocks live execution until explicit provider spend and
  credential approval, plus typed `proxy_rejection_gates` that reject known
  scaffolds and repo-health checks as completion evidence. Blocked metrics stay
  missing rather than fake zeros. It does not prove live provider behavior,
  validation-set scores, or paper-close.
- `just evoskill-paper-manifest` writes the current no-spend local manifest to
  `target/evoskill-paper-close/replica-manifest.json`.
- `just evoskill-paper-final-report` writes both the manifest and current
  no-spend final report truth surface under `target/evoskill-paper-close/`.
