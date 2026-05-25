use crate::support::workspace_root;
use std::{collections::BTreeMap, path::Path};

use futures::future::BoxFuture;
use leaven_public_seam::{
    CapabilityDocument, PlanExecutionHost, PlanLmCompleteOutcome, PlanLmCompleteRequest,
    PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest, PlanWorkspaceQueryOutcome,
    PlanWorkspaceQueryRequest, PublicSeamError, PublicSeamPackage,
};
use leaven_workspace::{
    Command, CommandOutput, Workspace, WorkspaceBackend, WorkspaceError, WorkspacePath,
};
use serde_json::{Value, json};

#[test]
fn workspace_query_executes_finite_reads_through_workspace_view() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = WorkspaceQueryHost::new();

    let report = package
        .execute_plan_document_with_capability(
            &workspace_query_plan(),
            &plan_execution_context(),
            &workspace_query_capability(),
            &mut host,
        )
        .unwrap();
    let values = &report.value()["values"];

    assert_eq!(
        host.queries,
        vec![
            "file",
            "listing",
            "stat",
            "digest",
            "digest_blake3",
            "snapshot",
            "captured"
        ]
    );
    assert_eq!(values["file"]["content"], json!("hello seam\n"));
    assert_eq!(values["file"]["path"], json!("README.md"));
    assert_eq!(values["listing"]["entries"][0]["path"], json!("README.md"));
    assert_eq!(
        values["listing"]["entries"].as_array().unwrap().len(),
        1,
        "max_entries must bound the workspace-view helper result"
    );
    assert_eq!(values["stat"]["entries"][0]["bytes"], json!(11));
    assert!(
        values["digest"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        values["digest_blake3"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("blake3:")
    );
    assert!(
        values["snapshot"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("blake3:")
    );
    assert_eq!(values["captured"]["entries"][0]["path"], json!("README.md"));
}

#[test]
fn workspace_query_view_helper_rejects_unbounded_controls() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    for (name, op, expected) in [
        (
            "file",
            json!({
                "kind": "read_file",
                "path": "README.md",
                "max_bytes": 4,
                "expected_data_classes": ["candidate.artifact"]
            }),
            "workspace_query read_file exceeded max_bytes",
        ),
        (
            "captured",
            json!({
                "kind": "capture_artifacts",
                "paths": ["README.md"],
                "max_bytes": 4
            }),
            "workspace_query capture_artifacts exceeded max_bytes",
        ),
    ] {
        let mut host = WorkspaceQueryHost::new();
        let mut plan = workspace_query_plan();
        plan["ops"]
            .as_array_mut()
            .unwrap()
            .push(workspace_query_let_op(name, op));
        plan["return"] = json!([name]);

        let error = package
            .execute_plan_document_with_capability(
                &plan,
                &plan_execution_context(),
                &workspace_query_capability(),
                &mut host,
            )
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "unexpected error for {name}: {error:?}"
        );
    }
}

#[test]
fn workspace_query_view_helper_rejects_git_queries_as_host_owned() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    for (name, op, expected) in [
        (
            "log",
            json!({"kind": "git_log", "max_entries": 5}),
            "workspace_query `git_log` requires a host-provided Git workspace outcome",
        ),
        (
            "diff",
            json!({"kind": "git_diff", "against": "seed", "max_bytes": 4096}),
            "workspace_query `git_diff` requires a host-provided Git workspace outcome",
        ),
        (
            "status",
            json!({"kind": "git_status", "porcelain": true}),
            "workspace_query `git_status` requires a host-provided Git workspace outcome",
        ),
    ] {
        let mut host = WorkspaceQueryHost::new();
        let mut plan = workspace_query_plan();
        plan["ops"]
            .as_array_mut()
            .unwrap()
            .push(workspace_query_let_op(name, op));
        plan["return"] = json!([name]);

        let error = package
            .execute_plan_document_with_capability(
                &plan,
                &plan_execution_context(),
                &workspace_query_capability(),
                &mut host,
            )
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "unexpected error for {name}: {error:?}"
        );
    }
}

struct WorkspaceQueryHost {
    workspace: Workspace,
    queries: Vec<String>,
}

impl WorkspaceQueryHost {
    fn new() -> Self {
        let mut files = BTreeMap::new();
        files.insert(
            WorkspacePath::new("README.md").unwrap(),
            b"hello seam\n".to_vec(),
        );
        files.insert(
            WorkspacePath::new("src/lib.rs").unwrap(),
            b"pub fn answer() -> u8 { 42 }\n".to_vec(),
        );
        Self {
            workspace: Workspace::new(
                std::env::temp_dir().join("leaven-public-seam-workspace-query-test"),
                Box::new(MapWorkspaceBackend { files }),
            ),
            queries: Vec::new(),
        }
    }
}

impl PlanExecutionHost for WorkspaceQueryHost {
    fn lm_complete(
        &mut self,
        _request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        unreachable!("workspace query test does not execute lm calls")
    }

    fn workspace_materialize(
        &mut self,
        request: PlanWorkspaceMaterializeRequest<'_>,
    ) -> Result<PlanWorkspaceMaterializeOutcome, PublicSeamError> {
        Ok(PlanWorkspaceMaterializeOutcome::new(
            "ws_workspace_query_contract",
            request.lifetime()?,
            "fp_runtime_sha256_workspacequery",
        ))
    }

    fn workspace_query(
        &mut self,
        request: PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        self.queries.push(request.name().to_owned());
        request.execute_on_workspace_view(
            &self.workspace.view(),
            "rev_workspace_query_contract",
            ["candidate.artifact".to_owned(), "public".to_owned()],
        )
    }

    fn emit_run_event(
        &mut self,
        _request: leaven_public_seam::PlanEmitRunEventRequest<'_>,
    ) -> Result<leaven_public_seam::PlanEmitRunEventOutcome, PublicSeamError> {
        unreachable!("workspace query test does not emit events")
    }
}

struct MapWorkspaceBackend {
    files: BTreeMap<WorkspacePath, Vec<u8>>,
}

impl WorkspaceBackend for MapWorkspaceBackend {
    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| WorkspaceError::Io(format!("missing file `{}`", path.as_str())))
    }

    fn list_files(&mut self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        let root = path.as_str();
        Ok(self
            .files
            .keys()
            .filter(|file| {
                root.is_empty()
                    || file.as_str() == root
                    || file.as_str().starts_with(&format!("{root}/"))
            })
            .cloned()
            .collect())
    }

    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        self.files.insert(path.clone(), bytes.to_vec());
        Ok(())
    }

    fn run_command(&mut self, _command: Command) -> Result<CommandOutput, WorkspaceError> {
        Err(WorkspaceError::UnsupportedOperation {
            operation: "run_command",
        })
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        Box::pin(async { Ok(()) })
    }

    fn local_mount(&self) -> Option<&Path> {
        None
    }
}

fn workspace_query_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "workspacequerycontract001",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [
            {
                "kind": "call",
                "name": "workspace",
                "idempotency_key": "workspace-query-contract-materialize",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_workspacequery",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            workspace_query_let_op("file", json!({
                "kind": "read_file",
                "path": "README.md",
                "max_bytes": 64,
                "expected_data_classes": ["candidate.artifact"]
            })),
            workspace_query_let_op("listing", json!({
                "kind": "list",
                "path": ".",
                "recursive": false,
                "max_entries": 1
            })),
            workspace_query_let_op("stat", json!({"kind": "stat", "path": "README.md"})),
            workspace_query_let_op("digest", json!({
                "kind": "digest",
                "path": "README.md",
                "algorithm": "sha256"
            })),
            workspace_query_let_op("digest_blake3", json!({
                "kind": "digest",
                "path": "README.md",
                "algorithm": "blake3"
            })),
            workspace_query_let_op("snapshot", json!({"kind": "snapshot"})),
            workspace_query_let_op("captured", json!({
                "kind": "capture_artifacts",
                "paths": ["README.md"],
                "max_bytes": 64
            }))
        ],
        "return": ["file", "listing", "stat", "digest", "digest_blake3", "snapshot", "captured"],
        "commit": {"kind": "no_graph_writes"}
    })
}

fn workspace_query_let_op(name: &str, op: impl serde::Serialize) -> Value {
    json!({
        "kind": "let",
        "name": name,
        "deps": ["workspace"],
        "expr": {
            "kind": "workspace_query",
            "workspace": "ws_workspace_query_contract",
            "op": op
        }
    })
}

fn plan_execution_context() -> leaven_public_seam::PlanExecutionContext {
    leaven_public_seam::PlanExecutionContext::new(
        "fp_cap_sha256_workspacequery",
        "fp_policy_sha256_workspacequery",
        "rev_workspace_query_contract",
        "2026-05-24T12:00:00Z",
        "2026-05-24T12:00:01Z",
    )
}

fn workspace_query_capability() -> CapabilityDocument {
    CapabilityDocument::from_value(json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_workspacequery",
        "capability_fingerprint": "fp_cap_sha256_workspacequery",
        "policy_fingerprint": "fp_policy_sha256_workspacequery",
        "subject_fingerprint": "fp_subject_sha256_workspacequery",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_workspacequery",
            "stage_call_id": "sc_workspacequery",
            "role": "runner"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-24T00:00:00Z",
        "expires_at": "2026-05-24T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_workspacequery"
        },
        "revocation": {
            "mode": "issuer_epoch",
            "revocation_epoch": 7,
            "check": "on_every_request"
        },
        "renewal": {
            "mode": "renew_before_expiry",
            "max_extensions": 2,
            "max_total_lifetime_s": 3600
        },
        "grants": [
            {
                "action": "workspace.materialize",
                "resource": {
                    "candidate_ids": ["cand_workspacequery"]
                },
                "constraints": {
                    "workspace_ops": ["materialize"]
                }
            },
            {
                "action": "workspace.read",
                "resource": {
                    "workspace_ids": ["ws_workspace_query_contract"]
                },
                "constraints": {
                    "allowed_input_classes": ["candidate.artifact"],
                    "workspace_ops": [
                        "read_file",
                        "list",
                        "stat",
                        "digest",
                        "snapshot",
                        "git_log",
                        "git_diff",
                        "git_status",
                        "capture_artifacts"
                    ]
                }
            }
        ],
        "budgets": {},
        "execution_policy": {
            "profile": "managed_sandbox",
            "network": "leaven_endpoint_only",
            "subprocess": "deny_except_sandbox_exec",
            "filesystem": "workspace_handles_only",
            "byo_effects": "forbidden"
        },
        "delegation": {
            "may_delegate": false,
            "max_depth": 0,
            "must_attenuate": true,
            "allowed_actions": []
        }
    }))
    .unwrap()
}
