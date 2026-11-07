// Commands the UI sends into the tox core.

// A file the user wants to send.
#[derive(Debug, Clone)]
pub struct FileSendRequest {
    pub friend_number: u32,
    pub path: std::path::PathBuf,
}

// Commands processed on the toxcore thread.
#[derive(Debug)]
pub enum ToxCommand {
    // Send a text message to a friend.
    SendMessage {
        friend_number: u32,
        text: String,
        action: bool,
    },
    // Send a friend request with an address (76 hex chars) and greeting.
    AddFriend { address: String, message: String },
    // Accept an incoming friend request by public key (64 hex chars).
    AcceptFriendRequest { public_key: String },
    // Decline an incoming friend request (no protocol signal; just drop it).
    DeclineFriendRequest { public_key: String },
    // Remove a friend.
    DeleteFriend { friend_number: u32 },
    // Start sending a file.
    SendFile(FileSendRequest),
    // Accept an incoming file transfer and write it under `dest_dir`.
    AcceptFile {
        friend_number: u32,
        file_number: u32,
        dest_dir: std::path::PathBuf,
    },
    // Reject / cancel a file transfer.
    CancelFile {
        friend_number: u32,
        file_number: u32,
    },
    // Update local nickname.
    SetName(String),
    // Update local status message.
    SetStatusMessage(String),
    // Re-bootstrap to the configured nodes (after a disconnect).
    Reconnect,
    // Ask the core to re-emit [`crate::events::ToxEvent::FriendList`].
    RefreshFriends,
    // Ask the core to emit [`crate::events::ToxEvent::LocalInfo`].
    QueryLocalInfo,
    // Bootstrap to an extra node (host, port, hex DHT key) — used by tests.
    BootstrapOnce {
        host: String,
        port: u16,
        dht_key_hex: String,
    },
    // Shutdown cleanly (save identity).
    Shutdown,
}
