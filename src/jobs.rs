use crate::error::{Result, XScraperError};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobItem {
    pub operation: String,
    pub target: String,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingJobItem {
    pub id: i64,
    pub operation: String,
    pub target: String,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCheckpoint {
    pub operation: String,
    pub cursor: Option<String>,
    #[serde(rename = "lastSeenId")]
    pub last_seen_id: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct JobCheckpointStore {
    path: PathBuf,
}

impl JobItem {
    pub fn new(
        operation: impl Into<String>,
        target: impl Into<String>,
        cursor: Option<String>,
    ) -> Self {
        Self { operation: operation.into(), target: target.into(), cursor }
    }
}

impl JobCheckpointStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn create_job(&self, name: &str) -> Result<i64> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO jobs (name, status, created_at) VALUES (?1, 'pending', ?2)",
            params![name, Utc::now().to_rfc3339()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn enqueue_item(&self, job_id: i64, item: JobItem) -> Result<bool> {
        let conn = self.connect()?;
        let fingerprint = item_fingerprint(&item);
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO job_items (
                job_id, operation, target, cursor, fingerprint, status, attempts
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 0)",
            params![job_id, item.operation, item.target, item.cursor, fingerprint],
        )?;
        Ok(inserted == 1)
    }

    pub fn pending_items(&self, job_id: i64) -> Result<Vec<PendingJobItem>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, operation, target, cursor
             FROM job_items WHERE job_id = ?1 AND status = 'pending' ORDER BY id",
        )?;
        let rows = stmt.query_map(params![job_id], |row| {
            Ok(PendingJobItem {
                id: row.get("id")?,
                operation: row.get("operation")?,
                target: row.get("target")?,
                cursor: row.get("cursor")?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn mark_checkpoint(
        &self,
        job_id: i64,
        operation: &str,
        cursor: Option<&str>,
        last_seen_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO job_checkpoints (job_id, operation, cursor, last_seen_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(job_id, operation) DO UPDATE SET
                cursor = excluded.cursor,
                last_seen_id = excluded.last_seen_id,
                updated_at = excluded.updated_at",
            params![job_id, operation, cursor, last_seen_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn checkpoint(&self, job_id: i64, operation: &str) -> Result<Option<JobCheckpoint>> {
        let conn = self.connect()?;
        conn.query_row(
            "SELECT operation, cursor, last_seen_id, updated_at
             FROM job_checkpoints WHERE job_id = ?1 AND operation = ?2",
            params![job_id, operation],
            |row| {
                let updated_at: String = row.get("updated_at")?;
                let updated_at = DateTime::parse_from_rfc3339(&updated_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(JobCheckpoint {
                    operation: row.get("operation")?,
                    cursor: row.get("cursor")?,
                    last_seen_id: row.get("last_seen_id")?,
                    updated_at,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    fn connect(&self) -> Result<Connection> {
        if let Some(parent) = Path::new(&self.path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|source| XScraperError::io(parent, source))?;
        }
        let conn = Connection::open(&self.path)?;
        conn.busy_timeout(Duration::from_secs(30))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&conn)?;
        Ok(conn)
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS job_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id INTEGER NOT NULL,
            operation TEXT NOT NULL,
            target TEXT NOT NULL,
            cursor TEXT DEFAULT NULL,
            fingerprint TEXT NOT NULL,
            status TEXT NOT NULL,
            attempts INTEGER DEFAULT 0 NOT NULL,
            evidence_ref TEXT DEFAULT NULL,
            UNIQUE(job_id, fingerprint)
        );
        CREATE TABLE IF NOT EXISTS job_checkpoints (
            job_id INTEGER NOT NULL,
            operation TEXT NOT NULL,
            cursor TEXT DEFAULT NULL,
            last_seen_id TEXT DEFAULT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(job_id, operation)
        );",
    )?;
    Ok(())
}

fn item_fingerprint(item: &JobItem) -> String {
    let raw =
        format!("{}:{}:{}", item.operation, item.target, item.cursor.as_deref().unwrap_or(""));
    Sha256::digest(raw.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}
