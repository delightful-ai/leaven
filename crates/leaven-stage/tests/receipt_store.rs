use futures::executor::block_on;
use leaven_kernel::{Fingerprint, StageAttemptOutcome, StageCallId, StageRole, WorkspaceId};
use leaven_stage::{
    StageAttemptReceiptBuilder,
    receipt_store::{InlineReceiptStore, StageReceiptStore},
};

#[test]
fn inline_receipt_store_round_trips_receipts_by_id() {
    block_on(async {
        let store = InlineReceiptStore::default();
        let receipt = StageAttemptReceiptBuilder::new(
            WorkspaceId::new(),
            StageCallId::new(),
            StageRole::reflect(),
            Fingerprint::from_bytes([9; 32]),
        )
        .finish(StageAttemptOutcome::Completed);
        let id = receipt.receipt_id;

        let reference = store.write(receipt.clone()).await.unwrap();
        assert_eq!(reference.id, id);
        assert!(reference.fingerprint.is_some());
        assert_eq!(store.read(id).await.unwrap().unwrap().receipt_id, id);
        assert!(
            store
                .read(leaven_kernel::StageAttemptReceiptId::new())
                .await
                .unwrap()
                .is_none()
        );
    });
}
