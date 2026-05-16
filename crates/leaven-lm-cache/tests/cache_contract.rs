use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leaven_kernel::{Cost, Fingerprint, Metered};
use leaven_lm::{
    Lm, LmContinuation, LmError, LmId, LmRequest, LmResponse, Message, Messages, ModelName,
    ProviderName, TokenUsage,
};
use leaven_lm_cache::{
    CachedLm, InMemoryLmCache, LmCacheEntry, LmCacheError, LmCacheKey, LmCachePolicy, LmCacheStore,
    SqliteLmCache,
};

#[derive(Clone)]
struct CountingLm {
    calls: Arc<AtomicUsize>,
}

struct FailingCache;

impl LmCacheStore for FailingCache {
    async fn get(&self, _key: LmCacheKey) -> Result<Option<LmCacheEntry>, LmCacheError> {
        Err(LmCacheError::Backend {
            operation: "get",
            message: "read refused".to_owned(),
        })
    }

    async fn put(&self, _key: LmCacheKey, _entry: LmCacheEntry) -> Result<(), LmCacheError> {
        Err(LmCacheError::Backend {
            operation: "put",
            message: "write refused".to_owned(),
        })
    }
}

impl CountingLm {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Lm for CountingLm {
    fn id(&self) -> LmId {
        LmId::new("counting")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([7; 32])
    }

    async fn complete(&self, _request: LmRequest) -> Result<Metered<LmResponse>, LmError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let usage = TokenUsage {
            input_tokens: 3,
            cached_input_tokens: 0,
            output_tokens: 2,
            reasoning_tokens: 0,
        };
        Ok(Metered::new(
            LmResponse::new(Message::assistant(format!("call {n}")), usage.clone()).unwrap(),
            usage.to_cost(),
        ))
    }
}

fn request() -> LmRequest {
    LmRequest::new(ModelName::new("mock-model"), Messages::from_user("hello"))
}

fn cache_key() -> LmCacheKey {
    LmCacheKey::for_request(Fingerprint::from_bytes([9; 32]), &request())
}

fn cache_entry(key: LmCacheKey, content: &str) -> LmCacheEntry {
    let usage = TokenUsage {
        input_tokens: 5,
        cached_input_tokens: 1,
        output_tokens: 3,
        reasoning_tokens: 2,
    };
    let response = LmResponse::new(Message::assistant(content), usage)
        .unwrap()
        .with_provider_response_id("resp_original");
    LmCacheEntry::new(key, Fingerprint::from_bytes([9; 32]), request(), response)
}

#[tokio::test]
async fn read_write_policy_serves_second_call_from_cache() {
    let inner = CountingLm::new();
    let lm = CachedLm::new(
        inner.clone(),
        InMemoryLmCache::default(),
        LmCachePolicy::ReadWrite,
    );

    let first = lm.complete(request()).await.unwrap();
    let second = lm.complete(request()).await.unwrap();

    assert_eq!(first.value.assistant.content(), "call 1");
    assert_eq!(second.value.assistant.content(), "call 1");
    assert_eq!(first.cost.llm_calls, 1);
    assert_eq!(second.cost, Cost::zero());
    assert_eq!(inner.calls(), 1);
}

#[tokio::test]
async fn cloned_cached_lm_handles_share_cache_and_preserve_cached_usage() {
    let inner = CountingLm::new();
    let lm = CachedLm::read_write(inner.clone(), InMemoryLmCache::default());
    let clone = lm.clone();

    let first = lm.complete(request()).await.unwrap();
    let second = clone.complete(request()).await.unwrap();

    assert_eq!(second.value.assistant.content(), "call 1");
    assert_eq!(second.value.usage, first.value.usage);
    assert_eq!(second.cost, Cost::zero());
    assert_eq!(inner.calls(), 1);
}

#[tokio::test]
async fn never_policy_bypasses_cache_and_accessors_expose_parts() {
    let inner = CountingLm::new();
    let cache = InMemoryLmCache::default();
    let lm = CachedLm::read_write(inner.clone(), cache.clone());

    assert_eq!(lm.inner().id().as_str(), "counting");
    assert_eq!(lm.fingerprint(), Fingerprint::from_bytes([7; 32]));
    assert!(lm.cache().is_empty());

    let first = lm
        .complete_with_policy(request(), LmCachePolicy::Never)
        .await
        .unwrap();
    let second = lm
        .complete_with_policy(request(), LmCachePolicy::Never)
        .await
        .unwrap();

    assert_eq!(first.value.assistant.content(), "call 1");
    assert_eq!(second.value.assistant.content(), "call 2");
    assert_eq!(inner.calls(), 2);
    assert_eq!(cache.len(), 0);
}

#[tokio::test]
async fn refresh_policy_bypasses_read_and_updates_entry() {
    let inner = CountingLm::new();
    let cache = InMemoryLmCache::default();
    let read_write = CachedLm::new(inner.clone(), cache.clone(), LmCachePolicy::ReadWrite);
    let refresh = CachedLm::new(inner.clone(), cache.clone(), LmCachePolicy::Refresh);

    let cached = read_write.complete(request()).await.unwrap();
    let refreshed = refresh.complete(request()).await.unwrap();
    let after_refresh = read_write.complete(request()).await.unwrap();

    assert_eq!(cached.value.assistant.content(), "call 1");
    assert_eq!(refreshed.value.assistant.content(), "call 2");
    assert_eq!(after_refresh.value.assistant.content(), "call 2");
    assert_eq!(inner.calls(), 2);
}

#[tokio::test]
async fn read_only_policy_reads_but_does_not_write() {
    let inner = CountingLm::new();
    let lm = CachedLm::new(
        inner.clone(),
        InMemoryLmCache::default(),
        LmCachePolicy::ReadOnly,
    );

    let first = lm.complete(request()).await.unwrap();
    let second = lm.complete(request()).await.unwrap();

    assert_eq!(first.value.assistant.content(), "call 1");
    assert_eq!(second.value.assistant.content(), "call 2");
    assert_eq!(inner.calls(), 2);
}

#[tokio::test]
async fn sqlite_cache_opens_and_creates_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/cache/lm-cache.sqlite");

    let cache = SqliteLmCache::open(&path).unwrap();

    assert_eq!(cache.path(), path.as_path());
    assert!(path.exists());
    assert_eq!(cache.get(cache_key()).await.unwrap(), None);
}

#[tokio::test]
async fn sqlite_cache_schema_carries_audit_columns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let key = cache_key();
    let entry = cache_entry(key, "schema");

    SqliteLmCache::open(&path)
        .unwrap()
        .put(key, entry.clone())
        .await
        .unwrap();

    let connection = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);

    let (provider_fingerprint, model, request_json): (String, String, String) = connection
        .query_row(
            "SELECT provider_fingerprint, model, request_json FROM lm_cache_entries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        provider_fingerprint,
        "0909090909090909090909090909090909090909090909090909090909090909"
    );
    assert_eq!(model, "mock-model");
    assert_eq!(
        serde_json::from_str::<LmRequest>(&request_json).unwrap(),
        entry.request
    );
}

#[tokio::test]
async fn sqlite_cache_reports_parent_directory_creation_failure() {
    let dir = tempfile::tempdir().unwrap();
    let blocked_parent = dir.path().join("not-a-directory");
    std::fs::write(&blocked_parent, b"file blocks child dirs").unwrap();
    let path = blocked_parent.join("lm-cache.sqlite");

    let Err(error) = SqliteLmCache::open(&path) else {
        panic!("parent creation through a file should fail");
    };
    assert!(matches!(
        error,
        LmCacheError::Backend {
            operation: "open",
            ..
        }
    ));
}

#[tokio::test]
async fn sqlite_cache_round_trips_entries_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let key = cache_key();
    let entry = cache_entry(key, "persisted");

    SqliteLmCache::open(&path)
        .unwrap()
        .put(key, entry.clone())
        .await
        .unwrap();
    let reopened = SqliteLmCache::open(&path).unwrap();

    assert_eq!(reopened.get(key).await.unwrap(), Some(entry));
}

#[tokio::test]
async fn sqlite_cache_rejects_newer_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);

    let Err(error) = SqliteLmCache::open(&path) else {
        panic!("newer schema version should be rejected");
    };
    assert!(matches!(
        error,
        LmCacheError::Backend {
            operation: "open",
            ..
        }
    ));
    assert!(error.to_string().contains("schema version 99"));
}

#[tokio::test]
async fn sqlite_cache_rejects_write_key_mismatch_before_storage() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let cache = SqliteLmCache::open(&path).unwrap();
    let key = cache_key();
    let other_key = LmCacheKey {
        fingerprint: Fingerprint::from_bytes([10; 32]),
    };

    let error = cache
        .put(other_key, cache_entry(key, "mismatched"))
        .await
        .unwrap_err();

    assert!(matches!(error, LmCacheError::Codec { .. }));
    assert_eq!(cache.get(key).await.unwrap(), None);
}

#[tokio::test]
async fn sqlite_cache_rejects_stored_key_mismatch_and_missing_schema() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let key = cache_key();
    let cache = SqliteLmCache::open(&path).unwrap();
    cache
        .put(key, cache_entry(key, "corrupt key"))
        .await
        .unwrap();
    drop(cache);

    let connection = rusqlite::Connection::open(&path).unwrap();
    let mut entry: LmCacheEntry = serde_json::from_str(
        &connection
            .query_row("SELECT entry_json FROM lm_cache_entries", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
    )
    .unwrap();
    entry.key = LmCacheKey {
        fingerprint: Fingerprint::from_bytes([11; 32]),
    };
    connection
        .execute(
            "UPDATE lm_cache_entries SET entry_json = ?1",
            [serde_json::to_string(&entry).unwrap()],
        )
        .unwrap();
    drop(connection);

    let cache = SqliteLmCache::open(&path).unwrap();
    let mismatch = cache.get(key).await.unwrap_err();
    assert!(matches!(mismatch, LmCacheError::Codec { .. }));
    drop(cache);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DROP TABLE lm_cache_entries", [])
        .unwrap();
    drop(connection);
    let cache = SqliteLmCache::open(&path).unwrap();

    let read_error = cache.get(key).await.unwrap_err();
    assert!(matches!(
        read_error,
        LmCacheError::Backend {
            operation: "get",
            ..
        }
    ));
    let write_error = cache
        .put(key, cache_entry(key, "write refused"))
        .await
        .unwrap_err();
    assert!(matches!(
        write_error,
        LmCacheError::Backend {
            operation: "put",
            ..
        }
    ));
}

#[tokio::test]
async fn sqlite_cache_read_miss_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let cache = SqliteLmCache::open(path).unwrap();

    assert_eq!(cache.get(cache_key()).await.unwrap(), None);
}

#[tokio::test]
async fn sqlite_cache_refresh_overwrites_cached_entry_through_cached_lm() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let inner = CountingLm::new();
    let cache = SqliteLmCache::open(path).unwrap();
    let read_write = CachedLm::new(inner.clone(), cache.clone(), LmCachePolicy::ReadWrite);
    let refresh = CachedLm::new(inner.clone(), cache.clone(), LmCachePolicy::Refresh);

    let cached = read_write.complete(request()).await.unwrap();
    let refreshed = refresh.complete(request()).await.unwrap();
    let after_refresh = read_write.complete(request()).await.unwrap();

    assert_eq!(cached.value.assistant.content(), "call 1");
    assert_eq!(refreshed.value.assistant.content(), "call 2");
    assert_eq!(after_refresh.value.assistant.content(), "call 2");
    assert_eq!(after_refresh.cost, Cost::zero());
    assert_eq!(inner.calls(), 2);
}

#[tokio::test]
async fn sqlite_cache_reports_malformed_entry_json_as_codec_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lm-cache.sqlite");
    let key = cache_key();
    let cache = SqliteLmCache::open(&path).unwrap();
    cache
        .put(key, cache_entry(key, "corrupt me"))
        .await
        .unwrap();
    drop(cache);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("UPDATE lm_cache_entries SET entry_json = '{not json'", [])
        .unwrap();
    drop(connection);

    let cache = SqliteLmCache::open(&path).unwrap();
    let error = cache.get(key).await.unwrap_err();
    assert!(matches!(error, LmCacheError::Codec { .. }));
}

#[tokio::test]
async fn cache_backend_failures_are_lifted_to_lm_errors() {
    let read_failure = CachedLm::new(CountingLm::new(), FailingCache, LmCachePolicy::ReadOnly)
        .complete(request())
        .await
        .unwrap_err();
    assert_eq!(
        read_failure.to_string(),
        "lm response cache failed: lm cache backend failed during get: read refused"
    );

    let put_failure = CachedLm::new(CountingLm::new(), FailingCache, LmCachePolicy::Refresh)
        .complete(request())
        .await
        .unwrap_err();
    assert_eq!(
        put_failure.to_string(),
        "lm response cache failed: lm cache backend failed during put: write refused"
    );

    let codec = LmCacheError::codec("bad key");
    assert_eq!(codec.to_string(), "lm cache codec failed: bad key");
}

#[test]
fn cache_key_ignores_provider_continuation_tokens() {
    let base = request();
    let with_continuation = base.clone().with_continuation(LmContinuation {
        provider: ProviderName::new("openai"),
        response_id: "resp_1".to_owned(),
        covered_messages: 1,
    });

    assert_eq!(
        LmCacheKey::for_request(Fingerprint::from_bytes([1; 32]), &base),
        LmCacheKey::for_request(Fingerprint::from_bytes([1; 32]), &with_continuation)
    );
}
