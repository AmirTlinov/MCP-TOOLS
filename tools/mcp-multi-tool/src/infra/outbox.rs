use crate::infra::metrics;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

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
        let wait = Instant::now();
        let _guard = self.write_lock.lock();
        metrics::observe_lock_wait("outbox_write_lock", wait.elapsed());

        let primary_result = match &self.backend {
            Backend::File { main_path } => Self::write_line(main_path, &line),
            Backend::Sqlite { conn } => {
                let wait = Instant::now();
                let conn = conn.lock();
                metrics::observe_lock_wait("outbox_sqlite_conn", wait.elapsed());
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
            metrics::increment_outbox_backlog();
            Ok(())
        }
    }

    fn write_line(path: &Path, line: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("open outbox file {}", path.display()))?;
        writeln!(file, "{line}")
            .with_context(|| format!("append outbox line {}", path.display()))?;
        file.sync_all()
            .with_context(|| format!("fsync outbox file {}", path.display()))
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
    use serde::{Deserialize, Serialize};
    use std::{sync::Arc, thread};
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

    #[test]
    fn file_backend_fallbacks_to_dlq() -> Result<()> {
        let dir = tempdir()?;
        let primary_dir = dir.path().join("primary_dir");
        std::fs::create_dir_all(&primary_dir)?;
        let dlq = dir.path().join("dlq.jsonl");
        let outbox = Outbox::file(&primary_dir, &dlq)?;
        let event = DummyEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            payload: "fallback".into(),
        };
        let result = outbox.append(&event);
        assert!(
            result.is_err(),
            "primary append should fail for directory path"
        );
        let dlq_data = std::fs::read_to_string(&dlq)?;
        assert!(dlq_data.contains("fallback"));
        Ok(())
    }

    #[test]
    fn file_backend_persists_across_reopen() -> Result<()> {
        let dir = tempdir()?;
        let primary = dir.path().join("events.jsonl");
        let dlq = dir.path().join("dlq.jsonl");
        let outbox = Outbox::file(&primary, &dlq)?;
        let event = DummyEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            payload: "persist".into(),
        };
        outbox.append(&event)?;
        drop(outbox);

        let data = std::fs::read_to_string(&primary)?;
        assert!(data.contains("persist"));

        // reopening must succeed and continue writing without data loss
        let reopened = Outbox::file(&primary, &dlq)?;
        assert_eq!("file", reopened.backend_description());
        Ok(())
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct PersistedEvent {
        event_id: String,
        payload: String,
    }

    #[test]
    fn file_backend_concurrent_appends_no_loss() -> Result<()> {
        let dir = tempdir()?;
        let primary = dir.path().join("events.jsonl");
        let dlq = dir.path().join("dlq.jsonl");
        let outbox = Arc::new(Outbox::file(&primary, &dlq)?);
        let workers = 32;
        thread::scope(|scope| {
            for idx in 0..workers {
                let outbox = Arc::clone(&outbox);
                scope.spawn(move || {
                    let event = DummyEvent {
                        event_id: uuid::Uuid::new_v4().to_string(),
                        payload: format!("payload-{idx}"),
                    };
                    outbox.append(&event).expect("append");
                });
            }
        });

        let data = std::fs::read_to_string(&primary)?;
        let persisted: Vec<PersistedEvent> = data
            .lines()
            .map(|line| serde_json::from_str(line))
            .collect::<Result<_, _>>()?;
        assert_eq!(persisted.len(), workers as usize);
        Ok(())
    }

    #[test]
    fn sqlite_backend_persists_across_reopen() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("outbox.db");
        let dlq = dir.path().join("dlq.jsonl");
        {
            let outbox = Outbox::sqlite(&db_path, &dlq)?;
            let event = DummyEvent {
                event_id: uuid::Uuid::new_v4().to_string(),
                payload: "persist".into(),
            };
            outbox.append(&event)?;
        }

        let conn = Connection::open(&db_path)?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM outbox_events", [], |row| row.get(0))?;
        assert_eq!(count, 1);

        // reopen should retain ability to append
        let reopened = Outbox::sqlite(&db_path, &dlq)?;
        assert_eq!("sqlite", reopened.backend_description());
        let event = DummyEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            payload: "second".into(),
        };
        reopened.append(&event)?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM outbox_events", [], |row| row.get(0))?;
        assert_eq!(count, 2);
        Ok(())
    }

    #[test]
    fn sqlite_backend_concurrent_appends_no_loss() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("outbox.db");
        let dlq = dir.path().join("dlq.jsonl");
        let outbox = Arc::new(Outbox::sqlite(&db_path, &dlq)?);
        let workers = 32;
        thread::scope(|scope| {
            for idx in 0..workers {
                let outbox = Arc::clone(&outbox);
                scope.spawn(move || {
                    let event = DummyEvent {
                        event_id: uuid::Uuid::new_v4().to_string(),
                        payload: format!("payload-{idx}"),
                    };
                    outbox.append(&event).expect("append sqlite");
                });
            }
        });

        let conn = Connection::open(&db_path)?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM outbox_events", [], |row| row.get(0))?;
        assert_eq!(count, workers as i64);
        Ok(())
    }
}
