use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Outbox {
    main_path: PathBuf,
    dlq_path: PathBuf,
    lock: Mutex<()>,
}

impl Outbox {
    pub fn new<P: Into<PathBuf>, Q: Into<PathBuf>>(main_path: P, dlq_path: Q) -> Result<Self> {
        let outbox = Self {
            main_path: main_path.into(),
            dlq_path: dlq_path.into(),
            lock: Mutex::new(()),
        };
        outbox.ensure_parent_dirs(&outbox.main_path)?;
        outbox.ensure_parent_dirs(&outbox.dlq_path)?;
        Ok(outbox)
    }

    fn ensure_parent_dirs(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .with_context(|| format!("creating outbox directory {}", parent.display()))?;
        }
        Ok(())
    }

    pub fn append<T: Serialize>(&self, event: &T) -> Result<()> {
        let line = serde_json::to_string(event).context("serialize outbox event")?;
        let _guard = self.lock.lock();
        if let Err(main_err) = Self::write_line(&self.main_path, &line) {
            // fallback to DLQ
            Self::write_line(&self.dlq_path, &line)
                .context("write to outbox DLQ after primary failure")?;
            Err(main_err.context("write to outbox primary file"))
        } else {
            Ok(())
        }
    }

    fn write_line(path: &Path, line: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening outbox file {}", path.display()))?;
        writeln!(file, "{line}").with_context(|| format!("writing outbox line {}", path.display()))
    }
}
