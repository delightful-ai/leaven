use leaven_kernel::{Cost, Metered};
use leaven_lm::{Lm, LmError, LmId, LmRequest, LmResponse};

use crate::{LmCacheEntry, LmCacheKey, LmCachePolicy, LmCacheStore};

/// LM wrapper that memoizes provider-neutral responses through a cache store.
pub struct CachedLm<M, C> {
    inner: M,
    cache: C,
    policy: LmCachePolicy,
}

impl<M, C> CachedLm<M, C> {
    /// Wraps an LM with a cache backend and default policy.
    #[must_use]
    pub const fn new(inner: M, cache: C, policy: LmCachePolicy) -> Self {
        Self {
            inner,
            cache,
            policy,
        }
    }

    /// Wraps an LM with read/write cache policy.
    #[must_use]
    pub const fn read_write(inner: M, cache: C) -> Self {
        Self::new(inner, cache, LmCachePolicy::ReadWrite)
    }

    /// Returns the wrapped LM.
    #[must_use]
    pub const fn inner(&self) -> &M {
        &self.inner
    }

    /// Returns the cache backend.
    #[must_use]
    pub const fn cache(&self) -> &C {
        &self.cache
    }
}

impl<M, C> CachedLm<M, C>
where
    M: Lm,
    C: LmCacheStore,
{
    /// Completes a request using an explicit one-call cache policy.
    ///
    /// # Errors
    ///
    /// Returns [`LmError`] from either the cache backend or wrapped LM.
    pub async fn complete_with_policy(
        &self,
        request: LmRequest,
        policy: LmCachePolicy,
    ) -> Result<Metered<LmResponse>, LmError> {
        let key = LmCacheKey::for_request(self.inner.fingerprint(), &request);
        match policy {
            LmCachePolicy::Never => self.inner.complete(request).await,
            LmCachePolicy::ReadWrite => {
                if let Some(entry) = self.cache.get(key).await.map_err(LmError::from)? {
                    return Ok(cached_response(entry));
                }
                let metered = self.inner.complete(request).await?;
                self.cache
                    .put(key, LmCacheEntry::new(key, metered.value.clone()))
                    .await
                    .map_err(LmError::from)?;
                Ok(metered)
            }
            LmCachePolicy::ReadOnly => {
                if let Some(entry) = self.cache.get(key).await.map_err(LmError::from)? {
                    return Ok(cached_response(entry));
                }
                self.inner.complete(request).await
            }
            LmCachePolicy::Refresh => {
                let metered = self.inner.complete(request).await?;
                self.cache
                    .put(key, LmCacheEntry::new(key, metered.value.clone()))
                    .await
                    .map_err(LmError::from)?;
                Ok(metered)
            }
        }
    }
}

impl<M, C> Lm for CachedLm<M, C>
where
    M: Lm,
    C: LmCacheStore,
{
    fn id(&self) -> LmId {
        self.inner.id()
    }

    fn fingerprint(&self) -> leaven_kernel::Fingerprint {
        self.inner.fingerprint()
    }

    async fn complete(&self, request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        self.complete_with_policy(request, self.policy).await
    }
}

fn cached_response(entry: LmCacheEntry) -> Metered<LmResponse> {
    Metered::new(entry.response, Cost::zero())
}

impl From<crate::LmCacheError> for LmError {
    fn from(error: crate::LmCacheError) -> Self {
        Self::cache(error.to_string())
    }
}
