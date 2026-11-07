// Events emitted by the tox core toward the UI.

// Connection state of self or a friend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    None,
    Tcp,
    Udp,
}

impl ConnectionStatus {
    pub fn from_raw(v: tox_sys::Tox_Connection) -> Self {
        match v {
            tox_sys::Tox_Connection::None => Self::None,
            tox_sys::Tox_Connection::Tcp => Self::Tcp,
            tox_sys::Tox_Connection::Udp => Self::Udp,
        }
    }

    pub fn is_online(self) -> bool {
        matches!(self, Self::Tcp | Self::Udp)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "offline",
            Self::Tcp => "online (tcp)",
            Self::Udp => "online (udp)",
        }
    }
}

// User status (none / away / busy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    None,
    Away,
    Busy,
}

impl UserStatus {
    pub fn from_raw(v: tox_sys::Tox_User_Status) -> Self {
        match v {
            tox_sys::Tox_User_Status::None => Self::None,
            tox_sys::Tox_User_Status::Away => Self::Away,
            tox_sys::Tox_User_Status::Busy => Self::Busy,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "online",
            Self::Away => "away",
            Self::Busy => "busy",
        }
    }
}

// Friend-scoped events.
#[derive(Debug, Clone)]
pub enum FriendEvent {
    Connected(ConnectionStatus),
    Disconnected,
    NameChanged(String),
    StatusMessageChanged(String),
    StatusChanged(UserStatus),
    Typing(bool),
    ReadReceipt(u32),
}

// Snapshot of a friend loaded from savedata / queried on demand.
#[derive(Debug, Clone)]
pub struct FriendInfo {
    pub number: u32,
    pub public_key: String,
    pub name: String,
    pub status_message: String,
    pub connection: ConnectionStatus,
    pub status: UserStatus,
}

// Everything the core pushes to the UI.
#[derive(Debug, Clone)]
pub enum ToxEvent {
    // Full friend list (emitted at startup and on [`crate::ToxCommand::RefreshFriends`]).
    FriendList(Vec<FriendInfo>),
    // Reply to [`crate::ToxCommand::QueryLocalInfo`] — used by tests and diagnostics.
    LocalInfo {
        address: String,
        public_key: String,
        dht_id: String,
        udp_port: u16,
    },
    // Self DHT connection changed.
    SelfConnection(ConnectionStatus),
    // A new friend request arrived.
    FriendRequest {
        public_key: String,
        message: String,
    },
    // Incoming chat message.
    FriendMessage {
        friend_number: u32,
        text: String,
        action: bool,
        message_id: u32,
    },
    // Outgoing message was accepted into the send queue (for echo/ack).
    MessageQueued {
        friend_number: u32,
        text: String,
        action: bool,
        message_id: u32,
    },
    // Friend-scoped state update.
    Friend {
        friend_number: u32,
        event: FriendEvent,
    },
    // A file transfer offer arrived from a friend.
    FileRecv {
        friend_number: u32,
        file_number: u32,
        filename: String,
        file_size: u64,
    },
    // Progress/completion info for an outgoing or incoming transfer.
    FileControl {
        friend_number: u32,
        file_number: u32,
        control: tox_sys::Tox_File_Control,
    },
    // Human-readable error / notice from the core.
    Notice(String),
    // Core shut down after [`crate::ToxCommand::Shutdown`].
    Shutdown,
}
