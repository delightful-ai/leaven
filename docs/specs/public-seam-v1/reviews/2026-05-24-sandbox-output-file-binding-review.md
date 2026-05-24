# Sandbox Output File Binding Review

Date: 2026-05-24

Scope:
- `ps1.sandbox.exec_streaming`
- `crates/leaven-public-seam/src/plan_execution/effects.rs`
- `crates/leaven-public-seam/src/plan_execution.rs`
- `crates/leaven-workspace/src/command.rs`
- `crates/leaven-workspace/src/view.rs`
- `crates/leaven-workspace-local/src/factory.rs`
- `crates/leaven-workspace-git/src/factory.rs`

Reviewer:
- Parfit (`019e5a8e-c393-7d71-8491-6c09312df119`)

Review method:
- Read-only adversarial semantic review.
- The reviewer was instructed not to treat rerunning the same tests as sign-off.
- Focus: spec drift, fake passes, missing negative tests, topology leaks, and
  public-maturity overclaiming.

Implementation claim reviewed:
- `sandbox_exec` file outputs are no longer host-attached after the fact.
- `PlanSandboxExecRequest::to_workspace_command` lowers the output-file
  contract into `leaven-workspace::Command::output_files` and
  `CommandLimits::max_output_file_bytes`.
- Workspace backends capture requested output-file bytes into
  `CommandOutput::output_files`.
- `PlanSandboxExecOutcome::from_command_output_with_file_refs` requires each
  public file blob ref to bind to captured bytes and rejects missing, extra,
  duplicate, mismatched, and truncated file refs.
- `WorkspaceView::run_command` scopes requested file paths for backend
  execution and unscopes returned captured file keys back into caller-view
  coordinates.

Initial findings:
- Critical: `PlanSandboxExecOutcome::with_file_ref` remained public, so a host
  could still bypass the `CommandOutput` binding and attach logs afterward.
- Important: concrete workspace backends returned empty `output_files`, so the
  public seam had no production-adjacent captured bytes to bind.
- Minor: duplicate file refs needed explicit negative coverage.

Resolutions:
- `with_file_ref` and `with_stream_refs` are private; public host construction
  of stream/file-bearing sandbox outcomes must go through `from_command_output`
  or `from_command_output_with_file_refs`.
- Local and Git workspace backends capture requested output-file bytes from the
  mounted workspace after command completion and apply
  `max_output_file_bytes`.
- Duplicate, extra, missing, mismatched, truncated, and unsafe output-contract
  paths have executable negative coverage.

Follow-up finding:
- Important: `WorkspaceView::run_command` scoped requested `output_files` but
  returned captured file keys in backend coordinates, which would make subdir
  views falsely reject legitimate file refs during public-seam validation.

Follow-up resolution:
- `WorkspaceView::run_command` now unscopes returned `CommandOutput.output_files`
  keys into caller-view coordinates.
- `workspace_view_delegates_commands_to_backend_with_scoped_cwd` asserts the
  backend receives `candidate/out.txt` while the caller receives `out.txt`.
- `local_workspace_subdir_views_return_captured_files_in_view_coordinates`
  proves local subdir capture returns `reports/out.txt`, not
  `candidate/reports/out.txt`.

Reviewer sign-off:
- No critical or important blockers remain for this partial tranche.
- Parfit explicitly signed off the scoped `WorkspaceView` plus local-backend
  output-file capture as partial evidence, including view-coordinate return
  semantics.
- Full `ps1.sandbox.exec_streaming` closeout remains unproven: Firkin/live
  sandbox capture, ACP/streaming transport delivery, and production runtime
  proof are still outside this tranche.

Verification:
- `cargo test -p leaven-public-seam --test sandbox_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document sandbox -- --nocapture`
- `cargo test -p leaven-workspace-local --test local_workspace -- --nocapture`
- `cargo test -p leaven-workspace --test workspace_view workspace_view_delegates_commands_to_backend_with_scoped_cwd -- --exact --nocapture`
- `cargo fmt --check`

Matrix status:
- `ps1.sandbox.exec_streaming` remains `pending`.
