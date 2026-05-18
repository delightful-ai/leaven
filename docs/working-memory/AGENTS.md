## Boundary
This subtree is Leaven's active goal ledger. It exists so future agents can
resume long-running work from durable, repo-local notes instead of lossy chat
context.

Working-memory files are not product law. They sit below `docs/specs`, code,
tests, and emitted run artifacts. Use them to find the next action and the
evidence trail, then verify the referenced current state before implementing or
claiming completion.

## Routing
- Use one file per active goal or investigation, named for the durable topic.
- Put live run handles, report paths, command lines, matrix rows, blockers, and
  verified next actions here.
- Put product semantics in `docs/specs`, not here.
- Put dated implementation breakdowns in `docs/plans`, not here.
- Put negative evidence from formal reviews in `reviews`, then link it here only
  when it drives the current goal.

## Update Rules
- Update the relevant ledger before ending a long turn, after a live run starts
  or fails, after a verifier/audit wave returns, or when the next action changes.
- Prefer append-only dated sections unless a statement is plainly obsolete; keep
  corrections explicit.
- Every claim should point at a concrete artifact: a spec section, test name,
  commit, report path, run directory, command output, or matrix row.
- Mark unproven items as unproven. A working-memory note must not convert a
  deterministic smoke, cache-only replay, or partial live failure into parity.

## Verification
- Doc-only updates: check referenced paths exist.
- If a ledger update changes a claimed product contract, update the owning spec
  or test in the same slice.
