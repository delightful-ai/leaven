#![cfg(feature = "firkin-apple-vz-live")]

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;

use firkin_e2b_contract::{RuntimeAdapter, StartPodRequest};
use firkin_e2b_wire::{
    PodContainerCreateRequest, PodCreateRequest, PodEmptyDir, PodStoreOptions,
    PodVolumeMountRequest,
};
use firkin_single_node::AppleVzLocalRuntimeDriver;
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback, GitProgramStores};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_kernel::RunId;
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_firkin::{
    FirkinGuestPath, FirkinImageRef, FirkinProductPodId, FirkinRuntimeAdapterRuntime,
    FirkinWorkspaceContext, FirkinWorkspaceFactory,
};
use tokio::runtime::Runtime;

#[test]
#[ignore = "signed live Apple/VZ Firkin Git workspace proof; boots a VM"]
fn live_apple_vz_product_pod_materializes_and_reads_back_git_workspaces()
-> Result<(), Box<dyn Error>> {
    let image = std::env::var("LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE").map_err(|_| {
        "LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE must name an OCI image with git, sh, cat, find, mkdir, rm, test, and sleep"
    })?;
    let pod_id = format!("leaven-git-live-{}", RunId::new());
    let driver = AppleVzLocalRuntimeDriver::new(image);
    let tokio = Runtime::new()?;

    start_product_pod(&tokio, &driver, &pod_id)?;
    let body = run_live_git_body(driver.clone(), &pod_id);
    let stop = stop_product_pod(&tokio, &driver, &pod_id);

    match (body, stop) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(body), Err(stop)) => {
            Err(format!("{body}; additionally failed to stop pod: {stop}").into())
        }
    }
}

fn start_product_pod(
    runtime: &Runtime,
    driver: &AppleVzLocalRuntimeDriver,
    pod_id: &str,
) -> Result<(), Box<dyn Error>> {
    runtime.block_on(driver.start_pod(StartPodRequest {
        create_request: PodCreateRequest {
            pod_id: Some(pod_id.to_owned()),
            timeout: Some(900),
            metadata: BTreeMap::from([(
                "leaven.live-proof".to_owned(),
                "firkin-git-workspace".to_owned(),
            )]),
            empty_dirs: vec![PodEmptyDir {
                name: "workspace".to_owned(),
            }],
            pod_store: PodStoreOptions::default(),
            containers: vec![PodContainerCreateRequest {
                name: "keeper".to_owned(),
                template_id: "base".to_owned(),
                command: vec![
                    "/bin/sh".to_owned(),
                    "-lc".to_owned(),
                    "mkdir -p /workspace && sleep 2147483647".to_owned(),
                ],
                env_vars: BTreeMap::new(),
                empty_dir_mounts: vec![PodVolumeMountRequest {
                    name: "workspace".to_owned(),
                    path: "/workspace".to_owned(),
                    read_only: false,
                }],
                capture_output: false,
            }],
        },
        prepared_templates: BTreeMap::new(),
    }))?;
    Ok(())
}

fn stop_product_pod(
    runtime: &Runtime,
    driver: &AppleVzLocalRuntimeDriver,
    pod_id: &str,
) -> Result<(), Box<dyn Error>> {
    runtime.block_on(driver.stop_pod(pod_id))?;
    Ok(())
}

fn run_live_git_body(
    driver: AppleVzLocalRuntimeDriver,
    pod_id: &str,
) -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new();
    let runtime = Arc::new(FirkinRuntimeAdapterRuntime::new(driver, "workspace")?);
    let factory = FirkinWorkspaceFactory::new(
        runtime,
        FirkinProductPodId::new(pod_id)?,
        FirkinGuestPath::new("/workspace")?,
        FirkinImageRef::new("base")?,
    );

    let mut workspace_a =
        futures::executor::block_on(factory.allocate(WorkspaceConfig::default()))?;
    let mut workspace_b =
        futures::executor::block_on(factory.allocate(WorkspaceConfig::default()))?;
    assert!(workspace_a.local_mount().is_none());
    assert!(workspace_b.local_mount().is_none());

    let context_a = workspace_a.factory_context::<FirkinWorkspaceContext>()?;
    let context_b = workspace_b.factory_context::<FirkinWorkspaceContext>()?;
    assert_eq!(context_a.product_pod_id(), context_b.product_pod_id());
    assert_ne!(context_a.container_id(), context_b.container_id());
    assert_ne!(context_a.workspace_root(), context_b.workspace_root());

    let artifact = fixture.artifact();
    let materializer = GitProgramMaterializer::new(fixture.stores());
    let mut view_a = workspace_a.view();
    let mut view_b = workspace_b.view();
    materializer.materialize_program(&artifact, &mut view_a)?;
    materializer.materialize_program(&artifact, &mut view_b)?;
    assert_eq!(
        view_a.read_file(&workspace_path("repos/program/program.txt"))?,
        b"program base\n"
    );
    assert_eq!(
        view_b.read_file(&workspace_path("repos/program/program.txt"))?,
        b"program base\n"
    );

    view_a.write_file(
        &workspace_path("repos/program/program.txt"),
        b"program child from live Firkin workspace\n",
    )?;
    let change = GitProgramReadback::new(fixture.stores())
        .read_back_change(&artifact, &mut view_a)?
        .expect("dirty live Firkin workspace should produce a Git change");
    let GitProgramChange::AdvanceRepo { child, .. } = change else {
        panic!("single repo live Firkin readback should return AdvanceRepo");
    };
    let GitRevision::Commit(child) = child else {
        panic!("live Firkin readback should import a commit");
    };
    assert_eq!(
        git_output(&fixture.store, ["show", &format!("{child}:program.txt")]),
        "program child from live Firkin workspace\n"
    );
    assert_eq!(
        view_b.read_file(&workspace_path("repos/program/program.txt"))?,
        b"program base\n"
    );

    drop(view_a);
    drop(view_b);
    futures::executor::block_on(workspace_a.cleanup())?;
    futures::executor::block_on(workspace_b.cleanup())?;
    Ok(())
}

struct GitFixture {
    store: PathBuf,
    parent: GitRevision,
    _temp: tempfile::TempDir,
}

impl GitFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let store = temp.path().join("program.git");
        create_repo(&source);
        run_git_at(temp.path(), ["clone", "--bare", "source", "program.git"]);
        let parent = GitRevision::Commit(git_object(&source, "main"));
        Self {
            store,
            parent,
            _temp: temp,
        }
    }

    fn stores(&self) -> GitProgramStores {
        GitProgramStores::new(BTreeMap::from([(repo_key("program"), self.store.clone())])).unwrap()
    }

    fn artifact(&self) -> GitProgramArtifact {
        GitProgramArtifact::new(
            BTreeMap::from([(
                repo_key("program"),
                GitRepoArtifact::new(
                    RepoRef::global(repo_key("program")),
                    self.parent.clone(),
                    None,
                    GitArtifactIdentityMode::Commit,
                ),
            )]),
            GitProgramLayout::new(BTreeMap::from([(
                repo_key("program"),
                git_path("repos/program"),
            )]))
            .unwrap(),
        )
        .unwrap()
    }
}

fn create_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    run_git(root, ["init", "--initial-branch=main"]);
    run_git(root, ["config", "user.name", "Leaven Test"]);
    run_git(root, ["config", "user.email", "leaven@example.invalid"]);
    fs::write(root.join("program.txt"), "program base\n").unwrap();
    run_git(root, ["add", "program.txt"]);
    run_git(root, ["commit", "-m", "base"]);
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    run_git_at(cwd, args);
}

fn run_git_at<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn repo_key(key: &str) -> RepoKey {
    RepoKey::new(key).unwrap()
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
}

fn git_object(cwd: &Path, rev: &str) -> GitObjectId {
    GitObjectId::new(git_output(cwd, ["rev-parse", rev]).trim()).unwrap()
}
