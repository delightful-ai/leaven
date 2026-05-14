use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use leaven_kernel::{StageAttemptReceiptId, StageAttemptReceiptRef};

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
        let id = receipt.receipt_id;
        self.receipts
            .lock()
            .map_err(|_| ReceiptStoreError::Store("receipt store poisoned".to_owned()))?
            .insert(id, receipt);
        Ok(StageAttemptReceiptRef {
            id,
            fingerprint: None,
        })
    }

    async fn read(
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
