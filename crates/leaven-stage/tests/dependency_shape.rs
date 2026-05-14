use std::process::Command;

#[test]
fn leaven_stage_has_no_gepa_or_agentic_dependency() {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata runs");
    assert!(output.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let package = packages
        .iter()
        .find(|package| package["name"] == "leaven-stage")
        .unwrap();
    let deps = package["dependencies"].as_array().unwrap();

    for forbidden in ["leaven-gepa", "leaven-agentic"] {
        assert!(
            !deps.iter().any(|dep| dep["name"] == forbidden),
            "leaven-stage must not depend on {forbidden}"
        );
    }
}
