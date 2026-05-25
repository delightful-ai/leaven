use crate::support::workspace_root;
use leaven_public_seam::{
    CapabilityDocument, PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext,
    PlanExecutionHost, PlanLmCompleteOutcome, PlanLmCompleteRequest,
    PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest, PlanWorkspaceQueryOutcome,
    PlanWorkspaceQueryRequest, PlanWorkspaceReleaseOutcome, PlanWorkspaceReleaseRequest,
    PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

const WORKSPACE_ID: &str = "ws_object_ref_contract";
const RUN_ID: &str = "run_object_ref_contract";
const RUN_ID_2: &str = "run_object_ref_contract_second";
const SNAPSHOT: &str = "fp_snapshot_sha256_objectrefcontract";
const SNAPSHOT_2: &str = "fp_snapshot_sha256_objectrefcontractsecond";

#[test]
fn workspace_lifecycle_preserves_and_enforces_object_refs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = WorkspaceRefHost::default();

    let report = package
        .execute_plan_document(
            &workspace_object_ref_plan(&workspace_ref(RUN_ID, SNAPSHOT)),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap();

    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert_eq!(
        report.value()["values"]["workspace"]["workspace"],
        workspace_ref(RUN_ID, SNAPSHOT)
    );
    assert_eq!(
        report.value()["values"]["release"]["workspace"],
        workspace_ref(RUN_ID, SNAPSHOT)
    );
    assert_eq!(report.value()["values"]["release"]["released"], true);
}

#[test]
fn workspace_lifecycle_rejects_object_ref_run_substitution_before_release_host_work() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = WorkspaceRefHost::default();

    let error = package
        .execute_plan_document(
            &workspace_object_ref_plan(&workspace_ref("run_wrong_workspace", SNAPSHOT)),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("workspace_release refused unmaterialized workspace"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.calls, vec!["workspace_materialize"]);
}

#[test]
fn workspace_lifecycle_does_not_collapse_same_id_distinct_object_refs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = WorkspaceRefHost::default();

    let report = package
        .execute_plan_document(
            &same_id_distinct_object_ref_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap();

    assert_eq!(
        host.calls,
        vec![
            "workspace_materialize",
            "workspace_materialize",
            "workspace_release",
            "workspace_release"
        ]
    );
    assert_eq!(
        report.value()["values"]["release_first"]["workspace"],
        workspace_ref(RUN_ID, SNAPSHOT)
    );
    assert_eq!(
        report.value()["values"]["release_second"]["workspace"],
        workspace_ref(RUN_ID_2, SNAPSHOT_2)
    );
}

#[test]
fn workspace_lifecycle_rejects_bare_id_release_of_object_ref_handle() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = WorkspaceRefHost::default();

    let error = package
        .execute_plan_document(
            &workspace_object_ref_plan(&json!(WORKSPACE_ID)),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("workspace_release refused unmaterialized workspace"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.calls, vec!["workspace_materialize"]);
}

#[test]
fn workspace_query_preserves_object_ref_receipt_preimage() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = WorkspaceRefHost::default();

    let report = package
        .execute_plan_document_with_capability(
            &workspace_object_ref_query_plan(&workspace_ref(RUN_ID, SNAPSHOT)),
            &plan_execution_context(),
            &workspace_ref_capability(),
            &mut host,
        )
        .unwrap();

    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_read_file"]
    );
    assert_eq!(
        report.value()["values"]["workspace"]["workspace"],
        workspace_ref(RUN_ID, SNAPSHOT)
    );
    assert_eq!(
        report.value()["values"]["file"]["path"].as_str(),
        Some("README.md")
    );
}

#[derive(Default)]
struct WorkspaceRefHost {
    calls: Vec<&'static str>,
}

impl PlanExecutionHost for WorkspaceRefHost {
    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        Err(unexpected_operation("lm_complete", request.name()))
    }

    fn workspace_materialize(
        &mut self,
        request: PlanWorkspaceMaterializeRequest<'_>,
    ) -> Result<PlanWorkspaceMaterializeOutcome, PublicSeamError> {
        assert_eq!(request.lifetime()?, "manual_release");
        self.calls.push("workspace_materialize");
        let (run, snapshot) = match request.candidate()? {
            "cand_object_ref" => (RUN_ID, SNAPSHOT),
            "cand_object_ref_second" => (RUN_ID_2, SNAPSHOT_2),
            other => panic!("unexpected candidate {other}"),
        };
        Ok(PlanWorkspaceMaterializeOutcome::new(
            WORKSPACE_ID,
            request.lifetime()?,
            "fp_runtime_sha256_workspaceobjectref",
        )
        .with_workspace_object_ref(Some(run), Some(snapshot)))
    }

    fn workspace_release(
        &mut self,
        request: PlanWorkspaceReleaseRequest<'_>,
    ) -> Result<PlanWorkspaceReleaseOutcome, PublicSeamError> {
        assert_eq!(request.live_workspace()?, WORKSPACE_ID);
        self.calls.push("workspace_release");
        let workspace = request.call()["workspace"]
            .as_object()
            .expect("workspace release test uses object refs");
        let run = workspace
            .get("run")
            .and_then(Value::as_str)
            .expect("workspace release object carries run");
        let snapshot = workspace
            .get("snapshot_fingerprint")
            .and_then(Value::as_str)
            .expect("workspace release object carries snapshot");
        Ok(PlanWorkspaceReleaseOutcome::new(
            WORKSPACE_ID,
            "manual_release",
            "fp_runtime_sha256_workspaceobjectref",
        )
        .with_workspace_object_ref(Some(run), Some(snapshot)))
    }

    fn workspace_query(
        &mut self,
        request: PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        assert_eq!(request.workspace(), WORKSPACE_ID);
        assert_eq!(request.op_kind()?, "read_file");
        assert_eq!(request.path()?, Some("README.md"));
        self.calls.push("workspace_read_file");
        Ok(PlanWorkspaceQueryOutcome::new(
            json!({
                "kind": "workspace_file",
                "path": "README.md",
                "content": "object ref read"
            }),
            "rev_workspaceobjectref_base",
        )
        .with_data_classes(["candidate.artifact".to_owned(), "public".to_owned()]))
    }

    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError> {
        Err(unexpected_operation("emit_run_event", request.name()))
    }
}

fn unexpected_operation(kind: &str, name: &str) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: format!("unexpected {kind} operation `{name}` in workspace ref contract host"),
    }
}

fn workspace_object_ref_plan(release_workspace: &Value) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planworkspaceobjectref001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            {
                "kind": "call",
                "name": "workspace",
                "idempotency_key": "workspace-object-ref-materialize-0001",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_object_ref",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "call",
                "name": "release",
                "deps": ["workspace"],
                "idempotency_key": "workspace-object-ref-release-0001",
                "call": {
                    "kind": "workspace_release",
                    "workspace": release_workspace,
                    "force": false
                }
            }
        ],
        "return": ["workspace", "release"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn workspace_object_ref_query_plan(query_workspace: &Value) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planworkspaceobjectrefquery001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            {
                "kind": "call",
                "name": "workspace",
                "idempotency_key": "workspace-object-ref-query-materialize-0001",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_object_ref",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "let",
                "name": "file",
                "deps": ["workspace"],
                "expr": {
                    "kind": "workspace_query",
                    "workspace": query_workspace,
                    "op": {
                        "kind": "read_file",
                        "path": "README.md",
                        "expected_data_classes": ["candidate.artifact"]
                    }
                }
            }
        ],
        "return": ["workspace", "file"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn same_id_distinct_object_ref_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plansameidobjectrefs001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            {
                "kind": "call",
                "name": "workspace_first",
                "idempotency_key": "same-id-object-ref-materialize-0001",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_object_ref",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "call",
                "name": "workspace_second",
                "idempotency_key": "same-id-object-ref-materialize-0002",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_object_ref_second",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "call",
                "name": "release_first",
                "deps": ["workspace_first", "workspace_second"],
                "idempotency_key": "same-id-object-ref-release-0001",
                "call": {
                    "kind": "workspace_release",
                    "workspace": workspace_ref(RUN_ID, SNAPSHOT),
                    "force": false
                }
            },
            {
                "kind": "call",
                "name": "release_second",
                "deps": ["workspace_first", "workspace_second"],
                "idempotency_key": "same-id-object-ref-release-0002",
                "call": {
                    "kind": "workspace_release",
                    "workspace": workspace_ref(RUN_ID_2, SNAPSHOT_2),
                    "force": false
                }
            }
        ],
        "return": ["release_first", "release_second"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn workspace_ref(run: &str, snapshot_fingerprint: &str) -> Value {
    json!({
        "kind": "workspace",
        "run": run,
        "id": WORKSPACE_ID,
        "snapshot_fingerprint": snapshot_fingerprint
    })
}

fn workspace_ref_capability() -> CapabilityDocument {
    CapabilityDocument::from_value(json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_workspace_object_ref",
        "capability_fingerprint": "fp_cap_sha256_workspaceobjectref",
        "policy_fingerprint": "fp_policy_sha256_workspaceobjectref",
        "subject_fingerprint": "fp_subject_sha256_workspaceobjectref",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_object_ref_contract",
            "stage_call_id": "sc_workspace_object_ref",
            "role": "runner"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-24T00:00:00Z",
        "expires_at": "2026-05-24T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_workspace_object_ref"
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
                    "candidate_ids": ["cand_object_ref"]
                },
                "constraints": {
                    "workspace_ops": ["materialize"]
                }
            },
            {
                "action": "workspace.read",
                "resource": {
                    "workspace_ids": [WORKSPACE_ID]
                },
                "constraints": {
                    "allowed_input_classes": ["candidate.artifact"],
                    "workspace_ops": ["read_file"]
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

fn plan_execution_context() -> PlanExecutionContext {
    PlanExecutionContext::new(
        "fp_cap_sha256_workspaceobjectref",
        "fp_policy_sha256_workspaceobjectref",
        "rev_workspaceobjectref_base",
        "2026-05-24T00:00:00Z",
        "2026-05-24T00:00:01Z",
    )
}
