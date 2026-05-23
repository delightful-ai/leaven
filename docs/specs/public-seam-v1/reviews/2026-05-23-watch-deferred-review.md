# ps1.watch.deferred adversarial review

Fresh evidence before review:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam deferred_watch`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`

Adversarial reviewer:

- Codex adversarial review in active goal thread `019e5486-a6b5-74e3-b137-d3572658090c`

Initial review result:

- No sign-off. The reviewer blocked closeout because the first proof validated a deferred marker and a hand-written `since_revision` plan as separate schema examples, which still allowed the fake pass of leaving a placeholder and claiming the example was a replacement route. It also asked for explicit subscription/streaming watch method negatives and complete matrix refs for the Plan schema and ACP profile used by the proof.

Fixes after review:

- Added `DeferredWatchReplacement` as an advanced public-seam contract.
- Added `PublicSeamPackage::validate_deferred_watch_replacement`, which validates the active deferred watch marker and Plan IR schema before requiring a finite event-diff Plan document through `consistency.since_revision`.
- Extended `PlanDocument` with revision and event-diff classification needed by the watch replacement validator.
- Added negative tests for schema-valid non-diff replacements: `latest_at_start`, `at_revision`, no event diff, and mismatched event base.
- Added runtime watch method negatives for start, subscribe, stream, next, ack, and close.
- Added the Plan schema and ACP profile to the row refs.

Follow-up review result:

- Sign-off granted. The reviewer found no blocking issues after the semantic replacement path was added, and confirmed the row can be marked proven after recording implementation, positive test, negative test, and review evidence.

Scope of sign-off:

- `ps1.watch.deferred` is signed off only for semantic V1 denial and replacement proof: the V1 marker routes to a finite `consistency.since_revision` Plan IR event diff, while runtime watch subscriptions, streaming, cursor/ack, lifecycle, backpressure, and delivery behavior remain deferred.
- This sign-off does not prove a watch runtime, ACP session lifecycle, graph execution, or RunContext revision enforcement; neighboring runtime rows remain pending until their own executable semantic proof and adversarial review exist.
