# Workspace View Query Helper Review

Date: 2026-05-24

Scope:
- `ps1.workspace.handles_lifecycle`
- `crates/leaven-public-seam/src/plan_execution/queries.rs`
- `crates/leaven-public-seam/tests/workspace_query_contract.rs`
- `crates/leaven-public-seam/AGENTS.md`

Reviewer:
- Kant (`019e5ad1-1438-7d61-a56a-a68301624f00`)

Review method:
- Read-only adversarial semantic review.
- The reviewer was instructed not to treat rerunning the same tests as sign-off.
- Focus: spec drift, fake passes, missing negative tests, topology leaks,
  public-maturity overclaiming, path/handle lifecycle bypasses, and accidental
  invention of locked schema semantics.

Implementation claim reviewed:
- `PlanWorkspaceQueryRequest::execute_on_workspace_view` executes finite
  `read_file`, `list`, `stat`, `digest`, `snapshot`, and `capture_artifacts`
  workspace queries through `leaven_workspace::WorkspaceView`.
- The helper refuses `git_log`, `git_diff`, and `git_status` because the V1
  workspace substrate does not expose Git preimage fields.
- The tranche is partial evidence only for `ps1.workspace.handles_lifecycle`.

Initial findings:
- High: bounded controls were silently ignored. `read_file.max_bytes`,
  `list.recursive`, `list.max_entries`, and `capture_artifacts.max_bytes` were
  present in the locked schema but not enforced by the helper.
- Medium: `capture_artifacts` was being described as artifact capture even
  though the locked result shape projects it through `workspace_listing`.
- Medium: digest support was narrower than the locked request schema because
  only `sha256` was implemented while the schema also permits `blake3`.
- Low: Git refusal coverage exercised only `git_status`, not `git_log` and
  `git_diff`.

Resolutions:
- `read_file.max_bytes` now rejects oversized inline content and requires a
  host-provided bounded blob-ref outcome for that case.
- `list.recursive` and `list.max_entries` are enforced by the helper.
- `capture_artifacts.max_bytes` is enforced across captured file bytes, and the
  returned `workspace_listing` entries include byte counts.
- `digest` supports both `sha256` and `blake3`.
- Negative tests cover oversized `read_file`, oversized `capture_artifacts`,
  and refusal of all three Git query kinds.
- `AGENTS.md` narrows the claim to requested-path capture-artifact listing and
  keeps richer artifact bundles plus Git backend truth pending.

Follow-up sign-off:
- No findings remained on the prior blockers.
- The reviewer signed off adding partial pending-row evidence only.
- `ps1.workspace.handles_lifecycle` remains pending. This does not prove full
  Git/artifact/snapshot backend closeout or digest path-level result-schema
  proof.

Verification:
- `cargo test -p leaven-public-seam --test workspace_query_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document workspace_ -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
- `cargo test -p leaven --test topology_contract -- --nocapture`
- `cargo fmt --check`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`

Matrix status:
- `ps1.workspace.handles_lifecycle` remains `pending`.
