use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const EXPECTED_WORKSPACE_MEMBERS: &[&str] = &[
    "crates/leaven",
    "crates/leaven-agent",
    "crates/leaven-agent-command",
    "crates/leaven-agent-codex-app-server",
    "crates/leaven-agent-codex-cli",
    "crates/leaven-acp",
    "crates/leaven-acp-stage-bridge",
    "crates/leaven-cli",
    "crates/leaven-agentic",
    "crates/leaven-agentic-agent-kit",
    "crates/leaven-agentic-git",
    "crates/leaven-agentic-skill",
    "crates/leaven-artifact-agent-kit",
    "crates/leaven-artifact-git",
    "crates/leaven-artifact-jj",
    "crates/leaven-artifact-skill",
    "crates/leaven-core",
    "crates/leaven-engine",
    "crates/leaven-eval",
    "crates/leaven-eval-parquet",
    "crates/leaven-evidence",
    "crates/leaven-gepa",
    "crates/leaven-gepa-agentic-agent-kit",
    "crates/leaven-gepa-agentic-git",
    "crates/leaven-gepa-agentic-skill",
    "crates/leaven-kernel",
    "crates/leaven-lm",
    "crates/leaven-lm-cache",
    "crates/leaven-lm-mock",
    "crates/leaven-lm-openai",
    "crates/leaven-population",
    "crates/leaven-preference",
    "crates/leaven-public-seam",
    "crates/leaven-run",
    "crates/leaven-seam-runtime",
    "crates/leaven-seam-service",
    "crates/leaven-seam-stdio",
    "crates/leaven-stage",
    "crates/leaven-std",
    "crates/leaven-store",
    "crates/leaven-store-file",
    "crates/leaven-store-inline",
    "crates/leaven-surface",
    "crates/leaven-workspace",
    "crates/leaven-workspace-firkin",
    "crates/leaven-workspace-git",
    "crates/leaven-workspace-local",
    "examples/p0_graph_skeleton",
    "examples/p1_keep_best",
    "examples/p2_pairwise_tournament",
    "examples/p3_gepa_parity",
    "examples/p4_meta_harness_lite",
    "examples/p5_evoskill_iteration",
    "examples/p5_skill_paper_reproductions",
    "examples/p6_optimizer_policy_self_opt",
    "examples/p7_self_optimization_kernel",
    "examples/p8_aime_gepa",
    "examples/p9_python_acp_gepa_codex",
    "examples/trace2skill_spreadsheetbench",
    "xtask",
];

const EXPECTED_CRATES: &[&str] = &[
    "leaven",
    "leaven-agent",
    "leaven-agent-command",
    "leaven-agent-codex-app-server",
    "leaven-agent-codex-cli",
    "leaven-acp",
    "leaven-acp-stage-bridge",
    "leaven-agentic",
    "leaven-agentic-agent-kit",
    "leaven-agentic-git",
    "leaven-agentic-skill",
    "leaven-artifact-agent-kit",
    "leaven-artifact-git",
    "leaven-artifact-jj",
    "leaven-artifact-skill",
    "leaven-core",
    "leaven-engine",
    "leaven-eval",
    "leaven-eval-parquet",
    "leaven-evidence",
    "leaven-gepa",
    "leaven-gepa-agentic-agent-kit",
    "leaven-gepa-agentic-git",
    "leaven-gepa-agentic-skill",
    "leaven-kernel",
    "leaven-lm",
    "leaven-lm-cache",
    "leaven-lm-mock",
    "leaven-lm-openai",
    "leaven-population",
    "leaven-preference",
    "leaven-public-seam",
    "leaven-run",
    "leaven-seam-runtime",
    "leaven-seam-service",
    "leaven-seam-stdio",
    "leaven-stage",
    "leaven-std",
    "leaven-store",
    "leaven-store-file",
    "leaven-store-inline",
    "leaven-surface",
    "leaven-workspace",
    "leaven-workspace-firkin",
    "leaven-workspace-git",
    "leaven-workspace-local",
];

const EXPECTED_BINARIES: &[&str] = &[
    "crates/leaven-cli",
    "examples/p0_graph_skeleton",
    "examples/p1_keep_best",
    "examples/p2_pairwise_tournament",
    "examples/p3_gepa_parity",
    "examples/p4_meta_harness_lite",
    "examples/p5_evoskill_iteration",
    "examples/p5_skill_paper_reproductions",
    "examples/p6_optimizer_policy_self_opt",
    "examples/p7_self_optimization_kernel",
    "examples/p8_aime_gepa",
    "examples/p9_python_acp_gepa_codex",
    "examples/trace2skill_spreadsheetbench",
    "xtask",
];

const EXPECTED_DEPENDENCIES: &[(&str, &[&str])] = &[
    (
        "leaven",
        &[
            "leaven-agentic",
            "leaven-agentic-git",
            "leaven-agentic-skill",
            "leaven-artifact-skill",
            "leaven-core",
            "leaven-engine",
            "leaven-eval",
            "leaven-gepa",
            "leaven-kernel",
            "leaven-lm",
            "leaven-lm-cache",
            "leaven-lm-openai",
            "leaven-run",
            "leaven-std",
            "leaven-surface",
            "leaven-workspace",
            "leaven-workspace-firkin",
            "leaven-workspace-git",
        ],
    ),
    ("leaven-agent", &["leaven-kernel", "leaven-workspace"]),
    (
        "leaven-agent-command",
        &["leaven-agent", "leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-agent-codex-app-server",
        &["leaven-agent", "leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-agent-codex-cli",
        &[
            "leaven-agent",
            "leaven-agent-command",
            "leaven-kernel",
            "leaven-workspace",
        ],
    ),
    ("leaven-acp", &["leaven-public-seam"]),
    (
        "leaven-acp-stage-bridge",
        &[
            "leaven-acp",
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-public-seam",
            "leaven-run",
        ],
    ),
    (
        "leaven-agentic",
        &[
            "leaven-agent",
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-store",
            "leaven-workspace",
        ],
    ),
    ("leaven-agentic-agent-kit", &["leaven-artifact-agent-kit"]),
    (
        "leaven-agentic-skill",
        &[
            "leaven-agent",
            "leaven-agentic",
            "leaven-artifact-skill",
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-agentic-git",
        &[
            "leaven-artifact-git",
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-workspace",
            "leaven-workspace-git",
        ],
    ),
    (
        "leaven-artifact-agent-kit",
        &[
            "leaven-artifact-git",
            "leaven-artifact-skill",
            "leaven-kernel",
        ],
    ),
    ("leaven-artifact-git", &["leaven-core", "leaven-kernel"]),
    (
        "leaven-artifact-jj",
        &[
            "leaven-core",
            "leaven-kernel",
            "leaven-stage",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-artifact-skill",
        &["leaven-core", "leaven-kernel", "leaven-surface"],
    ),
    ("leaven-core", &["leaven-kernel"]),
    (
        "leaven-engine",
        &[
            "leaven-core",
            "leaven-kernel",
            "leaven-store",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-evidence",
        &["leaven-artifact-skill", "leaven-core", "leaven-kernel"],
    ),
    ("leaven-eval", &["leaven-core", "leaven-kernel"]),
    ("leaven-eval-parquet", &["leaven-eval", "leaven-kernel"]),
    (
        "leaven-gepa",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-lm",
            "leaven-population",
            "leaven-stage",
            "leaven-surface",
        ],
    ),
    (
        "leaven-gepa-agentic-agent-kit",
        &[
            "leaven-agentic-agent-kit",
            "leaven-artifact-agent-kit",
            "leaven-artifact-git",
            "leaven-core",
            "leaven-gepa",
        ],
    ),
    (
        "leaven-gepa-agentic-git",
        &[
            "leaven-agent",
            "leaven-agentic",
            "leaven-agentic-git",
            "leaven-artifact-git",
            "leaven-core",
            "leaven-engine",
            "leaven-gepa",
            "leaven-kernel",
            "leaven-surface",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-gepa-agentic-skill",
        &[
            "leaven-agent",
            "leaven-agentic",
            "leaven-agentic-skill",
            "leaven-artifact-skill",
            "leaven-core",
            "leaven-engine",
            "leaven-gepa",
            "leaven-kernel",
            "leaven-surface",
            "leaven-workspace",
        ],
    ),
    ("leaven-kernel", &[]),
    ("leaven-lm", &["leaven-kernel"]),
    ("leaven-lm-cache", &["leaven-kernel", "leaven-lm"]),
    ("leaven-lm-mock", &["leaven-kernel", "leaven-lm"]),
    ("leaven-lm-openai", &["leaven-kernel", "leaven-lm"]),
    (
        "leaven-population",
        &[
            "leaven-artifact-skill",
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
        ],
    ),
    ("leaven-preference", &["leaven-core", "leaven-evidence"]),
    (
        "leaven-public-seam",
        &[
            "leaven-agent",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-lm",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-run",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-eval",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-store",
            "leaven-store-file",
            "leaven-store-inline",
        ],
    ),
    ("leaven-seam-runtime", &["leaven-public-seam"]),
    (
        "leaven-seam-service",
        &[
            "leaven-agent",
            "leaven-agent-codex-cli",
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-lm",
            "leaven-lm-mock",
            "leaven-lm-openai",
            "leaven-public-seam",
            "leaven-run",
            "leaven-seam-runtime",
            "leaven-store",
            "leaven-store-file",
            "leaven-store-inline",
            "leaven-workspace",
            "leaven-workspace-local",
        ],
    ),
    ("leaven-seam-stdio", &["leaven-seam-runtime"]),
    (
        "leaven-stage",
        &[
            "leaven-agent",
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-std",
        &[
            "leaven-artifact-git",
            "leaven-artifact-jj",
            "leaven-artifact-skill",
            "leaven-evidence",
            "leaven-population",
            "leaven-preference",
            "leaven-surface",
        ],
    ),
    ("leaven-store", &["leaven-core", "leaven-kernel"]),
    ("leaven-store-file", &["leaven-kernel", "leaven-store"]),
    ("leaven-store-inline", &["leaven-kernel", "leaven-store"]),
    ("leaven-surface", &["leaven-core", "leaven-kernel"]),
    ("leaven-workspace", &["leaven-kernel"]),
    (
        "leaven-workspace-firkin",
        &["leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-workspace-git",
        &["leaven-artifact-git", "leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-workspace-local",
        &["leaven-kernel", "leaven-workspace"],
    ),
];

#[test]
fn corrected_topology_workspace_members_have_entrypoints() {
    let root = workspace_root();
    assert_eq!(
        workspace_members(&root),
        str_set(EXPECTED_WORKSPACE_MEMBERS)
    );
    for krate in EXPECTED_CRATES {
        let crate_root = root.join("crates").join(krate);
        assert!(crate_root.join("Cargo.toml").exists());
        assert!(
            crate_root.join("src/lib.rs").exists(),
            "{krate} must expose a src/lib.rs entrypoint"
        );
    }
    assert_eq!(
        crate_directories(&root),
        expected_crate_directories(),
        "crate-like directories under crates/ must be real expected workspace crates; delete placeholder directories instead of quarantining them"
    );
    for member in EXPECTED_BINARIES {
        let member_root = root.join(member);
        assert!(member_root.join("Cargo.toml").exists());
        assert!(
            member_root.join("src/main.rs").exists(),
            "{member} must expose a src/main.rs entrypoint"
        );
    }
}

#[test]
fn leaven_dependency_edges_match_corrected_topology() {
    let root = workspace_root();
    let expected = dependency_map();
    let actual = EXPECTED_CRATES
        .iter()
        .map(|krate| {
            (
                (*krate).to_owned(),
                path_dependencies(&root.join(format!("crates/{krate}/Cargo.toml"))),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
}

#[test]
fn cold_core_has_no_projection_or_engine_leaks() {
    let root = workspace_root();
    let core = root.join("crates/leaven-core/src");
    let lib = fs::read_to_string(core.join("lib.rs")).unwrap();
    for forbidden in ["context", "graph", "stage", "engine", "workspace", "store"] {
        assert!(!lib.contains(&format!("pub mod {forbidden};")));
    }
    for path in rust_files(&core) {
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("Decomposable") && !text.contains("Component"),
            "cold core must not contain component projection assumptions: {}",
            path.display()
        );
    }
}

#[test]
fn codex_app_server_protocol_is_leaf_only() {
    let root = workspace_root();
    let allowed = "crates/leaven-agent-codex-app-server/Cargo.toml";

    for manifest in cargo_manifests(&root) {
        let relative = manifest.strip_prefix(&root).unwrap();
        let relative = relative.to_string_lossy();
        let text = fs::read_to_string(&manifest).unwrap();

        if relative == allowed {
            continue;
        }

        assert!(
            !text.contains("codex-app-server-protocol") && !text.contains("codex-protocol"),
            "Codex app-server protocol crates must stay leaf-only, but `{relative}` depends on them"
        );
    }

    let umbrella = fs::read_to_string(root.join("crates/leaven/Cargo.toml")).unwrap();
    assert!(
        !umbrella.contains("leaven-agent-codex-cli")
            && !umbrella.contains("leaven-agent-codex-app-server"),
        "umbrella leaven must not expose a Codex provider feature until import-experience design names one"
    );

    assert!(
        !root.join("crates/leaven-agent-codex").exists(),
        "deleted Codex facade placeholder must not return without behavior-bearing topology and public-route proof"
    );
}

#[test]
fn deleted_placeholder_crates_stay_deleted() {
    let root = workspace_root();

    for deleted in [
        "crates/leaven-derive",
        "crates/leaven-render",
        "crates/leaven-cuda",
        "crates/leaven-python",
        "crates/leaven-dsrs",
    ] {
        assert!(
            !root.join(deleted).exists(),
            "{deleted} must not return as a placeholder crate"
        );
    }
}

#[test]
fn gepa_agent_stage_scaffold_is_not_a_root_public_route() {
    let root = workspace_root();
    let lib = fs::read_to_string(root.join("crates/leaven-gepa/src/lib.rs")).unwrap();

    assert!(
        !lib.contains("pub mod agent_stage;"),
        "legacy GEPA agent-stage scaffold must route through test_support, not a root public module"
    );
    assert!(
        !lib.contains("mod agent_stage;") && !lib.contains("GepaReflectionBootstrap"),
        "legacy GEPA agent-stage scaffold should stay deleted until a behavior-bearing route replaces it"
    );
}

#[test]
fn fake_agent_runtime_is_explicit_test_support() {
    let root = workspace_root();
    let lib = fs::read_to_string(root.join("crates/leaven-agent/src/lib.rs")).unwrap();

    assert!(
        !lib.contains("pub use fake::{FakeAgentAction, FakeAgentRuntime};"),
        "fake runtime helpers must not be crate-root public provider routes"
    );
    assert!(
        !lib.contains("CommandRecord, FakeAgentAction, FakeAgentRuntime"),
        "fake runtime helpers must not flow through leaven_agent::prelude"
    );
    assert!(
        lib.contains("pub mod test_support")
            && lib.contains("pub use crate::fake::{FakeAgentAction, FakeAgentRuntime};"),
        "fake runtime helpers should remain available only through explicit test_support"
    );
}

#[test]
fn git_artifact_surfaces_are_not_empty_public_markers() {
    let root = workspace_root();
    let lib = fs::read_to_string(root.join("crates/leaven-artifact-git/src/lib.rs")).unwrap();
    let std = fs::read_to_string(root.join("crates/leaven-std/src/lib.rs")).unwrap();

    for symbol in [
        "GitPathSurface",
        "GitAgentKitSurface",
        "GitSkillFrontmatterSurface",
    ] {
        assert!(
            !lib.contains(symbol),
            "`{symbol}` must not be exported by leaven-artifact-git until it has behavior"
        );
        assert!(
            !std.contains(symbol),
            "`{symbol}` must not be laundered through leaven-std while it is absent"
        );
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under workspace/crates/leaven")
        .to_path_buf()
}

fn workspace_members(root: &Path) -> BTreeSet<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    section(&text, "workspace")
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let path = line.trim_matches(&['"', ','][..]);
            if path.starts_with("crates/") || path.starts_with("examples/") || path == "xtask" {
                Some(path.to_owned())
            } else {
                None
            }
        })
        .collect()
}

fn dependency_map() -> BTreeMap<String, BTreeSet<String>> {
    EXPECTED_DEPENDENCIES
        .iter()
        .map(|(krate, deps)| ((*krate).to_owned(), str_set(deps)))
        .collect()
}

fn str_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().copied().map(str::to_owned).collect()
}

fn expected_crate_directories() -> BTreeSet<String> {
    EXPECTED_CRATES
        .iter()
        .copied()
        .chain(["leaven-cli"])
        .map(str::to_owned)
        .collect()
}

fn path_dependencies(cargo_toml: &Path) -> BTreeSet<String> {
    let text = fs::read_to_string(cargo_toml).unwrap();
    section(&text, "dependencies")
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            if line.is_empty() || !line.contains("workspace = true") {
                return None;
            }
            dependency_name(line)
        })
        .filter(|name| name.starts_with("leaven"))
        .collect()
}

fn dependency_name(line: &str) -> Option<String> {
    let (name, _) = line.split_once('=')?;
    Some(name.trim().to_owned())
}

fn section<'a>(text: &'a str, name: &str) -> &'a str {
    let header = format!("[{name}]");
    let Some(start) = text.find(&header) else {
        return "";
    };
    let body = &text[start + header.len()..];
    let end = body.find("\n[").unwrap_or(body.len());
    &body[..end]
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path.to_path_buf());
        }
        return;
    }
    for entry in fs::read_dir(path).unwrap() {
        collect_rust_files(&entry.unwrap().path(), files);
    }
}

fn cargo_manifests(root: &Path) -> Vec<PathBuf> {
    std::iter::once(root.join("Cargo.toml"))
        .chain(
            EXPECTED_WORKSPACE_MEMBERS
                .iter()
                .map(|member| root.join(member).join("Cargo.toml")),
        )
        .collect()
}

fn crate_directories(root: &Path) -> BTreeSet<String> {
    fs::read_dir(root.join("crates"))
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("leaven-") || name == "leaven")
        .collect()
}
