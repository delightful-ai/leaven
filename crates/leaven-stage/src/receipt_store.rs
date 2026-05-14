use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use leaven_kernel::{FingerprintBuilder, StageAttemptReceiptId, StageAttemptReceiptRef};

use crate::StageAttemptReceipt;

#[allow(async_fn_in_trait)]
pub trait StageReceiptStore: Send + Sync {
    async fn write(
        &self,
        receipt: StageAttemptReceipt,
    ) -> Result<StageAttemptReceiptRef, ReceiptStoreError>;

    async fn read(
        &self,
        id: StageAttemptReceiptId,
    ) -> Result<Option<StageAttemptReceipt>, ReceiptStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiptStoreError {
    #[error("receipt store failed: {0}")]
    Store(String),
}

#[derive(Clone, Default)]
pub struct InlineReceiptStore {
    receipts: Arc<Mutex<BTreeMap<StageAttemptReceiptId, StageAttemptReceipt>>>,
}

impl StageReceiptStore for InlineReceiptStore {
    async fn write(
        &self,
        receipt: StageAttemptReceipt,
    ) -> Result<StageAttemptReceiptRef, ReceiptStoreError> {
        self.write_sync(receipt)
    }

    async fn read(
        &self,
        id: StageAttemptReceiptId,
    ) -> Result<Option<StageAttemptReceipt>, ReceiptStoreError> {
        self.read_sync(id)
    }
}

impl InlineReceiptStore {
    pub fn write_sync(
        &self,
        receipt: StageAttemptReceipt,
    ) -> Result<StageAttemptReceiptRef, ReceiptStoreError> {
        let id = receipt.receipt_id;
        let bytes = serde_json::to_vec(&receipt)
            .map_err(|err| ReceiptStoreError::Store(err.to_string()))?;
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint
            .update(b"leaven.stage.attempt-receipt.v1")
            .update(bytes);
        self.receipts
            .lock()
            .map_err(|_| ReceiptStoreError::Store("receipt store poisoned".to_owned()))?
            .insert(id, receipt);
        Ok(StageAttemptReceiptRef {
            id,
            fingerprint: Some(fingerprint.finish()),
        })
    }

    pub fn read_sync(
        &self,
        id: StageAttemptReceiptId,
    ) -> Result<Option<StageAttemptReceipt>, ReceiptStoreError> {
        Ok(self
            .receipts
            .lock()
            .map_err(|_| ReceiptStoreError::Store("receipt store poisoned".to_owned()))?
            .get(&id)
            .cloned())
    }
}
