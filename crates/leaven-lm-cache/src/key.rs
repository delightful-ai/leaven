use leaven_kernel::{Fingerprint, FingerprintBuilder, Timestamp, now};
use leaven_lm::{LmRequest, LmResponse};
use serde::{Deserialize, Serialize};

use crate::LmCacheError;

/// Deterministic key for a Leaven LM response-cache entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LmCacheKey {
    /// Hash of provider fingerprint plus semantic request inputs.
    pub fingerprint: Fingerprint,
}

impl LmCacheKey {
    /// Builds a response-cache key for a provider/request pair.
    ///
    /// Provider continuation tokens are intentionally excluded; canonical
    /// messages are the request truth for text-only LM calls.
    #[must_use]
    pub fn for_request(provider: Fingerprint, request: &LmRequest) -> Self {
        Self::try_for_request(provider, request).expect("lm cache key material serializes")
    }

    /// Fallible key builder used by cache backends that want explicit errors.
    ///
    /// # Errors
    ///
    /// Returns [`LmCacheError::Codec`] if JSON serialization fails.
    pub fn try_for_request(
        provider: Fingerprint,
        request: &LmRequest,
    ) -> Result<Self, LmCacheError> {
        #[derive(Serialize)]
        struct KeyMaterial<'a> {
            provider: Fingerprint,
            model: &'a leaven_lm::ModelName,
            model_role: &'a Option<leaven_lm::ModelRole>,
            messages: &'a leaven_lm::Messages,
            sampling: &'a leaven_lm::SamplingOptions,
            output: &'a leaven_lm::OutputMode,
            provider_hints: &'a leaven_lm::ProviderHints,
        }

        let material = KeyMaterial {
            provider,
            model: &request.model,
            model_role: &request.model_role,
            messages: &request.messages,
            sampling: &request.sampling,
            output: &request.output,
            provider_hints: &request.provider_hints,
        };
        let bytes = serde_json::to_vec(&material)
            .map_err(|error| LmCacheError::codec(error.to_string()))?;
        let mut builder = FingerprintBuilder::new();
        builder.update(b"leaven-lm-cache-key-v1");
        builder.update(bytes);
        Ok(Self {
            fingerprint: builder.finish(),
        })
    }
}

/// Stored response-cache entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LmCacheEntry {
    /// Cache key for this entry.
    pub key: LmCacheKey,
    /// Fingerprint of the provider behavior that produced this response.
    pub provider_fingerprint: Fingerprint,
    /// Canonical provider-neutral request that produced this response.
    pub request: LmRequest,
    /// Provider response preserved from the original call.
    pub response: LmResponse,
    /// UTC time when the entry was written.
    pub stored_at: Timestamp,
}

impl LmCacheEntry {
    /// Builds a cache entry with the current UTC timestamp.
    #[must_use]
    pub fn new(
        key: LmCacheKey,
        provider_fingerprint: Fingerprint,
        request: LmRequest,
        response: LmResponse,
    ) -> Self {
        Self {
            key,
            provider_fingerprint,
            request,
            response,
            stored_at: now(),
        }
    }
}
