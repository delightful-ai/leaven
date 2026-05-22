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
