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
  exactness gaps. It proves final report schema v19.
  `exactness_gaps` is first-class report data: local source pins remain
  `paper_release_unverified`, accepted OfficeQA/SealQA/BrowseComp substitute
  splits remain `accepted_paper_close_substitute`, and unresolved source
  artifacts remain `blocked_before_paper_close`. This keeps
  `paper_close_candidate` visibly separate from paper-exact even when source
  blockers are otherwise resolved.
  Materialized score slots carry split exactness, split fingerprint, and
  role-level source-id fingerprint directly. Unreported slots carry a null
  `score_evidence_id`, null `score_evidence_kind`, null
  `score_evidence_approval_id`, and null `score_evidence_artifact`; reported
  slots preserve the strict score sidecar entry's `evidence_id` as
  `score_evidence_id`, the checked scoring method as `score_evidence_kind`, any
  required external judge approval id as `score_evidence_approval_id`, and the
  checked evidence artifact path/hash/byte count as `score_evidence_artifact`.
  The embedded manifest carries
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
  counts, scored row counts, score-resolvable blockers, a nonempty score
  evidence id, and a readable strict JSONL evidence artifact with matching
  SHA-256 and byte count match the current report. Source and split provenance
  blockers cannot be resolved by score sidecars; they must be resolved by their
  owning source/split manifests before score import. Schema-v5 entries must declare
  `score_evidence_kind`: `rust_scorer_replay` for OfficeQA scorer replay,
  `exact_answer_replay` for conservative BrowseComp answer checks, or
  `external_judge_run` for approved judge outputs. External judge entries must
  carry a nonempty `score_evidence_approval_id`, row-level
  `judge_template_fingerprint` values matching the pinned scorer template, and
  enough reported LLM calls for the judged rows. Evidence rows must exactly
  cover the slot role source ids, cannot duplicate or omit rows, must carry
  finite scores in `[0, 1]`, and must aggregate back to the reported score.
  OfficeQA score rows are recomputed with the Rust paper scorer from the
  materialized hidden targets before import. BrowseComp transfer rows are
  recomputed only with a conservative exact-normalized answer scorer against
  materialized scorer-only targets; this is not the official simple-evals
  BrowseComp judge path. Stale result files, tampered evidence artifacts,
  fabricated aggregate rows, OfficeQA predictions whose scores do not match the
  scorer, BrowseComp row scores that fail the exact-answer check, source/split
  blocker-resolution claims, or external judge rows without matching template
  fingerprints fail the report build instead of becoming score evidence.
  Report-level errors, ablations, and the
  `paper_scorer` gate are derived after score sidecar import: partial SealQA
  judge imports leave `sealqa_judge_scored_run` blocked, while complete approved
  external judge evidence for every SealQA score slot can clear that blocker
  without clearing the separate live-run approval gate.
  The `--write-officeqa-score-result` CLI path, exposed as
  `just evoskill-paper-score-officeqa <predictions.jsonl>`, writes that strict
  score sidecar from external OfficeQA prediction rows. It refuses blocked score
  slots, requires exact role coverage, recomputes row scores in Rust, and writes
  evidence rows without ground-truth/reference fields. It is not an agent run
  and must not be used to resolve missing split/source proof.
  The `--write-sealqa-judge-score-result` CLI path, exposed as
  `just evoskill-paper-score-sealqa <judged_rows.jsonl> <approval_id>`, writes
  the same strict sidecar from already-approved SealQA external judge rows. It
  requires an explicit approval id, current pinned judge-template fingerprints
  on every row, finite `[0, 1]` scores, exact role coverage, and slots whose
  only blocker is `sealqa_judge_scored_run`. It writes checked score-evidence
  JSONL and reported LLM-call cost through the normal importer. It is an import
  lane only; it does not run a judge, approve spend, resolve source/split
  blockers, or make partial SealQA evidence prove the full scorer gate.
  Score writer commands append disjoint score-slot batches to an existing
  validated `score_result_manifest.json`; they refuse duplicate slot keys before
  writing new evidence artifacts, so OfficeQA and SealQA evidence can accumulate
  without silent replacement.
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
- `just evoskill-paper-no-spend-packet [csv]` chains the current local source
  pin, accepted substitute split policy, BrowseComp public substitute, runner
  input, and live-run request steps. This is the one-command local approval
  packet path only; it does not probe credentials, approve spend, call a
  provider, call a judge, import predictions, import judged rows, or prove
  paper-close results.
- `just evoskill-paper-score-officeqa <predictions.jsonl>` writes
  `tmp/replication/evoskill/score_result_manifest.json` plus checked OfficeQA
  score evidence JSONL from strict prediction rows, then rewrites the manifest
  and final report. It scores only already-unblocked materialized OfficeQA
  slots.
- `just evoskill-paper-runner-inputs` writes
  `tmp/replication/evoskill/runner_input_manifest.json` plus answer-free
  OfficeQA/SealQA runner input JSONL for current unreported materialized
  slots, then rewrites the manifest and final report. It provides source ids
  and inputs only; it must not include targets, references, predictions,
  scores, or BrowseComp transfer rows, and it is not live-run proof.
- `just evoskill-paper-runner-outputs <outputs.jsonl>` imports strict runner
  prediction JSONL that names the current runner input artifact hash for each
  row. It validates the runner input manifest and exact source-id coverage,
  then scores OfficeQA outputs and prepares SealQA judge requests. It does not
  run the agent, call a judge, approve spend, or prove paper-close.
- `just evoskill-paper-live-run-request` writes
  `tmp/replication/evoskill/live_run_request_manifest.json` from the current
  runner input manifest. It is an approval packet with runtime/model, blocker,
  input artifact, and requested-call evidence only; it must not probe
  credentials, permit provider calls, call a judge, approve spend, or prove
  live execution.
- `just evoskill-paper-sealqa-judge-requests <predictions.jsonl>` writes
  `tmp/replication/evoskill/sealqa_judge_request_manifest.json` plus pinned
  judge-only request JSONL from strict SealQA prediction rows, then rewrites the
  manifest and final report. It prepares scorer prompts for future approved
  judge execution only; it does not write score evidence, call a judge, clear
  `sealqa_judge_scored_run`, or provide runner input.
- `just evoskill-paper-score-sealqa <judged_rows.jsonl> <approval_id>` writes
  `tmp/replication/evoskill/score_result_manifest.json` plus checked SealQA
  external judge score evidence JSONL from strict approved rows, then rewrites
  the manifest and final report. It imports only already-approved judge outputs
  for materialized SealQA slots whose only blocker is `sealqa_judge_scored_run`.
- Score writer commands append only disjoint score-slot entries to the current
  validated score sidecar. To replace a slot, rebuild or remove the sidecar
  explicitly; do not rely on later writer invocations to overwrite evidence.
- `just evoskill-paper-final-report` writes both the manifest and current
  no-spend final report truth surface under `target/evoskill-paper-close/`.
- `just evoskill-paper-closeout-audit` writes the same manifest/report and
  then fails unless every paper-close gate is proven. The current no-spend
  packet should fail this audit while `paper_scorer` and `live_small_run` are
  approval-blocked; do not bypass this failure as a flaky check.
