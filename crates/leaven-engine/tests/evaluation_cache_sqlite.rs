use std::fs;

use leaven_core::{AssessmentGranularity, CacheIdentity, CaseSetVersion, EvaluationPurpose};
use leaven_engine::{
    CachePolicy, EvaluationCache, EvaluationCacheEntry, EvaluationCacheKey,
    EvaluationCacheRequestKind, EvaluationCacheSnapshot, EvaluationCacheStoreError,
    SqliteEvaluationCache,
};
use leaven_kernel::{AssessmentId, CaseId, ContentId, Fingerprint};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn sqlite_eval_cache_reopens_entries_and_preserves_semantic_key() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let key = cache_key(CachePolicy::Deterministic);
    let assessment_ids = vec![AssessmentId::new(), AssessmentId::new()];

    SqliteEvaluationCache::open(&path)
        .unwrap()
        .insert(&key, &assessment_ids)
        .unwrap();

    let reopened = SqliteEvaluationCache::open(&path).unwrap();
    assert_eq!(reopened.get(&key).unwrap(), Some(assessment_ids.clone()));

    let loaded = reopened.load_cache().unwrap();
    assert_eq!(loaded.get(&key), Some(&assessment_ids));

    let mut policy_changed = key.clone();
    policy_changed.policy = CachePolicy::DeterministicWithSeed(9);
    assert_eq!(reopened.get(&policy_changed).unwrap(), None);

    let mut candidate_order_changed = key;
    candidate_order_changed.candidates.reverse();
    assert_eq!(reopened.get(&candidate_order_changed).unwrap(), None);
}

#[test]
fn sqlite_eval_cache_reports_parent_directory_creation_failure() {
    let temp = tempdir().unwrap();
    let blocked_parent = temp.path().join("not-a-directory");
    fs::write(&blocked_parent, b"file blocks child dirs").unwrap();
    let path = blocked_parent.join("run.sqlite");

    let Err(error) = SqliteEvaluationCache::open(&path) else {
        panic!("parent creation through a file should fail");
    };
    assert!(matches!(
        error,
        EvaluationCacheStoreError::Io {
            operation: "open",
            ..
        }
    ));
}

#[test]
fn sqlite_eval_cache_replaces_from_snapshot() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let first = cache_key(CachePolicy::Deterministic);
    let second = cache_key(CachePolicy::UserKey(Fingerprint::from_bytes([9; 32])));
    backend.insert(&first, &[AssessmentId::new()]).unwrap();

    let second_assessment = AssessmentId::new();
    let snapshot = EvaluationCacheSnapshot {
        entries: vec![EvaluationCacheEntry {
            key: second.clone(),
            assessment_ids: vec![second_assessment],
        }],
    };
    backend.replace_from_snapshot(&snapshot).unwrap();

    let loaded = backend.load_cache().unwrap();
    assert_eq!(loaded.get(&first), None);
    assert_eq!(loaded.get(&second), Some(&vec![second_assessment]));
}

#[test]
fn sqlite_eval_cache_empty_snapshot_clears_rows() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let key = cache_key(CachePolicy::Deterministic);
    backend.insert(&key, &[AssessmentId::new()]).unwrap();

    backend
        .replace_from_snapshot(&EvaluationCacheSnapshot::default())
        .unwrap();

    assert_eq!(backend.load_snapshot().unwrap().entries, Vec::new());
    assert_eq!(backend.get(&key).unwrap(), None);
}

#[test]
fn sqlite_eval_cache_rejects_newer_schema_version() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);

    let Err(error) = SqliteEvaluationCache::open(&path) else {
        panic!("newer schema version should be rejected");
    };
    assert!(matches!(
        error,
        EvaluationCacheStoreError::SchemaVersion {
            found: 99,
            supported: 1
        }
    ));
}

#[test]
fn sqlite_eval_cache_rejects_corrupt_database_file() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    fs::write(&path, b"not a sqlite database").unwrap();

    let Err(error) = SqliteEvaluationCache::open(&path) else {
        panic!("corrupt database file should be rejected");
    };
    assert!(matches!(
        error,
        EvaluationCacheStoreError::Backend {
            operation: "open",
            ..
        }
    ));
}

#[test]
fn sqlite_eval_cache_rejects_corrupt_rows_without_clearing() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let key = cache_key(CachePolicy::Deterministic);
    backend.insert(&key, &[AssessmentId::new()]).unwrap();
    drop(backend);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE evaluation_cache_entries SET assessment_ids_json = '{not-json'",
            [],
        )
        .unwrap();
    drop(connection);

    let Err(error) = SqliteEvaluationCache::open(&path).unwrap().load_cache() else {
        panic!("corrupt cache row should be rejected");
    };
    assert!(matches!(
        error,
        EvaluationCacheStoreError::Codec {
            operation: "decode assessment ids",
            ..
        }
    ));

    let connection = Connection::open(&path).unwrap();
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM evaluation_cache_entries", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn sqlite_eval_cache_rejects_hash_and_key_mismatch_rows() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let key = cache_key(CachePolicy::Deterministic);
    let mut different_key = cache_key(CachePolicy::DeterministicWithSeed(5));
    different_key.case_ids.reverse();
    backend.insert(&key, &[AssessmentId::new()]).unwrap();
    drop(backend);

    let connection = Connection::open(&path).unwrap();
    let different_key_json = serde_json::to_string(&different_key).unwrap();
    connection
        .execute(
            "UPDATE evaluation_cache_entries SET key_json = ?1",
            [different_key_json],
        )
        .unwrap();
    drop(connection);

    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let get_error = backend.get(&key).unwrap_err();
    assert!(matches!(
        get_error,
        EvaluationCacheStoreError::Corrupt {
            operation: "get",
            ..
        }
    ));
    let load_error = backend.load_snapshot().unwrap_err();
    assert!(matches!(
        load_error,
        EvaluationCacheStoreError::Corrupt {
            operation: "load",
            ..
        }
    ));
}

#[test]
fn sqlite_eval_cache_rejects_malformed_key_json_on_get_and_load() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let key = cache_key(CachePolicy::Deterministic);
    backend.insert(&key, &[AssessmentId::new()]).unwrap();
    drop(backend);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE evaluation_cache_entries SET key_json = '{not-json'",
            [],
        )
        .unwrap();
    drop(connection);

    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let get_error = backend.get(&key).unwrap_err();
    assert!(matches!(
        get_error,
        EvaluationCacheStoreError::Codec {
            operation: "decode key",
            ..
        }
    ));
    let load_error = backend.load_snapshot().unwrap_err();
    assert!(matches!(
        load_error,
        EvaluationCacheStoreError::Codec {
            operation: "decode key",
            ..
        }
    ));
}

#[test]
fn sqlite_eval_cache_reports_missing_schema_for_operations_without_recreating() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let key = cache_key(CachePolicy::Deterministic);
    drop(backend);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute("DROP TABLE evaluation_cache_entries", [])
        .unwrap();
    drop(connection);

    let backend = SqliteEvaluationCache::open(&path).unwrap();
    for (operation, error) in [
        ("get", backend.get(&key).unwrap_err()),
        (
            "insert",
            backend.insert(&key, &[AssessmentId::new()]).unwrap_err(),
        ),
        ("load", backend.load_snapshot().unwrap_err()),
        (
            "replace",
            backend
                .replace_from_snapshot(&EvaluationCacheSnapshot::default())
                .unwrap_err(),
        ),
    ] {
        assert!(
            matches!(
                error,
                EvaluationCacheStoreError::Backend {
                    operation: actual,
                    ..
                } if actual == operation
            ),
            "expected backend error during {operation}, got {error:?}",
        );
    }
}

#[test]
fn sqlite_eval_cache_get_records_hit_metadata() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let key = cache_key(CachePolicy::Deterministic);
    backend.insert(&key, &[AssessmentId::new()]).unwrap();

    assert!(backend.get(&key).unwrap().is_some());

    let connection = Connection::open(&path).unwrap();
    let (hit_count, last_hit_at): (i64, Option<String>) = connection
        .query_row(
            "SELECT hit_count, last_hit_at FROM evaluation_cache_entries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(hit_count, 1);
    assert!(last_hit_at.is_some());
}

#[test]
fn sqlite_eval_cache_can_load_into_engine_cache_shape() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("run.sqlite");
    let backend = SqliteEvaluationCache::open(&path).unwrap();
    let key = cache_key(CachePolicy::Deterministic);
    let assessment = AssessmentId::new();
    let mut cache = EvaluationCache::default();
    cache.insert(key.clone(), vec![assessment]);

    backend.replace_from_cache(&cache).unwrap();

    let loaded = backend.load_cache().unwrap();
    assert_eq!(loaded.get(&key), Some(&vec![assessment]));
}

fn cache_key(policy: CachePolicy) -> EvaluationCacheKey {
    EvaluationCacheKey {
        evaluator: Fingerprint::from_bytes([1; 32]),
        policy,
        case_set_version: CaseSetVersion("cases-v1".to_owned()),
        case_ids: vec![CaseId::new(1), CaseId::new(2)],
        request_kind: EvaluationCacheRequestKind::Independent,
        granularity: AssessmentGranularity::Aggregate,
        purpose: EvaluationPurpose::Search,
        candidates: vec![
            CacheIdentity::Content(ContentId::from_bytes([2; 32])),
            CacheIdentity::ExternalContent("immutable-candidate".to_owned()),
        ],
    }
}
