# Map-First AGENTS Rubric Audit

Status: active qualitative audit for the 2026-05-11 hierarchy goal.

This checks the current hierarchy against `docs/AGENTSMD_INFO.md`, not just
against file count or path sanity.

## Rubric Read

The hierarchy now has strong orientation and routing power:

- root gives the horizon map and global bans;
- `crates/AGENTS.md` gives the crate-family map plus childless leaf guidance;
- high-risk seam crates have local route-away and proof anchors;
- docs/testing/examples/tooling subtrees have authority and proof-model files;
- `crates/leaven-dsrs` and `crates/leaven-lm-mock` are marked as bait instead
  of pretending every directory is normal precedent.

The first draft was useful but formulaic in exactly the way the rubric warns
about: many child files had `Boundary / Route Here / Route Away / Proof Anchors
/ Local Bait`, which is a good map but can under-serve decision-card and
landmark needs. The fix is not to make every file longer. The fix is to put
the richer playbook shape at high-traffic stack nodes and at seams where the
same bad move is likely to recur.

## Added After Rubric Pass

- Root `AGENTS.md` now says how to consume stacked context and run blind
  placement/refusal/imitation/verification/exception checks.
- `crates/AGENTS.md` now has decision cards for childless provider/backend
  leaves, skeleton-to-real transitions, and cross-crate moves.
- `crates/leaven-engine/AGENTS.md` now has decision cards for graph mutation
  and shared execution policy.
- `crates/leaven-agentic/AGENTS.md` now has decision cards for session-to-
  proposal parsing and cache/retry/repair behavior.

## Current Score Shape

- Orientation: strong. A future agent can identify root/docs/crates/examples
  roles and the major crate families.
- Routing power: strong. The main concept routes and route-away neighbors are
  explicit, including stale DSRS and live-provider proof traps.
- Landmarks: useful but still uneven. Many files name tests as proof anchors;
  only some name canonical implementation examples. This is acceptable for the
  first hierarchy pass because the user prioritized map over invariant density.
- Hazards: strong in high-risk crates and quarantine leaves.
- Proof closure: strong for known commands and proof traps; app-server feature
  gates are honestly marked as known-failing protocol drift gates.
- Zoom fit: improved after promoting childless leaf rules to `crates/AGENTS.md`
  so sibling files are not assumed to be stacked context.
- Delta efficiency: mostly good. Some similar section shapes remain, but the
  duplicated shape is carrying different local routing and proof content rather
  than boilerplate.
- Freshness: adequate. Root and hierarchy docs include on-touch maintenance;
  the coverage matrix records explicit deferred decisions.

## Residual Risks

- Deferred crates should be revisited when behavior lands. Their current
  parent coverage is intentional, not proof that they never need local files.
- Some leaf files could gain canonical example paths after the codebase settles
  further. Adding them now without local confidence would create false
  landmarks.
- The hierarchy is still a first serious pass. Future incident/review feedback
  should promote missing oral tradition into the nearest owning file.
