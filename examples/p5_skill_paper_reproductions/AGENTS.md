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
- `just evoskill-paper-manifest` writes the current no-spend local manifest to
  `target/evoskill-paper-close/replica-manifest.json`.
