use leaven_workspace::{WorkspacePath, WorkspacePathError};

#[test]
fn workspace_path_accepts_normal_relative_paths() {
    let path = WorkspacePath::new("skills/foo.md").unwrap();

    assert_eq!(path.as_str(), "skills/foo.md");
    assert_eq!(
        path.join("notes/bar.md").unwrap().as_str(),
        "skills/foo.md/notes/bar.md"
    );
}

#[test]
fn workspace_path_rejects_host_paths_and_traversal() {
    assert!(matches!(
        WorkspacePath::new("/tmp/secret"),
        Err(WorkspacePathError::Absolute(_))
    ));
    assert!(matches!(
        WorkspacePath::new("../secret"),
        Err(WorkspacePathError::ParentTraversal(_))
    ));
    assert!(matches!(
        WorkspacePath::new("skills//foo.md"),
        Err(WorkspacePathError::EmptyComponent(_))
    ));
}

#[test]
fn workspace_root_is_explicit_not_empty_parse() {
    assert_eq!(WorkspacePath::root().as_str(), "");
    assert!(matches!(
        WorkspacePath::new(""),
        Err(WorkspacePathError::Empty)
    ));
}
