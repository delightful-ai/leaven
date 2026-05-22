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
sidecar is the only no-spend import path for external scores. Schema-v3 entries
must match the current manifest fingerprint, scorer fingerprint, slot key, split
fingerprint, role source-id fingerprint, row counts, resolved blockers, a
nonempty `evidence_id`, and a readable JSONL evidence artifact with matching
SHA-256 and byte count. Each artifact row must name a source id, prediction, and
score; the importer checks exact role membership, rejects duplicate or missing
rows, recomputes the aggregate from rows, and replays the OfficeQA scorer before
reporting an OfficeQA score. Reported score slots preserve the id as
`score_evidence_id` and the checked artifact as `score_evidence_artifact`. It
is score evidence plumbing, not permission to treat fixtures, stale runs,
tampered evidence files, missing SealQA judge execution, missing transferred
BrowseComp runs, or missing provider approval as paper scores.
