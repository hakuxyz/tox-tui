// toxcore event loop: owns the C Tox*, pumps tox_iterate, bridges callbacks

use crate::commands::{FileSendRequest, ToxCommand};
use crate::events::{ConnectionStatus, FriendEvent, FriendInfo, ToxEvent, UserStatus};
use crate::identity::{Identity, IdentityStore};
use crate::private_file::{create_dir_private, write_private};
use crate::{ToxConfig, ToxHandle};
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Shared state between callbacks and the command handler
// ---------------------------------------------------------------------------

struct OutgoingFile {
    file: File,
    path: PathBuf,
    #[allow(dead_code)]
    size: u64,
}

struct IncomingFile {
    file: File,
    dest: PathBuf,
    #[allow(dead_code)]
    size: u64,
    written: u64,
}

#[derive(Default)]
struct CoreState {
    outgoing: HashMap<(u32, u32), OutgoingFile>,
    incoming: HashMap<(u32, u32), IncomingFile>,
    pending_incoming: HashMap<(u32, u32), (String, u64)>,
}

// Passed as `user_data` to `tox_iterate`; callbacks downcast it back.
struct CallbackCtx {
    events: mpsc::UnboundedSender<ToxEvent>,
    state: Arc<Mutex<CoreState>>,
}

unsafe impl Send for CallbackCtx {}
unsafe impl Sync for CallbackCtx {}

fn bytes_to_string(buf: *const u8, len: usize) -> String {
    if buf.is_null() || len == 0 {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(buf, len) };
    String::from_utf8_lossy(slice).into_owned()
}

fn hex_key(key: *const u8, len: usize) -> String {
    if key.is_null() {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(key, len) };
    hex::encode(slice)
}

// ---------------------------------------------------------------------------
// C callbacks (user_data → &CallbackCtx)
// ---------------------------------------------------------------------------

extern "C" fn on_self_connection(
    _tox: *mut tox_sys::Tox,
    status: tox_sys::Tox_Connection,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx
        .events
        .send(ToxEvent::SelfConnection(ConnectionStatus::from_raw(status)));
}

extern "C" fn on_friend_request(
    _tox: *mut tox_sys::Tox,
    public_key: *const tox_sys::Tox_Public_Key,
    message: *const u8,
    length: usize,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx.events.send(ToxEvent::FriendRequest {
        public_key: hex_key(public_key as *const u8, tox_sys::TOX_PUBLIC_KEY_SIZE),
        message: bytes_to_string(message, length),
    });
}

extern "C" fn on_friend_message(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    type_: tox_sys::Tox_Message_Type,
    message: *const u8,
    length: usize,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx.events.send(ToxEvent::FriendMessage {
        friend_number,
        text: bytes_to_string(message, length),
        action: matches!(type_, tox_sys::Tox_Message_Type::Action),
        message_id: 0,
    });
}

extern "C" fn on_friend_name(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    name: *const u8,
    length: usize,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx.events.send(ToxEvent::Friend {
        friend_number,
        event: FriendEvent::NameChanged(bytes_to_string(name, length)),
    });
}

extern "C" fn on_friend_status_message(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    message: *const u8,
    length: usize,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx.events.send(ToxEvent::Friend {
        friend_number,
        event: FriendEvent::StatusMessageChanged(bytes_to_string(message, length)),
    });
}

extern "C" fn on_friend_status(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    status: tox_sys::Tox_User_Status,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx.events.send(ToxEvent::Friend {
        friend_number,
        event: FriendEvent::StatusChanged(UserStatus::from_raw(status)),
    });
}

extern "C" fn on_friend_connection(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    status: tox_sys::Tox_Connection,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let conn = ConnectionStatus::from_raw(status);
    let event = if conn.is_online() {
        FriendEvent::Connected(conn)
    } else {
        FriendEvent::Disconnected
    };
    let _ = ctx.events.send(ToxEvent::Friend {
        friend_number,
        event,
    });
}

extern "C" fn on_friend_typing(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    typing: bool,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx.events.send(ToxEvent::Friend {
        friend_number,
        event: FriendEvent::Typing(typing),
    });
}

extern "C" fn on_read_receipt(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    message_id: u32,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let _ = ctx.events.send(ToxEvent::Friend {
        friend_number,
        event: FriendEvent::ReadReceipt(message_id),
    });
}

extern "C" fn on_file_recv(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    file_number: u32,
    _kind: u32,
    file_size: u64,
    filename: *const u8,
    filename_length: usize,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let name = bytes_to_string(filename, filename_length);
    {
        let mut st = ctx.state.lock().unwrap();
        st.pending_incoming
            .insert((friend_number, file_number), (name.clone(), file_size));
    }
    let _ = ctx.events.send(ToxEvent::FileRecv {
        friend_number,
        file_number,
        filename: name,
        file_size,
    });
}

extern "C" fn on_file_recv_chunk(
    tox: *mut tox_sys::Tox,
    friend_number: u32,
    file_number: u32,
    position: u64,
    data: *const u8,
    length: usize,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    let key = (friend_number, file_number);
    let mut st = ctx.state.lock().unwrap();

    // outcome: keep, finish(ok), or fail(reason)
    enum Outcome {
        Keep,
        Done(u64),
        Fail(String),
    }

    let outcome = {
        let Some(inc) = st.incoming.get_mut(&key) else {
            drop(st);
            log::debug!("chunk for unknown transfer {friend_number}/{file_number}");
            return;
        };

        if length == 0 || data.is_null() {
            match inc.file.flush() {
                Ok(()) => Outcome::Done(inc.written),
                Err(e) => Outcome::Fail(format!("flush: {e}")),
            }
        } else {
            let end = position.saturating_add(length as u64);
            if end > inc.size {
                Outcome::Fail(format!("size exceeded ({end} > {})", inc.size))
            } else {
                let slice = unsafe { std::slice::from_raw_parts(data, length) };
                match inc
                    .file
                    .seek(SeekFrom::Start(position))
                    .and_then(|_| inc.file.write_all(slice))
                {
                    Ok(()) => {
                        inc.written = end;
                        Outcome::Keep
                    }
                    Err(e) => Outcome::Fail(format!("write: {e}")),
                }
            }
        }
    };

    match outcome {
        Outcome::Keep => {}
        Outcome::Done(written) => {
            if let Some(inc) = st.incoming.remove(&key) {
                let _ = ctx.events.send(ToxEvent::Notice(format!(
                    "received {} ({written} bytes)",
                    inc.dest.display()
                )));
            }
        }
        Outcome::Fail(reason) => {
            log::error!("receive {reason} for {friend_number}/{file_number}");
            let dest = st.incoming.remove(&key).map(|inc| inc.dest);
            drop(st);
            // tell toxcore to stop sending, not just drop local state
            let mut err = tox_sys::Tox_Err_File_Control::Ok;
            unsafe {
                tox_sys::tox_file_control(
                    tox,
                    friend_number,
                    file_number,
                    tox_sys::Tox_File_Control::Cancel,
                    &mut err,
                );
            }
            if let Some(dest) = dest {
                let _ = ctx.events.send(ToxEvent::Notice(format!(
                    "receive failed ({reason}): {}",
                    dest.display()
                )));
            } else {
                let _ = ctx
                    .events
                    .send(ToxEvent::Notice(format!("receive failed ({reason})")));
            }
        }
    }
}

extern "C" fn on_file_chunk_request(
    tox: *mut tox_sys::Tox,
    friend_number: u32,
    file_number: u32,
    position: u64,
    length: usize,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };

    // length == 0 is the protocol's "send final empty chunk" signal
    let is_final = length == 0;
    let chunk = {
        let mut st = ctx.state.lock().unwrap();
        match st.outgoing.get_mut(&(friend_number, file_number)) {
            None => None,
            Some(_) if is_final => Some(Vec::new()),
            Some(out) => {
                let mut buf = vec![0u8; length];
                if out.file.seek(SeekFrom::Start(position)).is_err() {
                    None
                } else {
                    match out.file.read(&mut buf) {
                        Ok(n) => {
                            buf.truncate(n);
                            Some(buf)
                        }
                        Err(_) => None,
                    }
                }
            }
        }
    };

    match chunk {
        Some(buf) => {
            let mut err = tox_sys::Tox_Err_File_Send_Chunk::Ok;
            let ok = unsafe {
                tox_sys::tox_file_send_chunk(
                    tox,
                    friend_number,
                    file_number,
                    position,
                    if buf.is_empty() {
                        std::ptr::null()
                    } else {
                        buf.as_ptr()
                    },
                    buf.len(),
                    &mut err,
                )
            };
            if ok {
                if is_final {
                    // normal completion: drop state, do not Cancel
                    let mut st = ctx.state.lock().unwrap();
                    if let Some(out) = st.outgoing.remove(&(friend_number, file_number)) {
                        let _ = ctx.events.send(ToxEvent::Notice(format!(
                            "finished sending {}",
                            out.path.display()
                        )));
                    }
                }
            } else {
                log::warn!("tox_file_send_chunk failed: {err:?}");
                let mut st = ctx.state.lock().unwrap();
                if let Some(out) = st.outgoing.remove(&(friend_number, file_number)) {
                    let _ = ctx.events.send(ToxEvent::Notice(format!(
                        "send failed for {}",
                        out.path.display()
                    )));
                }
            }
        }
        None => {
            // read/seek error (or transfer already gone): cancel and drop state
            let mut err = tox_sys::Tox_Err_File_Control::Ok;
            unsafe {
                tox_sys::tox_file_control(
                    tox,
                    friend_number,
                    file_number,
                    tox_sys::Tox_File_Control::Cancel,
                    &mut err,
                );
            }
            let mut st = ctx.state.lock().unwrap();
            if let Some(out) = st.outgoing.remove(&(friend_number, file_number)) {
                log::error!("outgoing read failed for {}", out.path.display());
                let _ = ctx.events.send(ToxEvent::Notice(format!(
                    "send failed for {}",
                    out.path.display()
                )));
            }
        }
    }
}

extern "C" fn on_file_recv_control(
    _tox: *mut tox_sys::Tox,
    friend_number: u32,
    file_number: u32,
    control: tox_sys::Tox_File_Control,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackCtx) };
    if matches!(control, tox_sys::Tox_File_Control::Cancel) {
        let mut st = ctx.state.lock().unwrap();
        st.outgoing.remove(&(friend_number, file_number));
        st.incoming.remove(&(friend_number, file_number));
        st.pending_incoming.remove(&(friend_number, file_number));
    }
    let _ = ctx.events.send(ToxEvent::FileControl {
        friend_number,
        file_number,
        control,
    });
}

fn register_callbacks(tox: *mut tox_sys::Tox) {
    unsafe {
        tox_sys::tox_callback_self_connection_status(tox, Some(on_self_connection));
        tox_sys::tox_callback_friend_request(tox, Some(on_friend_request));
        tox_sys::tox_callback_friend_message(tox, Some(on_friend_message));
        tox_sys::tox_callback_friend_name(tox, Some(on_friend_name));
        tox_sys::tox_callback_friend_status_message(tox, Some(on_friend_status_message));
        tox_sys::tox_callback_friend_status(tox, Some(on_friend_status));
        tox_sys::tox_callback_friend_connection_status(tox, Some(on_friend_connection));
        tox_sys::tox_callback_friend_typing(tox, Some(on_friend_typing));
        tox_sys::tox_callback_friend_read_receipt(tox, Some(on_read_receipt));
        tox_sys::tox_callback_file_recv(tox, Some(on_file_recv));
        tox_sys::tox_callback_file_recv_chunk(tox, Some(on_file_recv_chunk));
        tox_sys::tox_callback_file_chunk_request(tox, Some(on_file_chunk_request));
        tox_sys::tox_callback_file_recv_control(tox, Some(on_file_recv_control));
    }
}

// ---------------------------------------------------------------------------
// Tox instance
// ---------------------------------------------------------------------------

struct RawTox {
    ptr: *mut tox_sys::Tox,
}

impl Drop for RawTox {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { tox_sys::tox_kill(self.ptr) };
        }
    }
}

unsafe impl Send for RawTox {}

fn bootstrap_all(tox: *mut tox_sys::Tox, nodes: &[(String, u16, String)]) {
    for (host, port, key_hex) in nodes {
        let Ok(key_bytes) = hex::decode(key_hex) else {
            continue;
        };
        if key_bytes.len() != tox_sys::TOX_DHT_ID_SIZE {
            continue;
        }
        let c_host = match CString::new(host.as_str()) {
            Ok(h) => h,
            Err(_) => continue,
        };
        let mut key = [0u8; tox_sys::TOX_DHT_ID_SIZE];
        key.copy_from_slice(&key_bytes);
        let mut err = tox_sys::Tox_Err_Bootstrap::Ok;
        let ok = unsafe { tox_sys::tox_bootstrap(tox, c_host.as_ptr(), *port, &key, &mut err) };
        if !ok {
            log::debug!("bootstrap {host}:{port} failed: {err:?}");
        } else {
            log::debug!("bootstrapped {host}:{port}");
        }
    }
}

// ---------------------------------------------------------------------------
// Command handling
// ---------------------------------------------------------------------------

fn collect_friend_list(tox: *mut tox_sys::Tox) -> Vec<FriendInfo> {
    unsafe {
        let n = tox_sys::tox_self_get_friend_list_size(tox);
        let mut numbers = vec![0u32; n];
        if n > 0 {
            tox_sys::tox_self_get_friend_list(tox, numbers.as_mut_ptr());
        }
        let mut out = Vec::with_capacity(n);
        for &num in &numbers {
            let mut pk = [0u8; tox_sys::TOX_PUBLIC_KEY_SIZE];
            let mut err_pk = tox_sys::Tox_Err_Friend_Get_Public_Key::Ok;
            tox_sys::tox_friend_get_public_key(tox, num, &mut pk, &mut err_pk);
            let public_key = if err_pk == tox_sys::Tox_Err_Friend_Get_Public_Key::Ok {
                hex::encode(pk)
            } else {
                format!("unknown-{num}")
            };

            let mut err_q = tox_sys::Tox_Err_Friend_Query::Ok;
            let name_len = tox_sys::tox_friend_get_name_size(tox, num, &mut err_q);
            let mut name = String::new();
            if err_q == tox_sys::Tox_Err_Friend_Query::Ok && name_len > 0 {
                let mut buf = vec![0u8; name_len];
                let mut err_n = tox_sys::Tox_Err_Friend_Query::Ok;
                if tox_sys::tox_friend_get_name(tox, num, buf.as_mut_ptr(), &mut err_n)
                    && err_n == tox_sys::Tox_Err_Friend_Query::Ok
                {
                    name = String::from_utf8_lossy(&buf).into_owned();
                }
            }

            let mut err_s = tox_sys::Tox_Err_Friend_Query::Ok;
            let sm_len = tox_sys::tox_friend_get_status_message_size(tox, num, &mut err_s);
            let mut status_message = String::new();
            if err_s == tox_sys::Tox_Err_Friend_Query::Ok && sm_len > 0 {
                let mut buf = vec![0u8; sm_len];
                let mut err_m = tox_sys::Tox_Err_Friend_Query::Ok;
                if tox_sys::tox_friend_get_status_message(tox, num, buf.as_mut_ptr(), &mut err_m)
                    && err_m == tox_sys::Tox_Err_Friend_Query::Ok
                {
                    status_message = String::from_utf8_lossy(&buf).into_owned();
                }
            }

            let mut err_c = tox_sys::Tox_Err_Friend_Query::Ok;
            let connection = ConnectionStatus::from_raw(tox_sys::tox_friend_get_connection_status(
                tox, num, &mut err_c,
            ));
            let mut err_u = tox_sys::Tox_Err_Friend_Query::Ok;
            let status = UserStatus::from_raw(tox_sys::tox_friend_get_status(tox, num, &mut err_u));

            out.push(FriendInfo {
                number: num,
                public_key,
                name,
                status_message,
                connection,
                status,
            });
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_command(
    tox: *mut tox_sys::Tox,
    cmd: ToxCommand,
    state: &Arc<Mutex<CoreState>>,
    events: &mpsc::UnboundedSender<ToxEvent>,
    save_path: &Path,
    shutdown: &mut bool,
    bootstrap: &[(String, u16, String)],
    last_bootstrap: &mut std::time::Instant,
) {
    match cmd {
        ToxCommand::SendMessage {
            friend_number,
            text,
            action,
        } => {
            let msg_type = if action {
                tox_sys::Tox_Message_Type::Action
            } else {
                tox_sys::Tox_Message_Type::Normal
            };
            let bytes = text.as_bytes();
            let mut err = tox_sys::Tox_Err_Friend_Send_Message::Ok;
            let id = unsafe {
                tox_sys::tox_friend_send_message(
                    tox,
                    friend_number,
                    msg_type,
                    bytes.as_ptr(),
                    bytes.len(),
                    &mut err,
                )
            };
            match err {
                tox_sys::Tox_Err_Friend_Send_Message::Ok => {
                    let _ = events.send(ToxEvent::MessageQueued {
                        friend_number,
                        text,
                        action,
                        message_id: id,
                    });
                }
                other => {
                    let _ = events.send(ToxEvent::Notice(format!("send failed: {other:?}")));
                }
            }
        }
        ToxCommand::AddFriend { address, message } => {
            let addr_hex: String = address.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            let Ok(addr_bytes) = hex::decode(&addr_hex) else {
                let _ = events.send(ToxEvent::Notice(
                    "invalid Tox ID (need 76 hex chars)".into(),
                ));
                return;
            };
            if addr_bytes.len() != tox_sys::TOX_ADDRESS_SIZE {
                let _ = events.send(ToxEvent::Notice(format!(
                    "Tox ID must be {} hex chars, got {}",
                    tox_sys::TOX_ADDRESS_SIZE * 2,
                    addr_bytes.len()
                )));
                return;
            }
            let mut addr = [0u8; tox_sys::TOX_ADDRESS_SIZE];
            addr.copy_from_slice(&addr_bytes);
            let msg_bytes = message.as_bytes();
            let mut err = tox_sys::Tox_Err_Friend_Add::Ok;
            let fn_num = unsafe {
                tox_sys::tox_friend_add(tox, &addr, msg_bytes.as_ptr(), msg_bytes.len(), &mut err)
            };
            match err {
                tox_sys::Tox_Err_Friend_Add::Ok => {
                    let _ = events.send(ToxEvent::Notice(format!(
                        "friend request sent (friend #{fn_num})"
                    )));
                }
                other => {
                    let _ = events.send(ToxEvent::Notice(format!("add friend failed: {other:?}")));
                }
            }
        }
        ToxCommand::AcceptFriendRequest { public_key } => {
            let pk_hex: String = public_key
                .chars()
                .filter(|c| c.is_ascii_hexdigit())
                .collect();
            let Ok(pk_bytes) = hex::decode(&pk_hex) else {
                let _ = events.send(ToxEvent::Notice("invalid public key".into()));
                return;
            };
            if pk_bytes.len() != tox_sys::TOX_PUBLIC_KEY_SIZE {
                let _ = events.send(ToxEvent::Notice("public key must be 32 bytes".into()));
                return;
            }
            let mut pk = [0u8; tox_sys::TOX_PUBLIC_KEY_SIZE];
            pk.copy_from_slice(&pk_bytes);
            let mut err = tox_sys::Tox_Err_Friend_Add::Ok;
            let fn_num = unsafe { tox_sys::tox_friend_add_norequest(tox, &pk, &mut err) };
            match err {
                tox_sys::Tox_Err_Friend_Add::Ok => {
                    let _ = events.send(ToxEvent::Notice(format!(
                        "friend request accepted (friend #{fn_num})"
                    )));
                }
                other => {
                    let _ = events.send(ToxEvent::Notice(format!("accept failed: {other:?}")));
                }
            }
        }
        ToxCommand::DeclineFriendRequest { public_key } => {
            // c-toxcore has no "reject request" API; the UI drops it locally.
            log::info!("dismissing friend request {public_key} (local only)");
            let _ = events.send(ToxEvent::Notice(
                "friend request dismissed (local only)".into(),
            ));
        }
        ToxCommand::DeleteFriend { friend_number } => {
            let mut err = tox_sys::Tox_Err_Friend_Delete::Ok;
            let ok = unsafe { tox_sys::tox_friend_delete(tox, friend_number, &mut err) };
            if ok {
                let _ = events.send(ToxEvent::Notice(format!("removed friend #{friend_number}")));
            } else {
                let _ = events.send(ToxEvent::Notice(format!("remove failed: {err:?}")));
            }
        }
        ToxCommand::SendFile(FileSendRequest {
            friend_number,
            path,
        }) => {
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    let _ = events.send(ToxEvent::Notice(format!(
                        "cannot read {}: {e}",
                        path.display()
                    )));
                    return;
                }
            };
            let size = meta.len();
            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file.bin".into());
            let fname_bytes = filename.as_bytes();
            if fname_bytes.len() > tox_sys::TOX_MAX_FILENAME_LENGTH {
                let _ = events.send(ToxEvent::Notice("filename too long".into()));
                return;
            }
            let mut err = tox_sys::Tox_Err_File_Send::Ok;
            let file_number = unsafe {
                tox_sys::tox_file_send(
                    tox,
                    friend_number,
                    0,
                    size,
                    std::ptr::null(),
                    fname_bytes.as_ptr(),
                    fname_bytes.len(),
                    &mut err,
                )
            };
            match err {
                tox_sys::Tox_Err_File_Send::Ok => match File::open(&path) {
                    Ok(f) => {
                        let mut st = state.lock().unwrap();
                        st.outgoing.insert(
                            (friend_number, file_number),
                            OutgoingFile {
                                file: f,
                                path: path.clone(),
                                size,
                            },
                        );
                        drop(st);
                        let _ = events.send(ToxEvent::Notice(format!(
                            "sending {} ({size} bytes) as #{file_number}",
                            path.display()
                        )));
                    }
                    Err(e) => {
                        let _ = events.send(ToxEvent::Notice(format!("open failed: {e}")));
                    }
                },
                other => {
                    let _ = events.send(ToxEvent::Notice(format!("file send failed: {other:?}")));
                }
            }
        }
        ToxCommand::AcceptFile {
            friend_number,
            file_number,
            dest_dir,
        } => {
            // peek only: keep the offer until dir+file are ready
            let pending = {
                let st = state.lock().unwrap();
                st.pending_incoming
                    .get(&(friend_number, file_number))
                    .cloned()
            };
            let Some((filename, file_size)) = pending else {
                let _ = events.send(ToxEvent::Notice("no such incoming file".into()));
                return;
            };
            if let Err(e) = create_dir_private(&dest_dir) {
                let _ = events.send(ToxEvent::Notice(format!(
                    "create dir failed: {e} (offer kept, retry or cancel)"
                )));
                return;
            }
            let safe_name = sanitize_filename(&filename);
            let (dest, f) = match unique_path_for(&dest_dir, &safe_name) {
                Ok(prepared) => prepared,
                Err(e) => {
                    let _ = events.send(ToxEvent::Notice(format!(
                        "create file failed: {e} (offer kept, retry or cancel)"
                    )));
                    return;
                }
            };
            // destination is ready: consume the offer
            {
                let mut st = state.lock().unwrap();
                st.pending_incoming.remove(&(friend_number, file_number));
            }
            finish_accept(
                tox,
                state,
                events,
                friend_number,
                file_number,
                f,
                dest,
                file_size,
                &filename,
            );
        }
        ToxCommand::CancelFile {
            friend_number,
            file_number,
        } => {
            let mut err = tox_sys::Tox_Err_File_Control::Ok;
            unsafe {
                tox_sys::tox_file_control(
                    tox,
                    friend_number,
                    file_number,
                    tox_sys::Tox_File_Control::Cancel,
                    &mut err,
                );
            }
            let mut st = state.lock().unwrap();
            st.outgoing.remove(&(friend_number, file_number));
            st.incoming.remove(&(friend_number, file_number));
            st.pending_incoming.remove(&(friend_number, file_number));
            drop(st);
            let _ = events.send(ToxEvent::Notice("file transfer cancelled".into()));
        }
        ToxCommand::SetName(name) => {
            let mut err = tox_sys::Tox_Err_Set_Info::Ok;
            let ok =
                unsafe { tox_sys::tox_self_set_name(tox, name.as_ptr(), name.len(), &mut err) };
            if !ok {
                let _ = events.send(ToxEvent::Notice(format!("set name failed: {err:?}")));
            } else {
                save_tox(tox, save_path);
            }
        }
        ToxCommand::SetStatusMessage(msg) => {
            let mut err = tox_sys::Tox_Err_Set_Info::Ok;
            let ok = unsafe {
                tox_sys::tox_self_set_status_message(tox, msg.as_ptr(), msg.len(), &mut err)
            };
            if !ok {
                let _ = events.send(ToxEvent::Notice(format!("set status failed: {err:?}")));
            } else {
                save_tox(tox, save_path);
            }
        }
        ToxCommand::Reconnect => {
            bootstrap_all(tox, bootstrap);
            *last_bootstrap = std::time::Instant::now();
            let _ = events.send(ToxEvent::Notice("reconnecting (re-bootstrapped)".into()));
        }
        ToxCommand::RefreshFriends => {
            let list = collect_friend_list(tox);
            let _ = events.send(ToxEvent::FriendList(list));
        }
        ToxCommand::QueryLocalInfo => unsafe {
            let mut address = [0u8; tox_sys::TOX_ADDRESS_SIZE];
            let mut pk = [0u8; tox_sys::TOX_PUBLIC_KEY_SIZE];
            let mut dht = [0u8; tox_sys::TOX_DHT_ID_SIZE];
            tox_sys::tox_self_get_address(tox, &mut address);
            tox_sys::tox_self_get_public_key(tox, &mut pk);
            tox_sys::tox_self_get_dht_id(tox, &mut dht);
            let mut err = tox_sys::Tox_Err_Get_Port::Ok;
            let port = tox_sys::tox_self_get_udp_port(tox, &mut err);
            let _ = events.send(ToxEvent::LocalInfo {
                address: hex::encode(address),
                public_key: hex::encode(pk),
                dht_id: hex::encode(dht),
                udp_port: port,
            });
        },
        ToxCommand::BootstrapOnce {
            host,
            port,
            dht_key_hex,
        } => {
            let Ok(key_bytes) = hex::decode(&dht_key_hex) else {
                let _ = events.send(ToxEvent::Notice("bad dht key hex".into()));
                return;
            };
            if key_bytes.len() != tox_sys::TOX_DHT_ID_SIZE {
                let _ = events.send(ToxEvent::Notice("dht key must be 32 bytes".into()));
                return;
            }
            let Ok(c_host) = CString::new(host.clone()) else {
                let _ = events.send(ToxEvent::Notice("bad host".into()));
                return;
            };
            let mut key = [0u8; tox_sys::TOX_DHT_ID_SIZE];
            key.copy_from_slice(&key_bytes);
            let mut err = tox_sys::Tox_Err_Bootstrap::Ok;
            let ok = unsafe { tox_sys::tox_bootstrap(tox, c_host.as_ptr(), port, &key, &mut err) };
            if ok {
                let _ = events.send(ToxEvent::Notice(format!("bootstrapped {host}:{port}")));
            } else {
                let _ = events.send(ToxEvent::Notice(format!("bootstrap failed: {err:?}")));
            }
        }
        ToxCommand::Shutdown => {
            save_tox(tox, save_path);
            *shutdown = true;
            let _ = events.send(ToxEvent::Shutdown);
        }
    }
}

// create a new file without ever overwriting an existing path.
// tries `name`, then "name (1)", "name (2)", ...
pub fn unique_path_for(dir: &Path, name: &str) -> std::io::Result<(PathBuf, File)> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let open = |p: &Path| {
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        o.mode(0o600);
        o.open(p)
    };

    let direct = dir.join(name);
    if let Ok(f) = open(&direct) {
        return Ok((direct, f));
    }

    let stem = std::path::Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let ext = std::path::Path::new(name)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();

    for i in 1..10_000 {
        let candidate = dir.join(format!("{stem} ({i}){ext}"));
        if let Ok(f) = open(&candidate) {
            return Ok((candidate, f));
        }
    }
    let fallback = dir.join(format!("{stem}-{}", std::process::id()));
    let f = open(&fallback)?;
    Ok((fallback, f))
}

// strip control chars / path separators; "" "." ".." become "download".
pub fn sanitize_filename(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || c == '/' || c == '\\' || c == '\0' {
                '_'
            } else {
                c
            }
        })
        .collect();
    // collapse any leftover path-ish junk from the original
    while cleaned.contains("..") {
        cleaned = cleaned.replace("..", "_");
    }
    cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    // empty or pure punctuation after cleanup is not a usable name
    if cleaned.is_empty() || cleaned.chars().all(|c| matches!(c, '.' | '_' | ' ')) {
        return "download".into();
    }
    // keep it a reasonable single path segment
    if cleaned.len() > 200 {
        let ext = std::path::Path::new(&cleaned)
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();
        let stem: String = cleaned.chars().take(180).collect();
        cleaned = format!("{stem}{ext}");
    }
    cleaned
}

#[allow(clippy::too_many_arguments)]
fn finish_accept(
    tox: *mut tox_sys::Tox,
    state: &Arc<Mutex<CoreState>>,
    events: &mpsc::UnboundedSender<ToxEvent>,
    friend_number: u32,
    file_number: u32,
    f: File,
    dest: PathBuf,
    file_size: u64,
    filename: &str,
) {
    let mut err = tox_sys::Tox_Err_File_Control::Ok;
    let ok = unsafe {
        tox_sys::tox_file_control(
            tox,
            friend_number,
            file_number,
            tox_sys::Tox_File_Control::Resume,
            &mut err,
        )
    };
    if ok {
        let mut st = state.lock().unwrap();
        st.incoming.insert(
            (friend_number, file_number),
            IncomingFile {
                file: f,
                dest: dest.clone(),
                size: file_size,
                written: 0,
            },
        );
        drop(st);
        let _ = events.send(ToxEvent::Notice(format!(
            "accepting {filename} → {}",
            dest.display()
        )));
    } else {
        // offer already consumed: cancel explicitly so state stays consistent
        let mut cerr = tox_sys::Tox_Err_File_Control::Ok;
        unsafe {
            tox_sys::tox_file_control(
                tox,
                friend_number,
                file_number,
                tox_sys::Tox_File_Control::Cancel,
                &mut cerr,
            );
        }
        let _ = events.send(ToxEvent::Notice(format!(
            "accept failed ({err:?}), transfer cancelled: {filename}"
        )));
    }
}

fn save_tox(tox: *mut tox_sys::Tox, path: &Path) {
    unsafe {
        let size = tox_sys::tox_get_savedata_size(tox);
        let mut buf = vec![0u8; size];
        tox_sys::tox_get_savedata(tox, buf.as_mut_ptr());
        if let Err(e) = write_private(path, &buf) {
            log::error!("failed to save tox.save: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

pub(crate) async fn spawn(config: ToxConfig) -> Result<ToxHandle> {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<ToxCommand>();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<ToxEvent>();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<Identity>>();

    let worker = tokio::task::spawn_blocking(move || -> Result<()> {
        let data_dir = config.data_dir.clone();
        create_dir_private(&data_dir).with_context(|| format!("create {}", data_dir.display()))?;
        let identity_store = IdentityStore::new(&data_dir)?;
        let state = Arc::new(Mutex::new(CoreState::default()));
        let bootstrap = config.bootstrap.clone();

        // Build Tox + identity + callback context together so user_data is stable.
        let save_path = data_dir.join("tox.save");
        let savedata = if save_path.exists() {
            std::fs::read(&save_path)?
        } else {
            Vec::new()
        };

        let (tox, identity, ctx_ptr) = unsafe {
            let mut opt_err = tox_sys::Tox_Err_Options_New::Ok;
            let options = tox_sys::tox_options_new(&mut opt_err);
            if options.is_null() {
                bail!("tox_options_new failed: {opt_err:?}");
            }
            if !savedata.is_empty() {
                tox_sys::tox_options_set_savedata_type(
                    options,
                    tox_sys::Tox_Savedata_Type::ToxSave,
                );
                tox_sys::tox_options_set_savedata_data(options, savedata.as_ptr(), savedata.len());
                tox_sys::tox_options_set_savedata_length(options, savedata.len());
            }
            let mut err = tox_sys::Tox_Err_New::Ok;
            let mut tox = tox_sys::tox_new(options, &mut err);
            tox_sys::tox_options_free(options);

            if tox.is_null()
                && matches!(
                    err,
                    tox_sys::Tox_Err_New::LoadBadFormat | tox_sys::Tox_Err_New::LoadEncrypted
                )
            {
                log::warn!("tox.save unusable ({err:?}); starting fresh identity");
                let _ = std::fs::remove_file(&save_path);
                let mut opt_err = tox_sys::Tox_Err_Options_New::Ok;
                let options = tox_sys::tox_options_new(&mut opt_err);
                let mut err2 = tox_sys::Tox_Err_New::Ok;
                tox = tox_sys::tox_new(options, &mut err2);
                tox_sys::tox_options_free(options);
                if tox.is_null() {
                    bail!("tox_new failed: {err2:?}");
                }
            } else if tox.is_null() {
                bail!("tox_new failed: {err:?}");
            }

            register_callbacks(tox);

            let mut address = [0u8; tox_sys::TOX_ADDRESS_SIZE];
            let mut pk = [0u8; tox_sys::TOX_PUBLIC_KEY_SIZE];
            tox_sys::tox_self_get_address(tox, &mut address);
            tox_sys::tox_self_get_public_key(tox, &mut pk);

            let identity = Identity {
                address: hex::encode(address),
                public_key: hex::encode(pk),
            };

            let ctx = Box::new(CallbackCtx {
                events: evt_tx.clone(),
                state: state.clone(),
            });
            let ctx_ptr = Box::into_raw(ctx) as *mut c_void;

            // Apply optional profile.
            if let Some(ref n) = config.name {
                if !n.is_empty() {
                    let mut e = tox_sys::Tox_Err_Set_Info::Ok;
                    tox_sys::tox_self_set_name(tox, n.as_ptr(), n.len(), &mut e);
                }
            }
            if let Some(ref sm) = config.status_message {
                if !sm.is_empty() {
                    let mut e = tox_sys::Tox_Err_Set_Info::Ok;
                    tox_sys::tox_self_set_status_message(tox, sm.as_ptr(), sm.len(), &mut e);
                }
            }
            save_tox(tox, &save_path);
            bootstrap_all(tox, &bootstrap);
            let initial_friends = collect_friend_list(tox);
            evt_tx.send(ToxEvent::FriendList(initial_friends)).ok();

            (tox, identity, ctx_ptr)
        };

        let raw = RawTox { ptr: tox };
        if let Err(e) = identity_store.save(&identity) {
            let _ = ready_tx.send(Err(e));
            return Ok(());
        }
        // Signal the async side that the core is up.
        let _ = ready_tx.send(Ok(identity.clone()));

        let mut shutdown = false;
        let mut last_bootstrap = std::time::Instant::now();

        while !shutdown {
            // Drain pending commands.
            loop {
                match cmd_rx.try_recv() {
                    Ok(cmd) => {
                        handle_command(
                            tox,
                            cmd,
                            &state,
                            &evt_tx,
                            &save_path,
                            &mut shutdown,
                            &bootstrap,
                            &mut last_bootstrap,
                        );
                        if shutdown {
                            break;
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        shutdown = true;
                        break;
                    }
                }
            }
            if shutdown {
                break;
            }

            // Pump the network with our callback context as user_data.
            let interval = unsafe { tox_sys::tox_iteration_interval(tox) };
            unsafe {
                tox_sys::tox_iterate(tox, ctx_ptr);
            }

            let self_conn = unsafe { tox_sys::tox_self_get_connection_status(tox) };
            if matches!(self_conn, tox_sys::Tox_Connection::None)
                && last_bootstrap.elapsed().as_secs() >= 30
            {
                bootstrap_all(tox, &bootstrap);
                last_bootstrap = std::time::Instant::now();
            }

            // Sleep until the next deadline, waking early for commands.
            let sleep_ms = (interval as u64).clamp(1, 1000);
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(sleep_ms);
            while std::time::Instant::now() < deadline {
                match cmd_rx.try_recv() {
                    Ok(cmd) => {
                        handle_command(
                            tox,
                            cmd,
                            &state,
                            &evt_tx,
                            &save_path,
                            &mut shutdown,
                            &bootstrap,
                            &mut last_bootstrap,
                        );
                        break; // handle, then re-enter main loop promptly
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        shutdown = true;
                        break;
                    }
                }
                if shutdown {
                    break;
                }
            }
        }

        save_tox(tox, &save_path);
        drop(raw);
        unsafe {
            if !ctx_ptr.is_null() {
                drop(Box::from_raw(ctx_ptr as *mut CallbackCtx));
            }
        }
        Ok(())
    });

    // Wait for core setup to finish (identity ready), then hand out the handle.
    // The worker keeps running the iterate loop until Shutdown.
    let identity = ready_rx
        .await
        .map_err(|_| anyhow::anyhow!("tox core task died during setup"))??;

    // Keep worker alive: detach by dropping the JoinHandle (it continues).
    // If the worker finishes early (setup error already propagated above),
    // dropping is fine.
    drop(worker);

    Ok(ToxHandle {
        cmd_tx,
        events: evt_rx,
        identity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_path_and_control() {
        assert_eq!(sanitize_filename("report.pdf"), "report.pdf");
        assert_eq!(sanitize_filename("../../etc/passwd"), "____etc_passwd");
        assert_eq!(sanitize_filename("a/b\\c.txt"), "a_b_c.txt");
        assert_eq!(sanitize_filename("bad\nname\0.txt"), "bad_name_.txt");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename("."), "download");
        assert_eq!(sanitize_filename(""), "download");
        assert_eq!(sanitize_filename("   "), "download");
        assert_eq!(sanitize_filename("..."), "download");
        assert_eq!(sanitize_filename("normal name.txt"), "normal name.txt");
        // no path separators survive
        let out = sanitize_filename("../evil/../file.txt");
        assert!(!out.contains('/'), "got {out:?}");
        assert!(!out.contains('\\'), "got {out:?}");
        assert!(!out.contains(".."), "got {out:?}");
    }

    #[test]
    fn sanitize_truncates_long_names() {
        let long = format!("{}.txt", "x".repeat(500));
        let out = sanitize_filename(&long);
        assert!(out.len() <= 200, "got {} bytes", out.len());
        assert!(out.ends_with(".txt"));
    }

    #[test]
    fn unique_path_for_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("tox-up-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (p1, f1) = unique_path_for(&dir, "file.txt").unwrap();
        drop(f1);
        std::fs::write(&p1, b"first").unwrap();

        let (p2, f2) = unique_path_for(&dir, "file.txt").unwrap();
        drop(f2);
        assert_ne!(p1, p2);
        assert!(p2.to_string_lossy().contains("(1)"));
        // original untouched
        assert_eq!(std::fs::read(&p1).unwrap(), b"first");
        // second file empty/new
        assert_eq!(std::fs::read(&p2).unwrap(), b"");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn unique_path_for_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("tox-upm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (p, f) = unique_path_for(&dir, "secret.bin").unwrap();
        drop(f);
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
