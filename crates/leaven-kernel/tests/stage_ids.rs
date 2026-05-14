use leaven_kernel::{
    StageAttemptReceiptId, StageCallId, StageQueryId, WorkspaceEntryId, WorkspaceId,
};

#[test]
fn stage_ids_serde_roundtrip() {
    let ids = (
        WorkspaceId::new(),
        StageCallId::new(),
        StageAttemptReceiptId::new(),
        StageQueryId::new(),
        WorkspaceEntryId::new(),
    );

    let encoded = serde_json::to_string(&ids).unwrap();
    let decoded: (
        WorkspaceId,
        StageCallId,
        StageAttemptReceiptId,
        StageQueryId,
        WorkspaceEntryId,
    ) = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, ids);
}
