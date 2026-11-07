// Identity (Tox ID / keys) persistence.

use crate::private_file::{create_dir_private, write_private};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// Public-facing identity material. The secret key lives only in tox.save.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    // Full Tox ID as 76 hex chars (address).
    pub address: String,
    // Long-term public key (64 hex chars).
    pub public_key: String,
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
        create_dir_private(dir).with_context(|| format!("create data dir {}", dir.display()))?;
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
        write_private(&tmp, serde_json::to_string_pretty(identity)?.as_bytes())?;
        std::fs::rename(&tmp, &self.path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
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
        };
        store.save(&id).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.address, id.address);
        assert_eq!(loaded.public_key, id.public_key);

        // never write a secret key into identity.json
        let raw = std::fs::read_to_string(store.path()).unwrap();
        assert!(
            !raw.contains("secret"),
            "identity.json must not contain secret_key"
        );
        assert!(!raw.contains("EF".repeat(32).as_str()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_secret_key_field_is_ignored() {
        let dir = std::env::temp_dir().join(format!("tox-id-leg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = format!(
            r#"{{"address":"{}","public_key":"{}","secret_key":"{}"}}"#,
            "AB".repeat(38),
            "CD".repeat(32),
            "EF".repeat(32),
        );
        std::fs::write(dir.join("identity.json"), legacy).unwrap();

        let store = IdentityStore::new(&dir).unwrap();
        let id = store.load().unwrap().expect("legacy file should load");
        assert_eq!(id.address, "AB".repeat(38));

        // re-save strips the secret
        store.save(&id).unwrap();
        let raw = std::fs::read_to_string(store.path()).unwrap();
        assert!(!raw.contains("secret_key"));
        assert!(!raw.contains("EF".repeat(32).as_str()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn identity_file_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("tox-id-perm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = IdentityStore::new(&dir).unwrap();
        store
            .save(&Identity {
                address: "AA".repeat(38),
                public_key: "BB".repeat(32),
            })
            .unwrap();
        let mode = std::fs::metadata(store.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
