use std::collections::BTreeMap;
use std::time::Duration;

use leaven_public_seam::{
    PlanWorkspaceQueryOutcome, PlanWorkspaceQueryRequest, PublicSeamError, WorkspaceGitAgainst,
    WorkspaceQueryOp,
};
use leaven_workspace::{Command, WorkspacePath, WorkspaceView};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Git setup for configured local workspaces.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamWorkspaceGitConfig {
    /// Initialize the materialized workspace as a Git repository and commit seed files.
    pub initialize: bool,
    /// UTF-8 files written after the seed commit so diff/status queries have real workspace state.
    pub post_commit_files: BTreeMap<String, String>,
}

pub(crate) fn initialize_workspace_git(
    view: &mut WorkspaceView<'_>,
    config: &SeamWorkspaceGitConfig,
) -> Result<(), PublicSeamError> {
    run_git_command(view, &["init"], None)?;
    run_git_command(
        view,
        &["config", "user.email", "leaven@example.invalid"],
        None,
    )?;
    run_git_command(view, &["config", "user.name", "Leaven Seam"], None)?;
    run_git_command(view, &["add", "--all"], None)?;
    run_git_command(
        view,
        &["commit", "--allow-empty", "-m", "leaven workspace seed"],
        None,
    )?;
    for (path, contents) in &config.post_commit_files {
        let path = WorkspacePath::new(path).map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("invalid configured post-commit file path `{path}`: {error}"),
        })?;
        view.write_file(&path, contents.as_bytes())
            .map_err(|error| PublicSeamError::InvalidPlan {
                message: format!(
                    "failed to write post-commit file `{}`: {error}",
                    path.as_str()
                ),
            })?;
    }
    Ok(())
}

pub(crate) fn execute_git_workspace_query(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &mut WorkspaceView<'_>,
    graph_revision: String,
    data_classes: Vec<String>,
) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
    let value = match request.op_kind() {
        "git_log" => git_log_value(request, view)?,
        "git_diff" => git_diff_value(request, view)?,
        "git_status" => git_status_value(request, view)?,
        other => {
            return Err(PublicSeamError::InvalidPlan {
                message: format!("unknown Git workspace query `{other}`"),
            });
        }
    };
    Ok(PlanWorkspaceQueryOutcome::new(value, graph_revision)
        .with_data_classes(data_classes)
        .with_replayability("pure_read"))
}

fn git_log_value(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &mut WorkspaceView<'_>,
) -> Result<Value, PublicSeamError> {
    let WorkspaceQueryOp::GitLog { max_entries } = request.op() else {
        return Err(PublicSeamError::InvalidPlan {
            message: "workspace_query git_log must carry typed op".to_owned(),
        });
    };
    let max_entries = max_entries.unwrap_or(50);
    let max_entries_arg = max_entries.to_string();
    let text = run_git_command(
        view,
        &["log", "--oneline", "--decorate=no", "-n", &max_entries_arg],
        Some(64 * 1024),
    )?;
    Ok(json!({
        "kind": "workspace_diff",
        "text": text,
        "source_refs": [{
            "kind": "external",
            "namespace": "leaven.workspace.git_log.max_entries",
            "id": max_entries_arg
        }]
    }))
}

fn git_diff_value(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &mut WorkspaceView<'_>,
) -> Result<Value, PublicSeamError> {
    let WorkspaceQueryOp::GitDiff { against, max_bytes } = request.op() else {
        return Err(PublicSeamError::InvalidPlan {
            message: "workspace_query git_diff must carry typed op".to_owned(),
        });
    };
    let rev = match against {
        WorkspaceGitAgainst::Seed | WorkspaceGitAgainst::Baseline | WorkspaceGitAgainst::Head => {
            "HEAD"
        }
        WorkspaceGitAgainst::Parent => "HEAD~1",
    };
    let text = run_git_command(
        view,
        &["diff", "--no-ext-diff", "--text", rev, "--"],
        *max_bytes,
    )?;
    let against = against.as_str();
    Ok(json!({
        "kind": "workspace_diff",
        "text": text,
        "source_refs": [{
            "kind": "external",
            "namespace": "leaven.workspace.git_diff.against",
            "id": against
        }]
    }))
}

fn git_status_value(
    request: &PlanWorkspaceQueryRequest<'_>,
    view: &mut WorkspaceView<'_>,
) -> Result<Value, PublicSeamError> {
    let WorkspaceQueryOp::GitStatus { porcelain } = request.op() else {
        return Err(PublicSeamError::InvalidPlan {
            message: "workspace_query git_status must carry typed op".to_owned(),
        });
    };
    let porcelain = porcelain.unwrap_or(false);
    let args = if porcelain {
        &["status", "--porcelain=v1"][..]
    } else {
        &["status", "--short"][..]
    };
    let text = run_git_command(view, args, Some(64 * 1024))?;
    Ok(json!({
        "kind": "workspace_diff",
        "text": text,
        "source_refs": [{
            "kind": "external",
            "namespace": "leaven.workspace.git_status.porcelain",
            "id": if porcelain { "true" } else { "false" }
        }]
    }))
}

fn run_git_command(
    view: &mut WorkspaceView<'_>,
    args: &[&str],
    max_stdout_bytes: Option<u64>,
) -> Result<String, PublicSeamError> {
    let mut command = Command::new("git");
    command.args = args.iter().map(ToString::to_string).collect();
    command.limits.timeout = Some(Duration::from_secs(10));
    command.limits.max_stdout_bytes = max_stdout_bytes;
    command.limits.max_stderr_bytes = Some(64 * 1024);
    let output = view
        .run_command(command)
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("Git workspace query failed to start: {error}"),
        })?;
    if output.status.code != Some(0) {
        return Err(PublicSeamError::InvalidPlan {
            message: format!(
                "Git workspace query failed with status {:?}: {}",
                output.status.code,
                String::from_utf8_lossy(&output.stderr.bytes)
            ),
        });
    }
    if output.stdout.truncated {
        return Err(PublicSeamError::InvalidPlan {
            message:
                "Git workspace query exceeded configured stdout limit; host must provide blob_ref"
                    .to_owned(),
        });
    }
    String::from_utf8(output.stdout.bytes).map_err(|_| PublicSeamError::InvalidPlan {
        message: "Git workspace query produced non-utf8 output; host must provide blob_ref"
            .to_owned(),
    })
}
