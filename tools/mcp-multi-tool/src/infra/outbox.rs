use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug)]
enum Backend {
    File { main_path: PathBuf },
    Sqlite { conn: Mutex<Connection> },
}

#[derive(Debug)]
pub struct Outbox {
    backend: Backend,
    dlq_path: PathBuf,
    write_lock: Mutex<()>,
}

impl Outbox {
    pub fn file<P: Into<PathBuf>, Q: Into<PathBuf>>(main_path: P, dlq_path: Q) -> Result<Self> {
        let main_path = main_path.into();
        let dlq_path = dlq_path.into();
        Self::ensure_parent(&main_path)?;
        Self::ensure_parent(&dlq_path)?;
        Ok(Self {
            backend: Backend::File {
                main_path: main_path.clone(),
            },
            dlq_path,
            write_lock: Mutex::new(()),
        })
    }

    pub fn sqlite<P: Into<PathBuf>, Q: Into<PathBuf>>(db_path: P, dlq_path: Q) -> Result<Self> {
        let db_path = db_path.into();
        let dlq_path = dlq_path.into();
        if let Some(parent) = db_path.parent() {
            create_dir_all(parent).with_context(|| {
                format!(
                    "creating directories for sqlite outbox {}",
                    parent.display()
                )
            })?;
        }
        Self::ensure_parent(&dlq_path)?;

        let conn = Connection::open(&db_path)
            .with_context(|| format!("open sqlite outbox {}", db_path.display()))?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS outbox_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );
            "#,
        )
        .context("initialise sqlite outbox schema")?;

        Ok(Self {
            backend: Backend::Sqlite {
                conn: Mutex::new(conn),
            },
            dlq_path,
            write_lock: Mutex::new(()),
        })
    }

    pub fn append<T: Serialize>(&self, event: &T) -> Result<()> {
        let line = serde_json::to_string(event).context("serialize outbox event")?;
        let event_id = extract_event_id(event).unwrap_or_else(uuid::Uuid::new_v4);
        let _guard = self.write_lock.lock();

        let primary_result = match &self.backend {
            Backend::File { main_path } => Self::write_line(main_path, &line),
            Backend::Sqlite { conn } => {
                let conn = conn.lock();
                conn.execute(
                    "INSERT INTO outbox_events (event_id, payload) VALUES (?1, ?2)",
                    params![event_id.to_string(), line],
                )
                .context("insert sqlite outbox row")
                .map(|_| ())
            }
        };

        if let Err(primary_err) = primary_result {
            Self::write_line(&self.dlq_path, &line)
                .context("write outbox DLQ after primary failure")?;
            Err(primary_err)
        } else {
            Ok(())
        }
    }

    fn write_line(path: &Path, line: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("open outbox file {}", path.display()))?;
        writeln!(file, "{line}").with_context(|| format!("append outbox line {}", path.display()))
    }

    fn ensure_parent(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .with_context(|| format!("create outbox directory {}", parent.display()))?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn backend_description(&self) -> &'static str {
        match &self.backend {
            Backend::File { .. } => "file",
            Backend::Sqlite { .. } => "sqlite",
        }
    }
}

fn extract_event_id<T: Serialize>(event: &T) -> Option<uuid::Uuid> {
    let value = serde_json::to_value(event).ok()?;
    value
        .get("event_id")
        .and_then(|v| v.as_str())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use tempfile::tempdir;

    #[derive(Serialize)]
    struct DummyEvent {
        event_id: String,
        payload: String,
    }

    #[test]
    fn file_backend_appends() -> Result<()> {
        let dir = tempdir()?;
        let primary = dir.path().join("events.jsonl");
        let dlq = dir.path().join("dlq.jsonl");
        let outbox = Outbox::file(&primary, &dlq)?;
        assert_eq!("file", outbox.backend_description());
        let event = DummyEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            payload: "hello".into(),
        };
        outbox.append(&event)?;
        let data = std::fs::read_to_string(primary)?;
        assert!(data.contains("hello"));
        Ok(())
    }

    #[test]
    fn sqlite_backend_appends() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("outbox.db");
        let dlq = dir.path().join("dlq.jsonl");
        let outbox = Outbox::sqlite(&db_path, &dlq)?;
        assert_eq!("sqlite", outbox.backend_description());
        let event = DummyEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            payload: "world".into(),
        };
        outbox.append(&event)?;
        let conn = Connection::open(db_path)?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM outbox_events", [], |row| row.get(0))?;
        assert_eq!(1, count);
        Ok(())
    }
}
