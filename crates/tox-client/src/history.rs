// Local chat history storage (append-only JSONL per friend).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

// One stored chat line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    // Unix timestamp (seconds).
    pub ts: i64,
    // `true` if the local user sent it.
    pub outgoing: bool,
    // `true` for `/me`-style actions.
    #[serde(default)]
    pub action: bool,
    pub text: String,
    // Optional tox message id for outgoing messages.
    #[serde(default)]
    pub message_id: Option<u32>,
}

impl HistoryEntry {
    pub fn now(outgoing: bool, action: bool, text: impl Into<String>, message_id: Option<u32>) -> Self {
        Self {
            ts: Utc::now().timestamp(),
            outgoing,
            action,
            text: text.into(),
            message_id,
        }
    }

    pub fn time_label(&self) -> String {
        let dt: DateTime<Utc> = chrono::DateTime::from_timestamp(self.ts, 0)
            .unwrap_or_else(Utc::now);
        dt.format("%H:%M").to_string()
    }
}

// JSONL-backed history, keyed by friend public key (hex).
pub struct HistoryStore {
    dir: PathBuf,
    cache: HashMap<String, Vec<HistoryEntry>>,
}

impl HistoryStore {
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        let dir = data_dir.as_ref().join("history");
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            cache: HashMap::new(),
        })
    }

    fn file_for(&self, friend_key: &str) -> PathBuf {
        // friend_key is hex; safe as a filename.
        self.dir.join(format!("{friend_key}.jsonl"))
    }

    // Load (or return cached) history for a friend.
    pub fn load(&mut self, friend_key: &str) -> Result<&[HistoryEntry]> {
        if !self.cache.contains_key(friend_key) {
            let path = self.file_for(friend_key);
            let mut entries = Vec::new();
            if path.exists() {
                let f = std::fs::File::open(&path)?;
                for line in BufReader::new(f).lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
                        entries.push(entry);
                    }
                }
            }
            self.cache.insert(friend_key.to_string(), entries);
        }
        Ok(self.cache.get(friend_key).map(|v| v.as_slice()).unwrap_or(&[]))
    }

    // Append one entry to disk and cache.
    pub fn append(&mut self, friend_key: &str, entry: HistoryEntry) -> Result<()> {
        let path = self.file_for(friend_key);
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open {}", path.display()))?;
        writeln!(f, "{}", serde_json::to_string(&entry)?)?;
        self.cache
            .entry(friend_key.to_string())
            .or_default()
            .push(entry);
        Ok(())
    }

    // Last `n` entries (most recent), for rendering.
    pub fn tail(&self, friend_key: &str, n: usize) -> Vec<HistoryEntry> {
        let empty: Vec<HistoryEntry> = Vec::new();
        let entries = self.cache.get(friend_key).unwrap_or(&empty);
        let start = entries.len().saturating_sub(n);
        entries[start..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_reload() {
        let dir = std::env::temp_dir().join(format!("tox-hist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut store = HistoryStore::new(&dir).unwrap();
        store
            .append("aabb", HistoryEntry::now(true, false, "hello", Some(1)))
            .unwrap();
        store
            .append("aabb", HistoryEntry::now(false, false, "hi!", None))
            .unwrap();

        // New store instance forces a disk reload.
        let mut store2 = HistoryStore::new(&dir).unwrap();
        let entries = store2.load("aabb").unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].outgoing);
        assert_eq!(entries[1].text, "hi!");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
