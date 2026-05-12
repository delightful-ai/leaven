# Layer 1 User Surface Canonical Audit

Layer 1 users should be able to run an optimizer without learning engine
actors, graph scopes, evidence stores, cache wrappers, or provider escape
hatches.

Current intended ordinary shape:

```rust
leaven::optimize(seed)
    .train(train)
    .validation(validation)
    .test(test)
    .runner(runner)
    .score(score)
    .using(Gepa::builder() /* configured for the artifact surface */)
    .budget(budget)
    .run()
    .await
```

The current canonical conclusion is: Layer 1 has a recognizable builder shell,
but the ordinary-user product contract is still blocked by proxy proof, sync
execution/scoring, unstable work identity, thin score/evidence/report truth,
missing runtime/cache roles, fixed GEPA reflection, and an ordinary prelude that
exports engine-author machinery.

## Canonical Layer 1 Audit Docs

- `root-cause-map.md`: Layer 1 root causes mapped to ideal contract, current
  implementation, user impact, correction direction, and proof requirements.
- `fix-priority-map.md`: ordered hard-cutover fixes and proof gates for Layer 1.
- `vision-comparison.md`: original Layer 1 vision compared against current repo
  reality.
- `surface-requirements.md`: exact ordinary-user public contract Layer 1 must
  satisfy.
