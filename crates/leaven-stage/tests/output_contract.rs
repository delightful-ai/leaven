use leaven_kernel::Fingerprint;
use leaven_stage::{
    MediaType, OutputEntry, OutputEntryId, OutputRole, OutputSchema, StageOutputContract,
    StageOutputContractError,
};
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

#[test]
fn output_contract_maps_single_json_schema_to_agent_json_file() {
    let mut contract =
        StageOutputContract::proposal_json(WorkspacePath::new("output/proposal.json").unwrap());
    contract.schema = Some(OutputSchema {
        media_type: MediaType::Json,
        schema_text: r#"{"type":"object"}"#.to_owned(),
        schema_fingerprint: Some(Fingerprint::from_bytes([7; 32])),
    });

    match contract.to_agent_output_contract() {
        leaven_agent::OutputContract::JsonFile { path, schema } => {
            assert_eq!(path, WorkspacePath::new("output/proposal.json").unwrap());
            let schema = schema.unwrap();
            assert_eq!(schema.name, "proposal");
            assert_eq!(schema.schema, r#"{"type":"object"}"#);
        }
        other => panic!("unexpected output contract: {other:?}"),
    }
}

#[test]
fn output_contract_maps_multiple_entries_to_file_list() {
    let json = OutputEntry::new(
        OutputEntryId::new("proposal").unwrap(),
        WorkspacePath::new("output/proposal.json").unwrap(),
        OutputRole::proposal_json(),
        MediaType::Json,
    );
    let mut text = OutputEntry::new(
        OutputEntryId::new("notes").unwrap(),
        WorkspacePath::new("output/notes.txt").unwrap(),
        OutputRole::new("notes").unwrap(),
        MediaType::Text,
    );
    text.max_bytes = Some(1024);
    let contract = StageOutputContract {
        required: vec![json],
        optional: vec![text],
        schema: None,
    };

    assert_eq!(
        contract
            .all_entries()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        vec!["output/proposal.json", "output/notes.txt"]
    );
    match contract.to_agent_output_contract() {
        leaven_agent::OutputContract::Files { paths } => assert_eq!(paths.len(), 2),
        other => panic!("unexpected output contract: {other:?}"),
    }
    assert!(OutputEntryId::new("bad/id").is_err());
    assert!(OutputRole::new("bad role").is_err());
    assert_eq!(OutputRole::new("notes").unwrap().as_str(), "notes");
}

#[test]
fn output_contract_requires_at_least_one_output() {
    let contract = StageOutputContract::new(Vec::new());

    assert!(matches!(
        contract.validate(),
        Err(StageOutputContractError::NoRequiredOutputs)
    ));
}
