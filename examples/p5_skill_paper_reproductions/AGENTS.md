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
  and rejects proxy completion claims. It also proves the schema v13
  `source_universe` links datasets to source artifact ids, available source
  revision ids, materialized row counts, source-row fingerprints, split ids,
  split exactness labels, source revision status fields, and blockers without
  treating local source identity or substitute splits as exact paper evidence.
  Optional schema-v1 `source_pin_manifest.json` input resolves only the
  `source_pin` blocker after id/path/head/branch/origin checks match the local
  git checkouts; mismatches fail the manifest build.
  Optional schema-v1 `split_policy_manifest.json` input accepts the current
  documented paper-close substitute split fingerprints only after
  dataset/split/source-row/split/role fingerprint checks match the materialized
  OfficeQA/SealQA reports. It removes split blockers without changing
  `paper_close_substitute` exactness; stale or partial policy manifests fail the
  build instead of becoming paper-exact proof.
  Optional `tmp/replication/evoskill/browsecomp/transfer_sample.jsonl` input
  materializes only a strict 128-row BrowseComp transfer denominator with
  source-id fingerprints and a held-out transfer split; it removes the
  BrowseComp source blocker without producing any score. The
  `--write-browsecomp-public-transfer-sample` operator path may derive that
  sidecar from the official encrypted BrowseComp CSV with a deterministic
  topic-stratified substitute policy; that is still `paper_close_substitute`,
  not author-exact sample membership.
  When source pins, accepted substitute splits, and the BrowseComp denominator
  remove all source blockers, the manifest/report top-level exactness becomes
  `paper_close_candidate`, not `paper_close`; live/judge blockers remain
  explicit and scores remain missing.
  The manifest also records OfficeQA baseline and skill-merge paper result
  targets, including the 67.9 versus 68.1 exact-match ambiguity, as typed
  `paper_result_targets` rather than a generic blocker. It also carries typed
  `source_blockers` for unresolved source policy, missing OfficeQA category and
  exact split artifacts, missing SealQA exact split membership, and absent
  BrowseComp transfer source, with checked local candidate-path evidence
  instead of bare path strings. The SealQA judge template pin also carries the
  checked paper-source artifact existence, size, and hash.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_scorer`
  proves the paper-specific Rust scorer laws for weighted multi-tolerance
  scoring, failure-threshold classification, unit normalization, incidental
  year filtering, hybrid text+number answers, multi-number answers, and
  normalized text containment. It also proves the SealQA paper auto-grader
  placeholder is pinned as a no-spend judge request template with source-backed
  manifest evidence and a stable fingerprint. It does not prove
  OfficeQA/SealQA split materialization, live
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
  manifests. It also proves the strict BrowseComp transfer JSONL sidecar lowers
  into a 128-row held-out denominator, proves the official encrypted BrowseComp
  CSV can produce a deterministic topic-stratified substitute sidecar in Rust,
  and refuses malformed row counts or duplicate source ids. SealQA row identity is physical row order, because the observed
  `canary` column is not unique. It keeps missing paper category/exact split
  artifacts as blockers unless the explicit paper-close split policy sidecar
  validates the current substitute fingerprints. Even then it does not prove
  exact OfficeQA/SealQA paper membership, SealQA judge scores, or live scores.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_loop_mechanics`
  proves a no-spend multi-iteration mechanics loop over the OfficeQA substitute
  split: train sampling, failure feedback history, round-robin parent
  selection, agentic Git materialize/readback of child proposals, typed
  `GitProgramArtifact` child lineage, full validation-role traversal before
  frontier admission, top-k admission/ignore/replacement, and
  checkpoint/resume. It does not prove live provider behavior, validation score
  quality, or paper scores.
- `cargo test -p p5_skill_paper_reproductions --test evoskill_final_report`
  proves the current no-spend final report truth surface writes manifest and
  report artifacts, carries baseline/optimized train/validation/held-out score
  slots, exposes zero spend by default, blockers, ablation statuses, and
  exactness gaps. It proves final report schema v15.
  Materialized score slots carry split exactness, split fingerprint, and
  role-level source-id fingerprint directly. Unreported slots carry a null
  `score_evidence_id` and null `score_evidence_artifact`; reported slots
  preserve the strict score sidecar entry's `evidence_id` as
  `score_evidence_id` and the checked evidence artifact path/hash/byte count as
  `score_evidence_artifact`. The embedded manifest carries
  typed `source_blockers`. The report also carries typed `paper_close_gates`
  that separate proven no-spend surfaces from source-blocked and
  approval-blocked gates, a `live_run_gate` that blocks live execution until
  explicit provider spend and credential approval, plus typed
  `proxy_rejection_gates` that reject known scaffolds and repo-health checks as
  completion evidence. Accepted substitute split policies turn OfficeQA slots
  into unscored `not_run` denominator slots and leave SealQA blocked on judge
  execution, without faking score values. A valid BrowseComp transfer sample
  turns its held-out transfer slots into unscored `not_run` denominator slots;
  if absent, they stay source-blocked. Blocked metrics stay missing rather
  than fake zeros. When source pins, accepted substitutes, and a BrowseComp
  transfer sidecar remove source blockers, ablation rows relabel those lanes as
  approval-blocked rather than continuing to cite absent source denominators.
  Optional `tmp/replication/evoskill/score_result_manifest.json` input can fill
  reported scores only after manifest/scorer/slot fingerprints, expected row
  counts, scored row counts, resolved slot blockers, a nonempty score evidence
  id, and a readable strict JSONL evidence artifact with matching SHA-256 and
  byte count match the current report. Schema-v3 evidence rows must exactly
  cover the slot role source ids, cannot duplicate or omit rows, must carry
  finite scores in `[0, 1]`, and must aggregate back to the reported score.
  OfficeQA score rows are also recomputed with the Rust paper scorer from the
  materialized hidden targets before import. Stale result files, tampered
  evidence artifacts, fabricated aggregate rows, or OfficeQA predictions whose
  scores do not match the scorer fail the report build instead of becoming
  score evidence.
  It does not prove live provider behavior, validation-score values, or
  paper-close.
- `just evoskill-paper-manifest` writes the current no-spend local manifest to
  `target/evoskill-paper-close/replica-manifest.json`.
- `just evoskill-paper-pin-local-sources` writes
  `tmp/replication/evoskill/source_pin_manifest.json` from the current local
  source checkout identities and then rewrites the manifest. This chooses the
  local-checkout source denominator only; it is not paper-release or
  remote-current evidence.
- `just evoskill-paper-accept-substitute-splits` writes
  `tmp/replication/evoskill/split_policy_manifest.json` from the current
  OfficeQA/SealQA substitute split fingerprints and then rewrites the manifest.
  This chooses a documented paper-close split denominator only; it is not
  paper-exact split membership or score evidence.
- `just evoskill-paper-browsecomp-public-sample <csv>` writes
  `tmp/replication/evoskill/browsecomp/transfer_sample.jsonl` from a local copy
  of the official encrypted BrowseComp CSV, then rewrites the manifest. This
  chooses a deterministic public BrowseComp substitute transfer denominator
  only; it is not the paper author's exact 128-example sample and it does not
  run the transferred SealQA skill.
- `just evoskill-paper-final-report` writes both the manifest and current
  no-spend final report truth surface under `target/evoskill-paper-close/`.
