//! Durable `SQLite` backend for the engine evaluation cache.
//!
//! This is intentionally scoped to the engine cache key and assessment-id
//! index. It does not store LM responses, optimizer state, evidence payloads, or
//! run graph rows.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use leaven_kernel::{AssessmentId, Fingerprint, FingerprintBuilder, now};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;

use crate::{EvaluationCache, EvaluationCacheEntry, EvaluationCacheKey, EvaluationCacheSnapshot};

const SCHEMA_VERSION: i64 = 1;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Durable `SQLite` store for [`EvaluationCache`] entries.
#[derive(Clone)]
pub struct SqliteEvaluationCache {
    path: Arc<PathBuf>,
    connection: Arc<Mutex<Connection>>,
}

impl SqliteEvaluationCache {
    /// Opens or creates the `SQLite` evaluation-cache database at `path`.
    ///
    /// Parent directories are created automatically. The backend uses `SQLite`
    /// `user_version`, WAL mode where the platform accepts it, and a bounded
    /// busy timeout for local concurrent access.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationCacheStoreError`] if the database cannot be opened,
    /// the schema is incompatible, or `SQLite` refuses schema creation.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, EvaluationCacheStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| EvaluationCacheStoreError::Io {
                operation: "open",
                source,
            })?;
        }

        let mut connection =
            Connection::open(&path).map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "open",
                source,
            })?;
        configure_connection(&mut connection)?;

        Ok(Self {
            path: Arc::new(path),
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Returns the `SQLite` file backing this cache.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Reads one cache entry and increments its hit metadata when present.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationCacheStoreError`] if `SQLite` refuses the read or the
    /// stored row cannot be decoded as an evaluation-cache entry.
    pub fn get(
        &self,
        key: &EvaluationCacheKey,
    ) -> Result<Option<Vec<AssessmentId>>, EvaluationCacheStoreError> {
        let key_hash = key_hash(key);
        let mut connection = self.connection.lock();
        let transaction =
            connection
                .transaction()
                .map_err(|source| EvaluationCacheStoreError::Backend {
                    operation: "get",
                    source,
                })?;

        let row = transaction
            .query_row(
                "SELECT key_json, assessment_ids_json
                 FROM evaluation_cache_entries
                 WHERE key_hash = ?1",
                params![key_hash],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "get",
                source,
            })?;

        let Some((key_json, assessment_ids_json)) = row else {
            transaction
                .commit()
                .map_err(|source| EvaluationCacheStoreError::Backend {
                    operation: "get",
                    source,
                })?;
            return Ok(None);
        };

        let stored_key = decode_key(&key_json)?;
        if &stored_key != key {
            return Err(EvaluationCacheStoreError::Corrupt {
                operation: "get",
                reason: "stored evaluation cache key does not match requested key".to_owned(),
            });
        }
        let assessment_ids = decode_assessment_ids(&assessment_ids_json)?;

        transaction
            .execute(
                "UPDATE evaluation_cache_entries
                 SET last_hit_at = ?1, hit_count = hit_count + 1
                 WHERE key_hash = ?2",
                params![now().to_rfc3339(), key_hash],
            )
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "get",
                source,
            })?;
        transaction
            .commit()
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "get",
                source,
            })?;

        Ok(Some(assessment_ids))
    }

    /// Stores or replaces one evaluation-cache entry.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationCacheStoreError`] if the key or value cannot be
    /// encoded or `SQLite` refuses the write.
    pub fn insert(
        &self,
        key: &EvaluationCacheKey,
        assessment_ids: &[AssessmentId],
    ) -> Result<(), EvaluationCacheStoreError> {
        let mut connection = self.connection.lock();
        let transaction =
            connection
                .transaction()
                .map_err(|source| EvaluationCacheStoreError::Backend {
                    operation: "insert",
                    source,
                })?;
        let created_at = now().to_rfc3339();
        insert_entry(&transaction, key, assessment_ids, &created_at)?;
        transaction
            .commit()
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "insert",
                source,
            })?;
        Ok(())
    }

    /// Loads all `SQLite` rows into the in-memory engine cache shape.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationCacheStoreError`] if rows cannot be read or decoded.
    pub fn load_cache(&self) -> Result<EvaluationCache, EvaluationCacheStoreError> {
        Ok(EvaluationCache::from_snapshot(self.load_snapshot()?))
    }

    /// Loads all `SQLite` rows into a deterministic cache snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationCacheStoreError`] if rows cannot be read or decoded.
    pub fn load_snapshot(&self) -> Result<EvaluationCacheSnapshot, EvaluationCacheStoreError> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT key_hash, key_json, assessment_ids_json
                 FROM evaluation_cache_entries
                 ORDER BY key_hash",
            )
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "load",
                source,
            })?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "load",
                source,
            })?;

        let mut entries = Vec::new();
        for row in rows {
            let (stored_hash, key_json, assessment_ids_json) =
                row.map_err(|source| EvaluationCacheStoreError::Backend {
                    operation: "load",
                    source,
                })?;
            let key = decode_key(&key_json)?;
            let expected_hash = key_hash_from_json(&key_json);
            if stored_hash != expected_hash {
                return Err(EvaluationCacheStoreError::Corrupt {
                    operation: "load",
                    reason: "stored evaluation cache key hash does not match key bytes".to_owned(),
                });
            }
            entries.push(EvaluationCacheEntry {
                key,
                assessment_ids: decode_assessment_ids(&assessment_ids_json)?,
            });
        }
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(EvaluationCacheSnapshot { entries })
    }

    /// Replaces `SQLite` cache rows with `cache`'s current in-memory snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationCacheStoreError`] if the cache cannot be encoded or
    /// `SQLite` refuses the transaction.
    pub fn replace_from_cache(
        &self,
        cache: &EvaluationCache,
    ) -> Result<(), EvaluationCacheStoreError> {
        self.replace_from_snapshot(&cache.snapshot())
    }

    /// Replaces `SQLite` cache rows with `snapshot`.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationCacheStoreError`] if the snapshot cannot be encoded
    /// or `SQLite` refuses the transaction.
    pub fn replace_from_snapshot(
        &self,
        snapshot: &EvaluationCacheSnapshot,
    ) -> Result<(), EvaluationCacheStoreError> {
        let mut connection = self.connection.lock();
        let transaction =
            connection
                .transaction()
                .map_err(|source| EvaluationCacheStoreError::Backend {
                    operation: "replace",
                    source,
                })?;
        transaction
            .execute("DELETE FROM evaluation_cache_entries", [])
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "replace",
                source,
            })?;
        let created_at = now().to_rfc3339();
        for entry in &snapshot.entries {
            insert_entry(&transaction, &entry.key, &entry.assessment_ids, &created_at)?;
        }
        transaction
            .commit()
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "replace",
                source,
            })?;
        Ok(())
    }
}

/// Failure while opening, reading, or writing the `SQLite` evaluation cache.
#[derive(Debug, Error)]
pub enum EvaluationCacheStoreError {
    /// Filesystem operation failed before `SQLite` could open the database.
    #[error("evaluation cache sqlite failed to {operation}: {source}")]
    Io {
        /// Operation that was running.
        operation: &'static str,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// `SQLite` refused an operation.
    #[error("evaluation cache sqlite failed to {operation}: {source}")]
    Backend {
        /// Operation that was running.
        operation: &'static str,
        /// Underlying `SQLite` error.
        #[source]
        source: rusqlite::Error,
    },
    /// `SQLite` `user_version` is not understood by this crate.
    #[error(
        "evaluation cache sqlite schema version {found} is not supported by version {supported}"
    )]
    SchemaVersion {
        /// Version found in `SQLite` `user_version`.
        found: i64,
        /// Version supported by this implementation.
        supported: i64,
    },
    /// Stored JSON could not be decoded.
    #[error("evaluation cache sqlite failed to {operation}: {source}")]
    Codec {
        /// Operation that was running.
        operation: &'static str,
        /// Underlying JSON codec error.
        #[source]
        source: serde_json::Error,
    },
    /// Stored rows passed the `SQLite` schema but violated the cache contract.
    #[error("evaluation cache sqlite row is corrupt during {operation}: {reason}")]
    Corrupt {
        /// Operation that was running.
        operation: &'static str,
        /// Human-readable invariant violation.
        reason: String,
    },
}

fn configure_connection(connection: &mut Connection) -> Result<(), EvaluationCacheStoreError> {
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|source| EvaluationCacheStoreError::Backend {
            operation: "open",
            source,
        })?;

    let _ = connection.pragma_update(None, "journal_mode", "WAL");
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|source| EvaluationCacheStoreError::Backend {
            operation: "open",
            source,
        })?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|source| EvaluationCacheStoreError::Backend {
            operation: "open",
            source,
        })?;

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|source| EvaluationCacheStoreError::Backend {
            operation: "open",
            source,
        })?;

    match version {
        0 => create_schema(connection),
        SCHEMA_VERSION => Ok(()),
        found => Err(EvaluationCacheStoreError::SchemaVersion {
            found,
            supported: SCHEMA_VERSION,
        }),
    }
}

fn create_schema(connection: &mut Connection) -> Result<(), EvaluationCacheStoreError> {
    let transaction =
        connection
            .transaction()
            .map_err(|source| EvaluationCacheStoreError::Backend {
                operation: "open",
                source,
            })?;
    transaction
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS evaluation_cache_entries (
                key_hash TEXT PRIMARY KEY NOT NULL,
                key_json TEXT NOT NULL,
                assessment_ids_json TEXT NOT NULL,
                evaluator_fingerprint TEXT NOT NULL,
                case_set_version TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_hit_at TEXT,
                hit_count INTEGER NOT NULL DEFAULT 0 CHECK (hit_count >= 0)
            );

            CREATE INDEX IF NOT EXISTS evaluation_cache_entries_evaluator_case_set
                ON evaluation_cache_entries (evaluator_fingerprint, case_set_version);

            PRAGMA user_version = 1;
            ",
        )
        .map_err(|source| EvaluationCacheStoreError::Backend {
            operation: "open",
            source,
        })?;
    transaction
        .commit()
        .map_err(|source| EvaluationCacheStoreError::Backend {
            operation: "open",
            source,
        })?;
    Ok(())
}

fn insert_entry(
    transaction: &Transaction<'_>,
    key: &EvaluationCacheKey,
    assessment_ids: &[AssessmentId],
    created_at: &str,
) -> Result<(), EvaluationCacheStoreError> {
    let key_json = serde_json::to_string(key).expect("evaluation cache keys are JSON-serializable");
    let assessment_ids_json =
        serde_json::to_string(assessment_ids).expect("assessment id vectors are JSON-serializable");
    transaction
        .execute(
            "INSERT INTO evaluation_cache_entries
                (key_hash, key_json, assessment_ids_json, evaluator_fingerprint, case_set_version,
                 created_at, last_hit_at, hit_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 0)
             ON CONFLICT(key_hash) DO UPDATE SET
                key_json = excluded.key_json,
                assessment_ids_json = excluded.assessment_ids_json,
                evaluator_fingerprint = excluded.evaluator_fingerprint,
                case_set_version = excluded.case_set_version,
                created_at = evaluation_cache_entries.created_at",
            params![
                key_hash_from_json(&key_json),
                key_json,
                assessment_ids_json,
                hex_fingerprint(key.evaluator),
                key.case_set_version.0.as_str(),
                created_at,
            ],
        )
        .map_err(|source| EvaluationCacheStoreError::Backend {
            operation: "insert",
            source,
        })?;
    Ok(())
}

fn decode_key(key_json: &str) -> Result<EvaluationCacheKey, EvaluationCacheStoreError> {
    serde_json::from_str(key_json).map_err(|source| EvaluationCacheStoreError::Codec {
        operation: "decode key",
        source,
    })
}

fn decode_assessment_ids(
    assessment_ids_json: &str,
) -> Result<Vec<AssessmentId>, EvaluationCacheStoreError> {
    serde_json::from_str(assessment_ids_json).map_err(|source| EvaluationCacheStoreError::Codec {
        operation: "decode assessment ids",
        source,
    })
}

fn key_hash(key: &EvaluationCacheKey) -> String {
    let key_json = serde_json::to_string(key).expect("evaluation cache keys are JSON-serializable");
    key_hash_from_json(&key_json)
}

fn key_hash_from_json(key_json: &str) -> String {
    let mut builder = FingerprintBuilder::new();
    builder.update(b"leaven-engine/evaluation-cache-key/v1\0");
    builder.update(key_json.as_bytes());
    hex_fingerprint(builder.finish())
}

fn hex_fingerprint(fingerprint: Fingerprint) -> String {
    let mut hash = String::with_capacity(64);
    for byte in fingerprint.0 {
        write!(&mut hash, "{byte:02x}").expect("writing to string cannot fail");
    }
    hash
}
