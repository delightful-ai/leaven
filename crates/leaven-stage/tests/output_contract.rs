use leaven_stage::{StageOutputContract, StageOutputContractError};
use leaven_workspace::WorkspacePath;

#[test]
fn output_paths_must_be_under_output() {
    let contract =
        StageOutputContract::proposal_json(WorkspacePath::new("output/proposal.json").unwrap());

    assert!(contract.validate().is_ok());
}

#[test]
fn output_paths_reject_parent_traversal_and_non_output_roots() {
    assert!(WorkspacePath::new("output/../secret.json").is_err());

    let contract =
        StageOutputContract::proposal_json(WorkspacePath::new("scratch/proposal.json").unwrap());

    assert!(matches!(
        contract.validate(),
        Err(StageOutputContractError::InvalidOutputPath { .. })
    ));
}
