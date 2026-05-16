use std::fmt::Write as _;
use std::fs;
use std::future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use leaven_kernel::now;
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{LmCacheEntry, LmCacheError, LmCacheKey, LmCacheStore};

const SCHEMA_VERSION: i64 = 2;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Durable `SQLite` response-cache backend.
#[derive(Clone)]
pub struct SqliteLmCache {
    path: Arc<PathBuf>,
    connection: Arc<Mutex<Connection>>,
}

impl SqliteLmCache {
    /// Opens or creates a `SQLite` LM response cache at `path`.
    ///
    /// Parent directories are created automatically. The cache uses `SQLite`
    /// `user_version` for schema compatibility, asks `SQLite` for WAL mode, and
    /// installs a bounded busy timeout for local concurrent access.
    ///
    /// # Errors
    ///
    /// Returns [`LmCacheError`] if the parent directory cannot be created, the
    /// database cannot be opened, or its schema is newer than this crate
    /// understands.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LmCacheError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| LmCacheError::backend("open", error.to_string()))?;
        }

        let mut connection = Connection::open(&path)
            .map_err(|error| LmCacheError::backend("open", error.to_string()))?;
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

    fn get_sync(&self, key: LmCacheKey) -> Result<Option<LmCacheEntry>, LmCacheError> {
        let key_hash = key_hash(&key);
        let mut connection = self.connection.lock();
        let transaction = connection
            .transaction()
            .map_err(|error| LmCacheError::backend("get", error.to_string()))?;

        let entry_json = transaction
            .query_row(
                "SELECT entry_json FROM lm_cache_entries WHERE key_hash = ?1",
                params![key_hash],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| LmCacheError::backend("get", error.to_string()))?;

        let Some(entry_json) = entry_json else {
            transaction
                .commit()
                .map_err(|error| LmCacheError::backend("get", error.to_string()))?;
            return Ok(None);
        };

        let entry: LmCacheEntry = serde_json::from_str(&entry_json)
            .map_err(|error| LmCacheError::codec(error.to_string()))?;
        if entry.key != key {
            return Err(LmCacheError::codec(
                "stored lm cache entry key does not match requested key",
            ));
        }

        transaction
            .execute(
                "UPDATE lm_cache_entries
                 SET last_hit_at = ?1, hit_count = hit_count + 1
                 WHERE key_hash = ?2",
                params![now().to_rfc3339(), key_hash],
            )
            .map_err(|error| LmCacheError::backend("get", error.to_string()))?;
        transaction
            .commit()
            .map_err(|error| LmCacheError::backend("get", error.to_string()))?;

        Ok(Some(entry))
    }

    fn put_sync(&self, key: LmCacheKey, entry: &LmCacheEntry) -> Result<(), LmCacheError> {
        if entry.key != key {
            return Err(LmCacheError::codec(
                "lm cache entry key does not match write key",
            ));
        }

        let key_hash = key_hash(&key);
        let key_json =
            serde_json::to_string(&key).map_err(|error| LmCacheError::codec(error.to_string()))?;
        let entry_json = serde_json::to_string(&entry)
            .map_err(|error| LmCacheError::codec(error.to_string()))?;
        let request_json = serde_json::to_string(&entry.request)
            .map_err(|error| LmCacheError::codec(error.to_string()))?;
        let response_json = serde_json::to_string(&entry.response)
            .map_err(|error| LmCacheError::codec(error.to_string()))?;
        let provider_fingerprint = fingerprint_hex(&entry.provider_fingerprint);
        let model = entry.request.model.as_str();

        let mut connection = self.connection.lock();
        let transaction = connection
            .transaction()
            .map_err(|error| LmCacheError::backend("put", error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO lm_cache_entries
                    (key_hash, key_json, provider_fingerprint, model, request_json, entry_json,
                     response_json, stored_at, last_hit_at, hit_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, 0)
                 ON CONFLICT(key_hash) DO UPDATE SET
                    key_json = excluded.key_json,
                    provider_fingerprint = excluded.provider_fingerprint,
                    model = excluded.model,
                    request_json = excluded.request_json,
                    entry_json = excluded.entry_json,
                    response_json = excluded.response_json,
                    stored_at = excluded.stored_at,
                    last_hit_at = NULL,
                    hit_count = 0",
                params![
                    key_hash,
                    key_json,
                    provider_fingerprint,
                    model,
                    request_json,
                    entry_json,
                    response_json,
                    entry.stored_at.to_rfc3339()
                ],
            )
            .map_err(|error| LmCacheError::backend("put", error.to_string()))?;
        transaction
            .commit()
            .map_err(|error| LmCacheError::backend("put", error.to_string()))?;

        Ok(())
    }
}

impl LmCacheStore for SqliteLmCache {
    fn get(
        &self,
        key: LmCacheKey,
    ) -> impl Future<Output = Result<Option<LmCacheEntry>, LmCacheError>> + Send + '_ {
        future::ready(self.get_sync(key))
    }

    fn put(
        &self,
        key: LmCacheKey,
        entry: LmCacheEntry,
    ) -> impl Future<Output = Result<(), LmCacheError>> + Send + '_ {
        future::ready(self.put_sync(key, &entry))
    }
}

fn configure_connection(connection: &mut Connection) -> Result<(), LmCacheError> {
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|error| LmCacheError::backend("open", error.to_string()))?;

    let _ = connection.pragma_update(None, "journal_mode", "WAL");
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|error| LmCacheError::backend("open", error.to_string()))?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| LmCacheError::backend("open", error.to_string()))?;

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| LmCacheError::backend("open", error.to_string()))?;

    match version {
        0 => create_schema(connection),
        SCHEMA_VERSION => Ok(()),
        version => Err(LmCacheError::backend(
            "open",
            format!(
                "lm cache sqlite schema version {version} is not supported by version {SCHEMA_VERSION}"
            ),
        )),
    }
}

fn create_schema(connection: &mut Connection) -> Result<(), LmCacheError> {
    let transaction = connection
        .transaction()
        .map_err(|error| LmCacheError::backend("open", error.to_string()))?;
    transaction
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS lm_cache_entries (
                key_hash TEXT PRIMARY KEY NOT NULL,
                key_json TEXT NOT NULL,
                provider_fingerprint TEXT NOT NULL,
                model TEXT NOT NULL,
                request_json TEXT NOT NULL,
                entry_json TEXT NOT NULL,
                response_json TEXT NOT NULL,
                stored_at TEXT NOT NULL,
                last_hit_at TEXT,
                hit_count INTEGER NOT NULL DEFAULT 0 CHECK (hit_count >= 0)
            );

            CREATE INDEX IF NOT EXISTS idx_lm_cache_entries_model
                ON lm_cache_entries(model);

            CREATE INDEX IF NOT EXISTS idx_lm_cache_entries_stored_at
                ON lm_cache_entries(stored_at);

            PRAGMA user_version = 2;
            ",
        )
        .map_err(|error| LmCacheError::backend("open", error.to_string()))?;
    transaction
        .commit()
        .map_err(|error| LmCacheError::backend("open", error.to_string()))?;

    Ok(())
}

fn key_hash(key: &LmCacheKey) -> String {
    fingerprint_hex(&key.fingerprint)
}

fn fingerprint_hex(fingerprint: &leaven_kernel::Fingerprint) -> String {
    let mut hash = String::with_capacity(64);
    for byte in fingerprint.0 {
        write!(&mut hash, "{byte:02x}").expect("writing to string cannot fail");
    }
    hash
}
