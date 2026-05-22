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
  and rejects proxy completion claims.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_scorer`
  proves the paper-specific Rust scorer laws for weighted multi-tolerance
  scoring, failure-threshold classification, unit normalization, incidental
  year filtering, hybrid text+number answers, multi-number answers, and
  normalized text containment. It does not prove OfficeQA/SealQA split
  materialization or live agent performance.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_feedback`
  proves scorer-visible attempts turn into ordered failure-only feedback rows
  with source identity, expected/actual values, weighted score, and proposer
  feedback text. It does not prove a live proposer consumes that feedback.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_materialization`
  proves the OfficeQA CSV lowers through `leaven-eval::SourceRowManifest` into
  row-stable Leaven cases, records deterministic difficulty-stratified
  train/validation/test substitute split fingerprints, and keeps missing paper
  category/exact split artifacts as blockers. It does not prove SealQA row
  extraction, exact OfficeQA paper membership, or live scores.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_loop_mechanics`
  proves a no-spend multi-iteration mechanics loop over the OfficeQA substitute
  split: train sampling, failure feedback history, round-robin parent
  selection, typed `GitProgramArtifact` child lineage, top-k admission/ignore,
  and checkpoint/resume. It does not prove agentic Git workspace readback or
  live provider behavior.
- `just evoskill-paper-manifest` writes the current no-spend local manifest to
  `target/evoskill-paper-close/replica-manifest.json`.
