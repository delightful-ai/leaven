use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const EXPECTED_WORKSPACE_MEMBERS: &[&str] = &[
    "crates/leaven",
    "crates/leaven-agent",
    "crates/leaven-agent-claude-code",
    "crates/leaven-agent-codex",
    "crates/leaven-agent-opencode",
    "crates/leaven-agentic",
    "crates/leaven-artifact-git",
    "crates/leaven-artifact-jj",
    "crates/leaven-artifact-skill",
    "crates/leaven-artifacts",
    "crates/leaven-core",
    "crates/leaven-cuda",
    "crates/leaven-derive",
    "crates/leaven-dsrs",
    "crates/leaven-engine",
    "crates/leaven-evidence",
    "crates/leaven-gepa",
    "crates/leaven-kernel",
    "crates/leaven-lm",
    "crates/leaven-lm-anthropic",
    "crates/leaven-lm-local",
    "crates/leaven-lm-mock",
    "crates/leaven-lm-openai",
    "crates/leaven-mipro",
    "crates/leaven-population",
    "crates/leaven-preference",
    "crates/leaven-python",
    "crates/leaven-render",
    "crates/leaven-std",
    "crates/leaven-store",
    "crates/leaven-store-file",
    "crates/leaven-store-inline",
    "crates/leaven-store-object",
    "crates/leaven-store-sqlite",
    "crates/leaven-surface",
    "crates/leaven-textgrad",
    "crates/leaven-trace",
    "crates/leaven-workspace",
    "crates/leaven-workspace-docker",
    "crates/leaven-workspace-e2b",
    "crates/leaven-workspace-firecracker",
    "crates/leaven-workspace-git",
    "crates/leaven-workspace-k8s",
    "crates/leaven-workspace-local",
    "examples/p0_graph_skeleton",
    "examples/p1_keep_best",
    "examples/p2_pairwise_tournament",
    "examples/p3_gepa_parity",
    "examples/p4_meta_harness_lite",
    "examples/p5_skill_paper_reproductions",
    "xtask",
];

const EXPECTED_CRATES: &[&str] = &[
    "leaven",
    "leaven-agent",
    "leaven-agent-claude-code",
    "leaven-agent-codex",
    "leaven-agent-opencode",
    "leaven-agentic",
    "leaven-artifact-git",
    "leaven-artifact-jj",
    "leaven-artifact-skill",
    "leaven-artifacts",
    "leaven-core",
    "leaven-cuda",
    "leaven-derive",
    "leaven-dsrs",
    "leaven-engine",
    "leaven-evidence",
    "leaven-gepa",
    "leaven-kernel",
    "leaven-lm",
    "leaven-lm-anthropic",
    "leaven-lm-local",
    "leaven-lm-mock",
    "leaven-lm-openai",
    "leaven-mipro",
    "leaven-population",
    "leaven-preference",
    "leaven-python",
    "leaven-render",
    "leaven-std",
    "leaven-store",
    "leaven-store-file",
    "leaven-store-inline",
    "leaven-store-object",
    "leaven-store-sqlite",
    "leaven-surface",
    "leaven-textgrad",
    "leaven-trace",
    "leaven-workspace",
    "leaven-workspace-docker",
    "leaven-workspace-e2b",
    "leaven-workspace-firecracker",
    "leaven-workspace-git",
    "leaven-workspace-k8s",
    "leaven-workspace-local",
];

const EXPECTED_BINARIES: &[&str] = &[
    "examples/p0_graph_skeleton",
    "examples/p1_keep_best",
    "examples/p2_pairwise_tournament",
    "examples/p3_gepa_parity",
    "examples/p4_meta_harness_lite",
    "examples/p5_skill_paper_reproductions",
    "xtask",
];

const EXPECTED_DEPENDENCIES: &[(&str, &[&str])] = &[
    (
        "leaven",
        &[
            "leaven-agentic",
            "leaven-artifact-git",
            "leaven-artifact-jj",
            "leaven-core",
            "leaven-derive",
            "leaven-engine",
            "leaven-gepa",
            "leaven-kernel",
            "leaven-lm-anthropic",
            "leaven-lm-openai",
            "leaven-std",
            "leaven-store-sqlite",
            "leaven-surface",
            "leaven-workspace",
            "leaven-workspace-docker",
            "leaven-workspace-e2b",
            "leaven-workspace-local",
        ],
    ),
    ("leaven-agent", &["leaven-kernel", "leaven-workspace"]),
    (
        "leaven-agent-claude-code",
        &["leaven-agent", "leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-agent-codex",
        &["leaven-agent", "leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-agent-opencode",
        &["leaven-agent", "leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-agentic",
        &[
            "leaven-agent",
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-render",
            "leaven-store",
            "leaven-surface",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-artifact-git",
        &[
            "leaven-artifacts",
            "leaven-core",
            "leaven-kernel",
            "leaven-surface",
        ],
    ),
    (
        "leaven-artifact-jj",
        &[
            "leaven-artifacts",
            "leaven-core",
            "leaven-kernel",
            "leaven-surface",
        ],
    ),
    (
        "leaven-artifact-skill",
        &["leaven-core", "leaven-kernel", "leaven-surface"],
    ),
    (
        "leaven-artifacts",
        &["leaven-core", "leaven-kernel", "leaven-surface"],
    ),
    ("leaven-core", &["leaven-kernel"]),
    (
        "leaven-cuda",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-store",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-derive",
        &["leaven-core", "leaven-kernel", "leaven-surface"],
    ),
    (
        "leaven-dsrs",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-kernel",
            "leaven-lm",
            "leaven-surface",
        ],
    ),
    (
        "leaven-engine",
        &[
            "leaven-core",
            "leaven-kernel",
            "leaven-store",
            "leaven-surface",
            "leaven-workspace",
        ],
    ),
    ("leaven-evidence", &["leaven-core", "leaven-kernel"]),
    (
        "leaven-gepa",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-lm",
            "leaven-population",
            "leaven-preference",
            "leaven-render",
            "leaven-surface",
        ],
    ),
    ("leaven-kernel", &[]),
    ("leaven-lm", &["leaven-kernel"]),
    ("leaven-lm-anthropic", &["leaven-kernel", "leaven-lm"]),
    ("leaven-lm-local", &["leaven-kernel", "leaven-lm"]),
    ("leaven-lm-mock", &["leaven-kernel", "leaven-lm"]),
    ("leaven-lm-openai", &["leaven-kernel", "leaven-lm"]),
    (
        "leaven-mipro",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-population",
        ],
    ),
    (
        "leaven-population",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-preference",
        ],
    ),
    (
        "leaven-preference",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
        ],
    ),
    (
        "leaven-python",
        &["leaven-core", "leaven-engine", "leaven-kernel"],
    ),
    (
        "leaven-render",
        &[
            "leaven-artifacts",
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-store",
            "leaven-surface",
            "leaven-workspace",
        ],
    ),
    (
        "leaven-std",
        &[
            "leaven-artifact-git",
            "leaven-artifact-jj",
            "leaven-artifacts",
            "leaven-evidence",
            "leaven-population",
            "leaven-preference",
            "leaven-render",
            "leaven-surface",
        ],
    ),
    ("leaven-store", &["leaven-core", "leaven-kernel"]),
    ("leaven-store-file", &["leaven-kernel", "leaven-store"]),
    ("leaven-store-inline", &["leaven-kernel", "leaven-store"]),
    ("leaven-store-object", &["leaven-kernel", "leaven-store"]),
    ("leaven-store-sqlite", &["leaven-kernel", "leaven-store"]),
    ("leaven-surface", &["leaven-core", "leaven-kernel"]),
    (
        "leaven-textgrad",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-lm",
            "leaven-population",
            "leaven-surface",
        ],
    ),
    (
        "leaven-trace",
        &[
            "leaven-core",
            "leaven-engine",
            "leaven-evidence",
            "leaven-kernel",
            "leaven-lm",
            "leaven-render",
        ],
    ),
    ("leaven-workspace", &["leaven-kernel"]),
    (
        "leaven-workspace-docker",
        &["leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-workspace-e2b",
        &["leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-workspace-firecracker",
        &["leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-workspace-git",
        &["leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-workspace-k8s",
        &["leaven-kernel", "leaven-workspace"],
    ),
    (
        "leaven-workspace-local",
        &["leaven-kernel", "leaven-workspace"],
    ),
];

#[test]
fn corrected_topology_workspace_members_are_scaffolded() {
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
            "{krate} must expose a src/lib.rs skeleton"
        );
    }
    for member in EXPECTED_BINARIES {
        let member_root = root.join(member);
        assert!(member_root.join("Cargo.toml").exists());
        assert!(
            member_root.join("src/main.rs").exists(),
            "{member} must expose a src/main.rs skeleton"
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
