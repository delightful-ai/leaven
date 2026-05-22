# Skill Paper Reproductions

Status: no-spend denominator harness.

This package starts the paper-close replication lane without pretending that
the paper has already been reproduced. The current implemented surface writes
an EvoSkill replica manifest that records source artifacts, split blockers,
paper scorer/frontier settings, model-pin gaps, and proxy proofs to reject.

Commands:

```bash
cargo test -p p5_skill_paper_reproductions --test evoskill_manifest
cargo test -p p5_skill_paper_reproductions --test evoskill_materialization
cargo test -p p5_skill_paper_reproductions --test evoskill_scorer
cargo test -p p5_skill_paper_reproductions --test evoskill_feedback
cargo test -p p5_skill_paper_reproductions --test evoskill_loop_mechanics
just evoskill-paper-manifest
just evoskill-paper-pin-local-sources
just evoskill-paper-accept-substitute-splits
just evoskill-paper-browsecomp-public-sample tmp/replication/evoskill/browsecomp/public_browsecomp_test_set.csv
just evoskill-paper-score-officeqa tmp/replication/evoskill/predictions/officeqa.jsonl
just evoskill-paper-score-sealqa tmp/replication/evoskill/judge/sealqa.jsonl approved-run-id
just evoskill-paper-final-report
```

The generated manifest belongs under `target/evoskill-paper-close/` and is
evidence about the denominator, not evidence of paper-close scores.
`just evoskill-paper-pin-local-sources` writes the ignored
`tmp/replication/evoskill/source_pin_manifest.json` sidecar for the current
local source checkouts; it chooses a local-checkout denominator, not a
paper-release or remote-current source policy.
`just evoskill-paper-accept-substitute-splits` writes the ignored
`tmp/replication/evoskill/split_policy_manifest.json` sidecar from the current
OfficeQA/SealQA substitute split fingerprints. It accepts those documented
substitutes as the paper-close split denominator without relabeling them as
paper-exact or producing scores.

The optional ignored
`tmp/replication/evoskill/browsecomp/transfer_sample.jsonl` sidecar is a strict
128-row BrowseComp transfer denominator. Each JSONL row must carry
`source_id`, `question`, and `answer`, with optional `stratum`. When present it
only materializes held-out transfer score slots as unscored `not_run` rows; it
does not reproduce the paper's 43.5/48.8 percent scores.

If the paper author's exact BrowseComp transfer sample is still absent, the
operator can create a declared paper-close substitute from the official
encrypted BrowseComp CSV:

```bash
mkdir -p tmp/replication/evoskill/browsecomp
curl -fsSL https://openaipublic.blob.core.windows.net/simple-evals/browse_comp_test_set.csv \
  -o tmp/replication/evoskill/browsecomp/public_browsecomp_test_set.csv
just evoskill-paper-browsecomp-public-sample tmp/replication/evoskill/browsecomp/public_browsecomp_test_set.csv
```

That command decrypts the local CSV in Rust, selects a deterministic
topic-stratified 128-row substitute sidecar, and keeps BrowseComp slots
unscored. It is not paper-exact sample membership.

The optional ignored `tmp/replication/evoskill/score_result_manifest.json`
sidecar is the only no-spend import path for external scores. Schema-v5 entries
must match the current manifest fingerprint, scorer fingerprint, slot key, split
fingerprint, role source-id fingerprint, row counts, resolved blockers, a
nonempty `evidence_id`, and a readable JSONL evidence artifact with matching
SHA-256 and byte count. Each artifact row must name a source id, prediction, and
score; the importer checks exact role membership, rejects duplicate or missing
rows, recomputes the aggregate from rows, and requires `score_evidence_kind`.
Score evidence cannot resolve source or split provenance blockers; those must
be cleared by the source pin, exact split, or accepted substitute split policy
manifests first. The current resolver only lets approved SealQA external-judge
evidence clear `sealqa_judge_scored_run`.
`rust_scorer_replay` replays the OfficeQA scorer before reporting an OfficeQA
score. `exact_answer_replay` checks BrowseComp transfer rows with a conservative
exact-normalized answer scorer when materialized targets are present. The
BrowseComp exact-answer check rejects fabricated row scores; it is not the
official simple-evals judge path. `external_judge_run` requires a nonempty
`score_evidence_approval_id`, reported LLM-call cost, and row-level
`judge_template_fingerprint` values matching the pinned scorer template for the
dataset. Reported score slots preserve the id as `score_evidence_id`, the
scoring kind, the optional approval id, and the checked artifact as
`score_evidence_artifact`. Report-level errors, ablations, and the
`paper_scorer` gate are recalculated after sidecar import, but only complete
approved external judge evidence for every SealQA score slot clears the
`sealqa_judge_scored_run` blocker. Partial imports stay blocked, and live-run
approval remains separate. It is score evidence plumbing, not permission to
treat fixtures, stale runs, tampered evidence files, missing SealQA judge
execution, missing transferred BrowseComp runs, official BrowseComp judge
approval, or missing provider approval as paper scores.

`just evoskill-paper-score-officeqa <predictions.jsonl>` is the Rust
operator path for OfficeQA scorer replay. Each prediction row must be strict
JSONL with `dataset_id`, `split_id`, `split_role`, `candidate_role`,
`source_id`, and `prediction`. The writer groups rows by score slot, refuses
blocked slots, requires exact role membership, computes row scores with the
Rust EvoSkill scorer, writes checked score-evidence JSONL without ground-truth
or reference fields, writes schema-v5 `score_result_manifest.json`, and then
regenerates the final report through the normal importer. This scores supplied
prediction artifacts only; it does not run an agent or resolve missing split
proof.

`just evoskill-paper-runner-inputs` is the Rust no-spend materialization path
for answer-free OfficeQA/SealQA runner inputs. It writes
`runner_input_manifest.json` plus per-slot JSONL artifacts for the current
unreported OfficeQA/SealQA slots whose source and split membership are
materialized. The artifacts carry source ids and inputs only; they deliberately
omit targets, references, predictions, scores, and BrowseComp transfer rows.
This prepares bounded run inputs only; it does not run an agent, score outputs,
call a judge, or prove live/paper-close results.

`just evoskill-paper-runner-outputs <outputs.jsonl>` is the Rust no-spend
import path for predictions produced from the current runner input manifest.
Each row is strict JSONL with `dataset_id`, `split_id`, `split_role`,
`candidate_role`, `input_artifact_sha256`, `source_id`, and `prediction`.
The importer validates the current `runner_input_manifest.json`, checks the
referenced input artifact hash and exact source-id coverage, scores OfficeQA
outputs with the Rust scorer, and materializes SealQA judge-request batches
for future approved judging. This imports runner outputs only; it does not run
an agent, call a judge, approve spend, or prove live/paper-close results.

`just evoskill-paper-live-run-request` is the Rust no-spend approval-packet
path for a bounded live run over the current runner inputs. It requires the
current `runner_input_manifest.json`, validates its artifacts, and writes
`live_run_request_manifest.json` with manifest/scorer fingerprints, runtime
role, candidate model, blocker ids, input artifact hashes, and requested
generation-call counts. It does not probe credentials, permit provider calls,
call a judge, approve spend, or prove live/paper-close results.

`just evoskill-paper-sealqa-judge-requests <predictions.jsonl>` is the Rust
no-spend request-materialization path for future approved SealQA judge runs.
Each input row is strict JSONL with `dataset_id`, `split_id`, `split_role`,
`candidate_role`, `source_id`, and `prediction`. The writer refuses
source/split-blocked slots and already-reported slots, requires exact role
coverage, binds every row to the current pinned judge-template fingerprint, and
writes `sealqa_judge_request_manifest.json` plus judge-only request JSONL. The
request artifact may contain reference answers because it is scorer input, not
runner input; it does not write score evidence, call a judge, or clear
`sealqa_judge_scored_run`.

`just evoskill-paper-score-sealqa <judged_rows.jsonl> <approval_id>` is the
Rust no-spend import path for already-approved SealQA external judge outputs.
Each row must be strict JSONL with `dataset_id`, `split_id`, `split_role`,
`candidate_role`, `source_id`, `prediction`, `score`, and the
`judge_template_fingerprint` used by the run. The writer refuses missing
approval ids, non-SealQA rows, stale judge-template fingerprints, source/split
blocked slots, duplicate or missing source ids, and out-of-range scores. It
writes checked score-evidence JSONL, records LLM-call cost from the judged row
count, writes schema-v5 `score_result_manifest.json`, and regenerates the final
report through the normal importer. This imports approved score evidence only;
it does not call a judge, approve spend, or make partial SealQA evidence clear
the full `sealqa_judge_scored_run` blocker.

Score writers append disjoint score-slot batches to the existing
`score_result_manifest.json` after validating the current sidecar. This lets
OfficeQA scorer replay and approved SealQA judge outputs accumulate in one
report. A batch that targets an already-reported slot is refused before new
evidence artifacts are written; replacement requires rebuilding the sidecar
explicitly.

The final report also carries first-class `exactness_gaps`. When local source
pins, accepted substitute splits, and the BrowseComp substitute denominator make
the report a `paper_close_candidate`, those gap rows still say which source
revisions are only local-checkout pins and which OfficeQA/SealQA/BrowseComp
splits are documented substitutes rather than paper-exact membership.
