use std::collections::{BTreeMap, BTreeSet};

use super::{CapabilityDocument, CapabilityError, parse_timestamp};

/// In-memory capability registry for resolving opaque token handles.
#[derive(Clone, Debug, Default)]
pub struct CapabilityRegistry {
    by_opaque_token: BTreeMap<String, CapabilityDocument>,
    revoked_jtis: BTreeSet<String>,
}

impl CapabilityRegistry {
    /// Inserts a capability document under its own opaque token binding.
    pub fn insert(&mut self, document: CapabilityDocument) -> Result<(), CapabilityError> {
        let token = document
            .opaque_token_id()
            .ok_or_else(|| CapabilityError::InvalidDocument {
                message: "capability document is not bound to an opaque lookup token".to_owned(),
            })?;
        self.insert_with_opaque_handle(token.to_owned(), document)
    }

    /// Inserts a capability document under an explicit opaque handle.
    ///
    /// This is public so tests and transport adapters can prove binding
    /// mismatch refusal instead of assuming map keys are always correct.
    pub fn insert_with_opaque_handle(
        &mut self,
        token_id: impl Into<String>,
        document: CapabilityDocument,
    ) -> Result<(), CapabilityError> {
        self.by_opaque_token.insert(token_id.into(), document);
        Ok(())
    }

    /// Marks a JTI revoked.
    pub fn revoke_jti(&mut self, jti: impl Into<String>) {
        self.revoked_jtis.insert(jti.into());
    }

    /// Resolves an opaque token for a new operation.
    pub fn resolve_opaque_for_new_operation(
        &self,
        token_id: &str,
        now: &str,
    ) -> Result<&CapabilityDocument, CapabilityError> {
        let document =
            self.by_opaque_token
                .get(token_id)
                .ok_or_else(|| CapabilityError::UnknownToken {
                    token_id: token_id.to_owned(),
                })?;
        if document.opaque_token_id() != Some(token_id) {
            return Err(CapabilityError::BindingMismatch {
                token_id: token_id.to_owned(),
                bound_token_id: document.opaque_token_id().map(ToOwned::to_owned),
            });
        }
        if self.revoked_jtis.contains(document.jti()) {
            return Err(CapabilityError::Revoked {
                jti: document.jti().to_owned(),
            });
        }
        let now = parse_timestamp(now)?;
        let expires_at = parse_timestamp(document.expires_at())?;
        if now > expires_at {
            return Err(CapabilityError::Expired {
                jti: document.jti().to_owned(),
                expires_at: document.expires_at().to_owned(),
            });
        }
        Ok(document)
    }
}
