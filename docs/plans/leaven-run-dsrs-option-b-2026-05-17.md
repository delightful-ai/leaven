# Leaven Run DSRs Option B Tombstone

Date: 2026-05-17
Status: completed plan, landed as simplified hard cutover on 2026-05-20.

This path used to contain the DSRs Option B plan for typed `leaven-run` output.
The final Leaven-side behavior is the same typed-output hard cutover summarized
in `docs/plans/typed-run-output-2026-05-17.md`, without the originally proposed
renderer API or renderer-fingerprint compatibility axis.

DSRs did not consume this path at closeout; it used its own custom
`Evaluator<P>` bridge instead. If DSRs later uses the ordinary
`.runner(...).score(...)` path, verify against current `leaven-run` code and
tests rather than this old option plan.

Current truth lives in:

- `docs/specs/case_visibility_and_target_isolation.md`
- `docs/plans/dsrs-leaven-integration-2026-05-16.md`
- `crates/leaven-run`
- `crates/leaven-run/AGENTS.md`

Use this file as provenance only.

