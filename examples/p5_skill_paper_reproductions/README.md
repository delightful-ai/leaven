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
```

The generated manifest belongs under `target/evoskill-paper-close/` and is
evidence about the denominator, not evidence of paper-close scores.
