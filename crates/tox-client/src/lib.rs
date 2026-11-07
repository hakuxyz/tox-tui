// safe async wrapper around c-toxcore: ToxCommand in, ToxEvent out

mod commands;
mod events;
mod history;
mod identity;
mod private_file;
mod runner;

pub use commands::{FileSendRequest, ToxCommand};
pub use events::{ConnectionStatus, FriendEvent, FriendInfo, ToxEvent, UserStatus};
pub use history::{HistoryEntry, HistoryStore};
pub use identity::{Identity, IdentityStore};
pub use runner::{sanitize_filename, unique_path_for};

use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;

// Well-known public DHT bootstrap nodes (host, port, hex DHT public key).
pub const DEFAULT_BOOTSTRAP_NODES: &[(&str, u16, &str)] = &[
    (
        "144.217.167.73",
        33445,
        "7E5668E0EE09E19F320AD47902419331FFEE147BB3606769CFBE921A2A2FD34C",
    ),
    (
        "tox.abilinski.com",
        33445,
        "10C00EB250C3233E343E2AEBA07115A5C28920E9C8D29492F6D00B29049EDC7E",
    ),
    (
        "45.32.184.23",
        33445,
        "81C916A3605724106C2E7487DC72FF2F9EB662EB34C85C0C692D8312B442635C",
    ),
    (
        "tox1.mf-net.eu",
        33445,
        "B3E5FA80DC8EBD1149AD2AB35ED8B85BD546DEDE261CA593234C619249419506",
    ),
    (
        "5.19.249.240",
        38296,
        "DA98A4C0CD7473A133E115FEA2EBDAEEA2EF4F79FD69325FC070DA4DE4BA3238",
    ),
    (
        "3.0.24.15",
        33445,
        "E20ABCF38CDBFFD7D04B29C956B33F7B27A3BB7AF0618101617B036E4AEA402D",
    ),
];

// Handle used by the UI to talk to the running Tox core.
pub struct ToxHandle {
    cmd_tx: mpsc::UnboundedSender<ToxCommand>,
    events: mpsc::UnboundedReceiver<ToxEvent>,
    identity: Identity,
}

impl ToxHandle {
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn send(&self, cmd: ToxCommand) -> Result<()> {
        self.cmd_tx
            .send(cmd)
            .map_err(|_| anyhow::anyhow!("tox core has shut down"))?;
        Ok(())
    }

    // Non-blocking poll of the next network event.
    pub async fn recv_event(&mut self) -> Option<ToxEvent> {
        self.events.recv().await
    }

    pub fn try_recv_event(&mut self) -> Option<ToxEvent> {
        self.events.try_recv().ok()
    }
}

// Configuration for spawning the core.
#[derive(Debug, Clone)]
pub struct ToxConfig {
    // Directory for `tox.save` and other state.
    pub data_dir: PathBuf,
    // Bootstrap nodes: (host, port, hex DHT key).
    pub bootstrap: Vec<(String, u16, String)>,
    // Optional display name set on first run.
    pub name: Option<String>,
    // Optional status message set on first run.
    pub status_message: Option<String>,
}

impl Default for ToxConfig {
    fn default() -> Self {
        Self {
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("tox-tui"),
            bootstrap: DEFAULT_BOOTSTRAP_NODES
                .iter()
                .map(|(h, p, k)| (h.to_string(), *p, k.to_string()))
                .collect(),
            name: None,
            status_message: None,
        }
    }
}

// Spawn the toxcore event loop. Returns a handle for the UI.
pub async fn spawn(config: ToxConfig) -> Result<ToxHandle> {
    runner::spawn(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_keys_are_valid_hex_32() {
        for (host, port, key) in DEFAULT_BOOTSTRAP_NODES {
            let bytes = hex::decode(key).expect("hex");
            assert_eq!(bytes.len(), 32, "bad key length for {host}:{port}");
            assert!(*port > 0);
        }
    }
}
