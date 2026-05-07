use leaven_workspace::{WorkspacePath, WorkspacePathError};
use proptest::prelude::*;

#[test]
fn workspace_path_accepts_normal_relative_paths() {
    let path = WorkspacePath::new("skills/foo.md").unwrap();

    assert_eq!(path.as_str(), "skills/foo.md");
    assert_eq!(
        WorkspacePath::root().join("child.txt").unwrap().as_str(),
        "child.txt"
    );
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
    assert_eq!(
        WorkspacePath::new("./visible.txt").unwrap().as_str(),
        "visible.txt"
    );
    assert!(matches!(
        WorkspacePath::new("."),
        Err(WorkspacePathError::Empty)
    ));
}

proptest! {
    #[test]
    fn generated_workspace_paths_never_escape_root(
        components in proptest::collection::vec("[a-zA-Z0-9_-]{1,12}", 1..8),
    ) {
        let raw = components.join("/");
        let path = WorkspacePath::new(&raw).unwrap();

        prop_assert_eq!(path.as_str(), raw);
        prop_assert!(!path.as_str().starts_with('/'));
        prop_assert!(!path.as_str().contains(".."));
        prop_assert!(!path.as_str().contains("//"));
    }

    #[test]
    fn generated_parent_traversal_paths_are_refused(
        prefix in proptest::option::of("[a-z]{1,8}"),
        suffix in proptest::option::of("[a-z]{1,8}"),
    ) {
        let raw = match (prefix, suffix) {
            (Some(prefix), Some(suffix)) => format!("{prefix}/../{suffix}"),
            (Some(prefix), None) => format!("{prefix}/.."),
            (None, Some(suffix)) => format!("../{suffix}"),
            (None, None) => "..".to_owned(),
        };

        prop_assert!(matches!(
            WorkspacePath::new(&raw),
            Err(WorkspacePathError::ParentTraversal(_))
        ));
    }
}
