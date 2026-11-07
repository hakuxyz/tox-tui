// Identity (Tox ID / keys) persistence.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Public-facing identity material.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    // Full Tox ID as 76 hex chars (address).
    pub address: String,
    // Long-term public key (64 hex chars).
    pub public_key: String,
    // Secret key (64 hex chars). Kept for backup export; tox.save already holds it.
    #[serde(default)]
    pub secret_key: Option<String>,
}

impl Identity {
    pub fn short_id(&self) -> &str {
        let n = self.address.len().min(8);
        &self.address[..n]
    }
}

// Loads/saves identity metadata next to the toxcore savedata blob.
pub struct IdentityStore {
    path: PathBuf,
}

impl IdentityStore {
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        let dir = data_dir.as_ref();
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create data dir {}", dir.display()))?;
        Ok(Self {
            path: dir.join("identity.json"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Option<Identity>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let id: Identity = serde_json::from_str(&raw).context("parse identity.json")?;
        Ok(Some(id))
    }

    pub fn save(&self, identity: &Identity) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(identity)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_identity() {
        let dir = std::env::temp_dir().join(format!("tox-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = IdentityStore::new(&dir).unwrap();
        assert!(store.load().unwrap().is_none());

        let id = Identity {
            address: "AB".repeat(38),
            public_key: "CD".repeat(32),
            secret_key: Some("EF".repeat(32)),
        };
        store.save(&id).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.address, id.address);
        assert_eq!(loaded.public_key, id.public_key);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
