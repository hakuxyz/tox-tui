// app state: friends, chat, requests, files

use crate::clipboard;
use crate::theme::{Theme, ThemeRegistry};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tox_client::{
    ConnectionStatus, FriendInfo, HistoryEntry, HistoryStore, ToxCommand, ToxConfig, ToxEvent,
    ToxHandle, UserStatus,
};

// which pane has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Chat,
    Input,
}

// modal dialogs over the main view
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    None,
    // add friend: id + greeting
    AddFriend,
    // incoming friend request
    FriendRequest,
    // send file path entry
    SendFile,
    // incoming file accept/reject
    IncomingFile,
    // help / keys
    Help,
    // set nickname
    SetName,
    // set status message
    SetStatus,
    // full tox id + copy
    ShowId,
    // theme picker list
    ThemePicker,
}

// a friend known to the ui
#[derive(Debug, Clone)]
pub struct Friend {
    pub number: u32,
    pub public_key: String,
    pub name: String,
    pub status_message: String,
    pub connection: ConnectionStatus,
    pub status: UserStatus,
    pub typing: bool,
    // unread while another chat was open
    pub unread: usize,
}

impl Friend {
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.public_key[..self.public_key.len().min(8)]
        } else {
            &self.name
        }
    }
}

// pending friend request
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub public_key: String,
    pub message: String,
}

// file transfer for the sidebar
#[derive(Debug, Clone)]
pub struct TransferView {
    pub friend_number: u32,
    pub file_number: u32,
    pub filename: String,
    pub size: u64,
    // best-effort done flag
    #[allow(dead_code)]
    pub done: bool,
    pub incoming: bool,
}

// one chat line to render
#[derive(Debug, Clone)]
pub struct ChatLine {
    pub time: String,
    pub text: String,
    pub outgoing: bool,
    #[allow(dead_code)]
    pub action: bool,
}

pub struct App {
    pub should_quit: bool,
    pub focus: Focus,
    pub dialog: Dialog,

    pub identity_address: String,
    pub self_connection: ConnectionStatus,
    pub self_name: String,

    pub friends: Vec<Friend>,
    pub selected_friend: usize,
    pub requests: Vec<PendingRequest>,
    pub selected_request: usize,

    pub input: String,
    pub dialog_input: String,
    // second dialog field (e.g. greeting)
    pub dialog_input2: String,
    pub dialog_step: usize,

    pub chat: Vec<ChatLine>,
    pub log: Vec<String>,
    pub scroll_offset: usize,

    pub transfers: Vec<TransferView>,
    pub selected_transfer: usize,

    pub history: HistoryStore,
    pub tox: ToxHandle,

    pub theme: Theme,
    pub themes: ThemeRegistry,
    // last clipboard status text
    pub clipboard_status: String,
    // selected row in the theme picker
    pub selected_theme: usize,
}

const MAX_LOG: usize = 500;
const MAX_CHAT_LOAD: usize = 200;

impl App {
    pub async fn new(config: ToxConfig) -> Result<Self> {
        let data_dir = config.data_dir.clone();
        let tox = tox_client::spawn(config).await?;
        let identity_address = tox.identity().address.clone();
        let history = HistoryStore::new(&data_dir)?;
        let themes = ThemeRegistry::load(&data_dir)?;
        let theme = themes
            .get(&themes.active)
            .unwrap_or_default();

        let mut app = Self {
            should_quit: false,
            focus: Focus::Sidebar,
            dialog: Dialog::None,
            identity_address,
            self_connection: ConnectionStatus::None,
            self_name: String::new(),
            friends: Vec::new(),
            selected_friend: 0,
            requests: Vec::new(),
            selected_request: 0,
            input: String::new(),
            dialog_input: String::new(),
            dialog_input2: String::new(),
            dialog_step: 0,
            chat: Vec::new(),
            log: Vec::new(),
            scroll_offset: 0,
            transfers: Vec::new(),
            selected_transfer: 0,
            history,
            tox,
            theme,
            themes,
            clipboard_status: String::new(),
            selected_theme: 0,
        };
        app.log.push(format!("Tox ID: {}", app.identity_address));
        app.log.push(format!("theme: {}", app.theme.name));
        app.log.push("bootstrapping to DHT…".into());
        Ok(app)
    }

    // pull pending core events each tick
    pub fn pump_events(&mut self) {
        while let Some(ev) = self.tox.try_recv_event() {
            self.apply_event(ev);
        }
    }

    fn apply_event(&mut self, ev: ToxEvent) {
        match ev {
            ToxEvent::FriendList(list) => {
                self.merge_friend_list(list);
            }
            ToxEvent::SelfConnection(c) => {
                let prev = self.self_connection;
                self.self_connection = c;
                if prev != c {
                    self.log.push(format!("network: {}", c.label()));
                }
            }
            ToxEvent::FriendRequest {
                public_key,
                message,
            } => {
                self.log
                    .push(format!("friend request from {}", short(&public_key)));
                if !self
                    .requests
                    .iter()
                    .any(|r| r.public_key == public_key)
                {
                    self.requests.push(PendingRequest {
                        public_key,
                        message,
                    });
                    // Surface the request immediately.
                    self.selected_request = self.requests.len() - 1;
                    self.selected_friend = 0;
                    self.dialog = Dialog::FriendRequest;
                }
            }
            ToxEvent::FriendMessage {
                friend_number,
                text,
                action,
                ..
            } => {
                if let Some(f) = self.friends.iter().find(|f| f.number == friend_number) {
                    let key = f.public_key.clone();
                    let name = f.display_name().to_string();
                    let line = ChatLine {
                        time: now_hm(),
                        text: if action {
                            format!("* {name} {text}")
                        } else {
                            text.clone()
                        },
                        outgoing: false,
                        action,
                    };
                    let _ = self.history.append(
                        &key,
                        HistoryEntry::now(false, action, text, None),
                    );
                    if self.current_friend_number() == Some(friend_number) {
                        self.chat.push(line);
                    } else if let Some(f) = self.friends.iter_mut().find(|f| f.number == friend_number)
                    {
                        f.unread += 1;
                    }
                    self.log.push(format!("message from {name}"));
                }
            }
            ToxEvent::MessageQueued {
                friend_number,
                text,
                action,
                message_id,
            } => {
                if let Some(f) = self.friends.iter().find(|f| f.number == friend_number) {
                    let key = f.public_key.clone();
                    let line = ChatLine {
                        time: now_hm(),
                        text: if action {
                            format!("* you {text}")
                        } else {
                            text.clone()
                        },
                        outgoing: true,
                        action,
                    };
                    let _ = self.history.append(
                        &key,
                        HistoryEntry::now(true, action, text, Some(message_id)),
                    );
                    if self.current_friend_number() == Some(friend_number) {
                        self.chat.push(line);
                    }
                }
            }
            ToxEvent::Friend { friend_number, event } => {
                if !self.friends.iter().any(|f| f.number == friend_number) {
                    // Unknown number (e.g. loaded profile) — create a stub and
                    // ask the core for a full refresh to fill in the key.
                    self.ensure_friend(friend_number);
                    self.tox.send(ToxCommand::RefreshFriends).ok();
                }
                if let Some(f) = self.friends.iter_mut().find(|f| f.number == friend_number) {
                    use tox_client::FriendEvent;
                    match event {
                        FriendEvent::Connected(c) => {
                            f.connection = c;
                            self.log
                                .push(format!("{} is {}", f.display_name(), c.label()));
                        }
                        FriendEvent::Disconnected => {
                            f.connection = ConnectionStatus::None;
                            f.typing = false;
                            self.log
                                .push(format!("{} went offline", f.display_name()));
                        }
                        FriendEvent::NameChanged(n) => {
                            let old = f.display_name().to_string();
                            f.name = n.clone();
                            self.log.push(format!("rename {old} → {n}"));
                        }
                        FriendEvent::StatusMessageChanged(sm) => f.status_message = sm,
                        FriendEvent::StatusChanged(s) => f.status = s,
                        FriendEvent::Typing(t) => f.typing = t,
                        FriendEvent::ReadReceipt(_) => {}
                    }
                } else {
                    // Should not happen after ensure_friend; request a refresh anyway.
                    self.tox.send(ToxCommand::RefreshFriends).ok();
                }
            }
            ToxEvent::FileRecv {
                friend_number,
                file_number,
                filename,
                file_size,
            } => {
                self.transfers.push(TransferView {
                    friend_number,
                    file_number,
                    filename: filename.clone(),
                    size: file_size,
                    done: false,
                    incoming: true,
                });
                self.dialog = Dialog::IncomingFile;
                self.selected_transfer = self.transfers.len() - 1;
                self.dialog_input.clear();
                self.log.push(format!("incoming file: {filename}"));
            }
            ToxEvent::FileControl {
                friend_number,
                file_number,
                control,
            } => {
                use tox_sys::Tox_File_Control;
                if matches!(control, Tox_File_Control::Cancel) {
                    self.transfers
                        .retain(|t| !(t.friend_number == friend_number && t.file_number == file_number));
                    self.log.push("file transfer cancelled".into());
                } else if matches!(control, Tox_File_Control::Resume) {
                    if let Some(t) = self
                        .transfers
                        .iter_mut()
                        .find(|t| t.friend_number == friend_number && t.file_number == file_number)
                    {
                        self.log.push(format!("transfer resumed: {}", t.filename));
                    }
                }
            }
            ToxEvent::Notice(msg) => {
                self.log.push(msg);
            }
            ToxEvent::LocalInfo { .. } => {
                // Diagnostic reply; the TUI does not display it by default.
            }
            ToxEvent::Shutdown => {
                self.log.push("core shutdown".into());
                self.should_quit = true;
            }
        }
        while self.log.len() > MAX_LOG {
            self.log.remove(0);
        }
    }

    pub fn current_friend(&self) -> Option<&Friend> {
        self.friends.get(self.selected_friend)
    }

    pub fn current_friend_number(&self) -> Option<u32> {
        self.current_friend().map(|f| f.number)
    }

    pub fn current_friend_key(&self) -> Option<String> {
        self.current_friend().map(|f| f.public_key.clone())
    }

    // reload chat history for the selected friend
    pub fn load_chat_for_selected(&mut self) {
        self.chat.clear();
        self.scroll_offset = 0;
        let Some(key) = self.current_friend_key() else {
            return;
        };
        if self.history.load(&key).is_ok() {
            let entries = self.history.tail(&key, MAX_CHAT_LOAD);
            self.chat = entries
                .into_iter()
                .map(|e| ChatLine {
                    time: e.time_label(),
                    text: if e.action {
                        format!("* {}", e.text)
                    } else {
                        e.text
                    },
                    outgoing: e.outgoing,
                    action: e.action,
                })
                .collect();
        }
    }

    // merge a full friend list from the core
    fn merge_friend_list(&mut self, list: Vec<FriendInfo>) {
        let selected_key = self.current_friend_key();
        let selected_num = self.current_friend_number();
        let old_unread: HashMap<String, usize> = self
            .friends
            .iter()
            .map(|f| (f.public_key.clone(), f.unread))
            .collect();
        self.friends = list
            .into_iter()
            .map(|info| Friend {
                number: info.number,
                public_key: info.public_key,
                name: info.name,
                status_message: info.status_message,
                connection: info.connection,
                status: info.status,
                typing: false,
                unread: 0,
            })
            .collect();
        for f in &mut self.friends {
            if let Some(&u) = old_unread.get(&f.public_key) {
                f.unread = u;
            }
        }
        // Preserve selection.
        if let Some(num) = selected_num {
            if let Some(idx) = self.friends.iter().position(|f| f.number == num) {
                self.selected_friend = idx;
            }
        } else if let Some(key) = selected_key {
            if let Some(idx) = self.friends.iter().position(|f| f.public_key == key) {
                self.selected_friend = idx;
            }
        }
        if self.selected_friend >= self.friends.len() {
            self.selected_friend = 0;
        }
        if let Some(key) = self.current_friend_key() {
            // Reload chat against possibly new key mapping.
            let _ = key;
            self.load_chat_for_selected();
        }
        self.log.push(format!("{} friends loaded", self.friends.len()));
    }

    // stub friend by number until we know the key
    pub fn ensure_friend(&mut self, number: u32) -> &mut Friend {
        if let Some(idx) = self.friends.iter().position(|f| f.number == number) {
            &mut self.friends[idx]
        } else {
            self.friends.push(Friend {
                number,
                public_key: format!("unknown-{number}"),
                name: String::new(),
                status_message: String::new(),
                connection: ConnectionStatus::None,
                status: UserStatus::None,
                typing: false,
                unread: 0,
            });
            self.friends.last_mut().unwrap()
        }
    }

    pub fn send_chat(&mut self, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        let Some(fn_num) = self.current_friend_number() else {
            self.log.push("select a friend first".into());
            return Ok(());
        };

        // Slash commands
        if let Some(rest) = text.strip_prefix('/') {
            return self.handle_slash(rest, fn_num);
        }

        self.tox.send(ToxCommand::SendMessage {
            friend_number: fn_num,
            text: text.to_string(),
            action: false,
        })?;
        Ok(())
    }

    fn handle_slash(&mut self, rest: &str, fn_num: u32) -> Result<()> {
        let mut parts = rest.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("").trim();
        match cmd {
            "me" if !arg.is_empty() => {
                self.tox.send(ToxCommand::SendMessage {
                    friend_number: fn_num,
                    text: arg.to_string(),
                    action: true,
                })?;
            }
            "add" => {
                self.dialog = Dialog::AddFriend;
                self.dialog_input = arg.to_string();
                self.dialog_input2.clear();
                self.dialog_step = if arg.is_empty() { 0 } else { 1 };
                self.focus = Focus::Input;
            }
            "file" if !arg.is_empty() => {
                let path = PathBuf::from(arg);
                self.tox.send(ToxCommand::SendFile(
                    tox_client::FileSendRequest {
                        friend_number: fn_num,
                        path,
                    },
                ))?;
            }
            "name" if !arg.is_empty() => {
                self.tox.send(ToxCommand::SetName(arg.to_string()))?;
                self.self_name = arg.to_string();
                self.log.push(format!("name set to {arg}"));
            }
            "status" if !arg.is_empty() => {
                self.tox.send(ToxCommand::SetStatusMessage(arg.to_string()))?;
                self.log.push(format!("status set to {arg}"));
            }
            "id" | "myid" => {
                self.open_id_dialog();
            }
            "copy" => {
                self.copy_id();
            }
            "theme" => {
                if arg.is_empty() {
                    self.dialog = Dialog::ThemePicker;
                    // Pre-select the active theme.
                    let names = self.theme_names();
                    self.selected_theme = names
                        .iter()
                        .position(|n| n == &self.theme.name)
                        .unwrap_or(0);
                } else {
                    self.apply_theme(arg);
                }
            }
            "themes" => {
                let names = self.theme_names().join(", ");
                self.log.push(format!("themes: {names}"));
            }
            "clear" => {
                self.chat.clear();
                self.log.push("chat cleared (history on disk kept)".into());
            }
            "help" => {
                self.dialog = Dialog::Help;
            }
            "quit" | "q" => {
                self.should_quit = true;
            }
            _ => {
                self.log
                    .push(format!("unknown command /{cmd} — try /help"));
            }
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.tox.send(ToxCommand::Shutdown);
        // Give the core a moment to persist.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        Ok(())
    }

    // accept the selected friend request
    pub fn accept_selected_request(&mut self) -> Result<()> {
        if self.requests.is_empty() {
            return Ok(());
        }
        let idx = self.selected_request.min(self.requests.len() - 1);
        let pk = self.requests[idx].public_key.clone();
        self.tox
            .send(ToxCommand::AcceptFriendRequest { public_key: pk })?;
        self.requests.remove(idx);
        if self.selected_request >= self.requests.len() && self.selected_request > 0 {
            self.selected_request -= 1;
        }
        self.log.push("friend request accepted".into());
        self.tox.send(ToxCommand::RefreshFriends)?;
        Ok(())
    }

    pub fn decline_selected_request(&mut self) -> Result<()> {
        if self.requests.is_empty() {
            return Ok(());
        }
        let idx = self.selected_request.min(self.requests.len() - 1);
        let pk = self.requests[idx].public_key.clone();
        self.tox
            .send(ToxCommand::DeclineFriendRequest { public_key: pk })?;
        self.requests.remove(idx);
        if self.selected_request >= self.requests.len() && self.selected_request > 0 {
            self.selected_request -= 1;
        }
        Ok(())
    }

    pub fn select_friend(&mut self, idx: usize) {
        if idx < self.friends.len() {
            self.selected_friend = idx;
            if let Some(f) = self.friends.get_mut(idx) {
                f.unread = 0;
            }
            self.load_chat_for_selected();
        }
    }

    // jump via hotkey 1–9
    pub fn jump_to_friend(&mut self, n: usize) {
        if n >= 1 && n <= self.friends.len() {
            self.select_friend(n - 1);
            // Jumping implies intent to chat.
            self.focus = Focus::Input;
        }
    }

    pub fn next_friend(&mut self) {
        if self.friends.is_empty() {
            return;
        }
        let idx = (self.selected_friend + 1) % self.friends.len();
        self.select_friend(idx);
    }

    pub fn prev_friend(&mut self) {
        if self.friends.is_empty() {
            return;
        }
        let idx = (self.selected_friend + self.friends.len() - 1) % self.friends.len();
        self.select_friend(idx);
    }

    // open id dialog and copy to clipboard
    pub fn open_id_dialog(&mut self) {
        self.dialog = Dialog::ShowId;
        let id = self.identity_address.clone();
        self.clipboard_status = clipboard::copy(&id);
        self.log.push(format!("ID {}", self.clipboard_status));
    }

    pub fn copy_id(&mut self) {
        let id = self.identity_address.clone();
        self.clipboard_status = clipboard::copy(&id);
        self.log.push(format!("ID {}", self.clipboard_status));
    }

    // apply a builtin or custom theme
    pub fn apply_theme(&mut self, name: &str) -> bool {
        if let Some(t) = self.themes.set_active(name) {
            self.log.push(format!("theme: {name}"));
            self.theme = t;
            true
        } else {
            self.log
                .push(format!("unknown theme {name} — /themes to list"));
            false
        }
    }

    pub fn theme_names(&self) -> Vec<String> {
        self.themes.all_names()
    }
}

fn short(key: &str) -> String {
    if key.len() > 8 {
        format!("{}…", &key[..8])
    } else {
        key.to_string()
    }
}

fn now_hm() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}
