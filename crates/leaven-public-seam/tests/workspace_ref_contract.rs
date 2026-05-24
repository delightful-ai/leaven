use leaven_public_seam::{
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost,
    PlanLmCompleteOutcome, PlanLmCompleteRequest, PlanWorkspaceMaterializeOutcome,
    PlanWorkspaceMaterializeRequest, PlanWorkspaceReleaseOutcome, PlanWorkspaceReleaseRequest,
    PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

const WORKSPACE_ID: &str = "ws_object_ref_contract";
const RUN_ID: &str = "run_object_ref_contract";
const SNAPSHOT: &str = "fp_snapshot_sha256_objectrefcontract";

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
        assert_eq!(request.candidate()?, "cand_object_ref");
        assert_eq!(request.lifetime()?, "manual_release");
        self.calls.push("workspace_materialize");
        Ok(PlanWorkspaceMaterializeOutcome::new(
            WORKSPACE_ID,
            request.lifetime()?,
            "fp_runtime_sha256_workspaceobjectref",
        )
        .with_workspace_object_ref(Some(RUN_ID), Some(SNAPSHOT)))
    }

    fn workspace_release(
        &mut self,
        request: PlanWorkspaceReleaseRequest<'_>,
    ) -> Result<PlanWorkspaceReleaseOutcome, PublicSeamError> {
        assert_eq!(request.live_workspace()?, WORKSPACE_ID);
        self.calls.push("workspace_release");
        Ok(PlanWorkspaceReleaseOutcome::new(
            WORKSPACE_ID,
            "manual_release",
            "fp_runtime_sha256_workspaceobjectref",
        )
        .with_workspace_object_ref(Some(RUN_ID), Some(SNAPSHOT)))
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

fn workspace_ref(run: &str, snapshot_fingerprint: &str) -> Value {
    json!({
        "kind": "workspace",
        "run": run,
        "id": WORKSPACE_ID,
        "snapshot_fingerprint": snapshot_fingerprint
    })
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

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
