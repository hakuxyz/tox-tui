// raw FFI to c-toxcore (tox.h 0.2.x) — only what the client needs

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]

use std::os::raw::{c_char, c_uint, c_void};

// ---------------------------------------------------------------------------
// Constants (from tox.h)
// ---------------------------------------------------------------------------

pub const TOX_PUBLIC_KEY_SIZE: usize = 32;
pub const TOX_SECRET_KEY_SIZE: usize = 32;
pub const TOX_ADDRESS_SIZE: usize = 38;
pub const TOX_NOSPAM_SIZE: usize = 4;
pub const TOX_DHT_ID_SIZE: usize = 32;
pub const TOX_MAX_NAME_LENGTH: usize = 128;
pub const TOX_MAX_STATUS_MESSAGE_LENGTH: usize = 1007;
pub const TOX_MAX_MESSAGE_LENGTH: usize = 1372;
pub const TOX_MAX_FRIEND_REQUEST_LENGTH: usize = 921;
pub const TOX_MAX_FILENAME_LENGTH: usize = 255;
pub const TOX_MAX_HOSTNAME_LENGTH: usize = 255;
pub const TOX_HASH_LENGTH: usize = 32;
pub const TOX_FILE_ID_LENGTH: usize = 32;

// ---------------------------------------------------------------------------
// Opaque types & fixed-size aliases
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct Tox {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct Tox_Options {
    _opaque: [u8; 0],
}

pub type Tox_Public_Key = [u8; TOX_PUBLIC_KEY_SIZE];
pub type Tox_Secret_Key = [u8; TOX_SECRET_KEY_SIZE];
pub type Tox_Address = [u8; TOX_ADDRESS_SIZE];
pub type Tox_Dht_Id = [u8; TOX_DHT_ID_SIZE];
pub type Tox_Hash = [u8; TOX_HASH_LENGTH];
pub type Tox_File_Id = [u8; TOX_FILE_ID_LENGTH];

pub type Tox_Friend_Number = u32;
pub type Tox_File_Number = u32;
pub type Tox_Friend_Message_Id = u32;

// ---------------------------------------------------------------------------
// Enums (repr(C) matches the C ABI: int-sized enums)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_User_Status {
    None = 0,
    Away = 1,
    Busy = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Message_Type {
    Normal = 0,
    Action = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Connection {
    None = 0,
    Tcp = 1,
    Udp = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_File_Control {
    Resume = 0,
    Pause = 1,
    Cancel = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_New {
    Ok = 0,
    Null = 1,
    Malloc = 2,
    PortAlloc = 3,
    ProxyBadType = 4,
    ProxyBadHost = 5,
    ProxyBadPort = 6,
    ProxyNotFound = 7,
    LoadEncrypted = 8,
    LoadBadFormat = 9,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Bootstrap {
    Ok = 0,
    Null = 1,
    BadHost = 2,
    BadPort = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Set_Info {
    Ok = 0,
    Null = 1,
    TooLong = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Friend_Add {
    Ok = 0,
    Null = 1,
    TooLong = 2,
    NoMessage = 3,
    OwnKey = 4,
    AlreadySent = 5,
    BadChecksum = 6,
    SetNewNospam = 7,
    Malloc = 8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Friend_Delete {
    Ok = 0,
    FriendNotFound = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Friend_By_Public_Key {
    Ok = 0,
    Null = 1,
    FriendNotFound = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Friend_Get_Public_Key {
    Ok = 0,
    FriendNotFound = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Friend_Query {
    Ok = 0,
    Null = 1,
    FriendNotFound = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Friend_Send_Message {
    Ok = 0,
    Null = 1,
    FriendNotFound = 2,
    FriendNotConnected = 3,
    SendQ = 4,
    TooLong = 5,
    Empty = 6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_File_Send {
    Ok = 0,
    Null = 1,
    FriendNotFound = 2,
    FriendNotConnected = 3,
    NameTooLong = 4,
    TooMany = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_File_Control {
    Ok = 0,
    FriendNotFound = 1,
    FriendNotConnected = 2,
    NotFound = 3,
    NotPaused = 4,
    Denied = 5,
    AlreadyPaused = 6,
    SendQ = 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_File_Send_Chunk {
    Ok = 0,
    Null = 1,
    FriendNotFound = 2,
    FriendNotConnected = 3,
    NotFound = 4,
    NotTransient = 5,
    SendQ = 6,
    WrongPosition = 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Get_Port {
    Ok = 0,
    NotBound = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Savedata_Type {
    None = 0,
    ToxSave = 1,
    SecretKey = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Proxy_Type {
    None = 0,
    Http = 1,
    Socks5 = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Err_Options_New {
    Ok = 0,
    Malloc = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tox_Log_Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
}

pub type tox_log_cb = extern "C" fn(
    tox: *mut Tox,
    level: Tox_Log_Level,
    file: *const c_char,
    line: c_uint,
    func: *const c_char,
    message: *const c_char,
    user_data: *mut c_void,
);

// ---------------------------------------------------------------------------
// Callback signatures
// ---------------------------------------------------------------------------

pub type tox_self_connection_status_cb =
    extern "C" fn(tox: *mut Tox, connection_status: Tox_Connection, user_data: *mut c_void);

pub type tox_friend_request_cb = extern "C" fn(
    tox: *mut Tox,
    public_key: *const Tox_Public_Key,
    message: *const u8,
    length: usize,
    user_data: *mut c_void,
);

pub type tox_friend_message_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    type_: Tox_Message_Type,
    message: *const u8,
    length: usize,
    user_data: *mut c_void,
);

pub type tox_friend_name_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    name: *const u8,
    length: usize,
    user_data: *mut c_void,
);

pub type tox_friend_status_message_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    message: *const u8,
    length: usize,
    user_data: *mut c_void,
);

pub type tox_friend_status_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    status: Tox_User_Status,
    user_data: *mut c_void,
);

pub type tox_friend_connection_status_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    connection_status: Tox_Connection,
    user_data: *mut c_void,
);

pub type tox_friend_typing_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    typing: bool,
    user_data: *mut c_void,
);

pub type tox_friend_read_receipt_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    message_id: Tox_Friend_Message_Id,
    user_data: *mut c_void,
);

pub type tox_file_recv_control_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    file_number: Tox_File_Number,
    control: Tox_File_Control,
    user_data: *mut c_void,
);

pub type tox_file_chunk_request_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    file_number: Tox_File_Number,
    position: u64,
    length: usize,
    user_data: *mut c_void,
);

pub type tox_file_recv_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    file_number: Tox_File_Number,
    kind: u32,
    file_size: u64,
    filename: *const u8,
    filename_length: usize,
    user_data: *mut c_void,
);

pub type tox_file_recv_chunk_cb = extern "C" fn(
    tox: *mut Tox,
    friend_number: Tox_Friend_Number,
    file_number: Tox_File_Number,
    position: u64,
    data: *const u8,
    length: usize,
    user_data: *mut c_void,
);

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

extern "C" {
    pub fn tox_options_new(error: *mut Tox_Err_Options_New) -> *mut Tox_Options;
    pub fn tox_options_free(options: *mut Tox_Options);
    pub fn tox_options_set_ipv6_enabled(options: *mut Tox_Options, ipv6_enabled: bool);
    pub fn tox_options_set_udp_enabled(options: *mut Tox_Options, udp_enabled: bool);
    pub fn tox_options_set_proxy_type(options: *mut Tox_Options, proxy_type: Tox_Proxy_Type);
    pub fn tox_options_set_proxy_host(options: *mut Tox_Options, proxy_host: *const c_char)
        -> bool;
    pub fn tox_options_set_proxy_port(options: *mut Tox_Options, proxy_port: u16);
    pub fn tox_options_set_savedata_type(
        options: *mut Tox_Options,
        savedata_type: Tox_Savedata_Type,
    );
    pub fn tox_options_set_savedata_data(
        options: *mut Tox_Options,
        savedata_data: *const u8,
        savedata_length: usize,
    );
    pub fn tox_options_set_savedata_length(options: *mut Tox_Options, savedata_length: usize);
    pub fn tox_options_set_local_discovery_enabled(
        options: *mut Tox_Options,
        local_discovery_enabled: bool,
    );
    pub fn tox_options_set_log_callback(options: *mut Tox_Options, callback: Option<tox_log_cb>);
}

// ---------------------------------------------------------------------------
// Lifecycle & event loop
// ---------------------------------------------------------------------------

extern "C" {
    pub fn tox_new(options: *const Tox_Options, error: *mut Tox_Err_New) -> *mut Tox;
    pub fn tox_kill(tox: *mut Tox);
    pub fn tox_get_savedata_size(tox: *const Tox) -> usize;
    pub fn tox_get_savedata(tox: *const Tox, savedata: *mut u8);
    pub fn tox_iteration_interval(tox: *const Tox) -> u32;
    pub fn tox_iterate(tox: *mut Tox, user_data: *mut c_void);

    pub fn tox_bootstrap(
        tox: *mut Tox,
        host: *const c_char,
        port: u16,
        public_key: *const Tox_Dht_Id,
        error: *mut Tox_Err_Bootstrap,
    ) -> bool;
    pub fn tox_add_tcp_relay(
        tox: *mut Tox,
        host: *const c_char,
        port: u16,
        public_key: *const Tox_Dht_Id,
        error: *mut Tox_Err_Bootstrap,
    ) -> bool;
}

// ---------------------------------------------------------------------------
// Self identity & profile
// ---------------------------------------------------------------------------

extern "C" {
    pub fn tox_self_get_address(tox: *const Tox, address: *mut Tox_Address);
    pub fn tox_self_get_nospam(tox: *const Tox) -> u32;
    pub fn tox_self_set_nospam(tox: *mut Tox, nospam: u32);
    pub fn tox_self_get_public_key(tox: *const Tox, public_key: *mut Tox_Public_Key);
    pub fn tox_self_get_secret_key(tox: *const Tox, secret_key: *mut Tox_Secret_Key);
    pub fn tox_self_set_name(
        tox: *mut Tox,
        name: *const u8,
        length: usize,
        error: *mut Tox_Err_Set_Info,
    ) -> bool;
    pub fn tox_self_get_name_size(tox: *const Tox) -> usize;
    pub fn tox_self_get_name(tox: *const Tox, name: *mut u8);
    pub fn tox_self_set_status_message(
        tox: *mut Tox,
        status_message: *const u8,
        length: usize,
        error: *mut Tox_Err_Set_Info,
    ) -> bool;
    pub fn tox_self_get_status_message_size(tox: *const Tox) -> usize;
    pub fn tox_self_get_status_message(tox: *const Tox, status_message: *mut u8);
    pub fn tox_self_set_typing(
        tox: *mut Tox,
        friend_number: Tox_Friend_Number,
        typing: bool,
    ) -> bool;
    pub fn tox_self_get_connection_status(tox: *const Tox) -> Tox_Connection;
    pub fn tox_self_get_dht_id(tox: *const Tox, dht_id: *mut Tox_Dht_Id);
    pub fn tox_self_get_udp_port(tox: *const Tox, error: *mut Tox_Err_Get_Port) -> u16;
    pub fn tox_self_get_tcp_port(tox: *const Tox, error: *mut Tox_Err_Get_Port) -> u16;
}

// ---------------------------------------------------------------------------
// Friends
// ---------------------------------------------------------------------------

extern "C" {
    pub fn tox_friend_add(
        tox: *mut Tox,
        address: *const Tox_Address,
        message: *const u8,
        length: usize,
        error: *mut Tox_Err_Friend_Add,
    ) -> Tox_Friend_Number;
    pub fn tox_friend_add_norequest(
        tox: *mut Tox,
        public_key: *const Tox_Public_Key,
        error: *mut Tox_Err_Friend_Add,
    ) -> Tox_Friend_Number;
    pub fn tox_friend_delete(
        tox: *mut Tox,
        friend_number: Tox_Friend_Number,
        error: *mut Tox_Err_Friend_Delete,
    ) -> bool;
    pub fn tox_friend_by_public_key(
        tox: *const Tox,
        public_key: *const Tox_Public_Key,
        error: *mut Tox_Err_Friend_By_Public_Key,
    ) -> Tox_Friend_Number;
    pub fn tox_friend_get_public_key(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        public_key: *mut Tox_Public_Key,
        error: *mut Tox_Err_Friend_Get_Public_Key,
    ) -> bool;
    pub fn tox_friend_get_last_online(tox: *const Tox, friend_number: Tox_Friend_Number) -> u64;
    pub fn tox_friend_get_name_size(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        error: *mut Tox_Err_Friend_Query,
    ) -> usize;
    pub fn tox_friend_get_name(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        name: *mut u8,
        error: *mut Tox_Err_Friend_Query,
    ) -> bool;
    pub fn tox_friend_get_status_message_size(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        error: *mut Tox_Err_Friend_Query,
    ) -> usize;
    pub fn tox_friend_get_status_message(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        status_message: *mut u8,
        error: *mut Tox_Err_Friend_Query,
    ) -> bool;
    pub fn tox_friend_get_status(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        error: *mut Tox_Err_Friend_Query,
    ) -> Tox_User_Status;
    pub fn tox_friend_get_connection_status(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        error: *mut Tox_Err_Friend_Query,
    ) -> Tox_Connection;
    pub fn tox_friend_send_message(
        tox: *mut Tox,
        friend_number: Tox_Friend_Number,
        type_: Tox_Message_Type,
        message: *const u8,
        length: usize,
        error: *mut Tox_Err_Friend_Send_Message,
    ) -> Tox_Friend_Message_Id;
    pub fn tox_self_get_friend_list_size(tox: *const Tox) -> usize;
    pub fn tox_self_get_friend_list(tox: *const Tox, friend_list: *mut Tox_Friend_Number);
}

// ---------------------------------------------------------------------------
// File transfer
// ---------------------------------------------------------------------------

extern "C" {
    pub fn tox_file_send(
        tox: *mut Tox,
        friend_number: Tox_Friend_Number,
        kind: u32,
        file_size: u64,
        file_id: *const Tox_File_Id,
        filename: *const u8,
        filename_length: usize,
        error: *mut Tox_Err_File_Send,
    ) -> Tox_File_Number;
    pub fn tox_file_control(
        tox: *mut Tox,
        friend_number: Tox_Friend_Number,
        file_number: Tox_File_Number,
        control: Tox_File_Control,
        error: *mut Tox_Err_File_Control,
    ) -> bool;
    pub fn tox_file_send_chunk(
        tox: *mut Tox,
        friend_number: Tox_Friend_Number,
        file_number: Tox_File_Number,
        position: u64,
        data: *const u8,
        length: usize,
        error: *mut Tox_Err_File_Send_Chunk,
    ) -> bool;
    pub fn tox_file_get_file_id(
        tox: *const Tox,
        friend_number: Tox_Friend_Number,
        file_number: Tox_File_Number,
        file_id: *mut Tox_File_Id,
        error: *mut Tox_Err_Friend_Query,
    ) -> bool;
}

// ---------------------------------------------------------------------------
// Callback registration
// ---------------------------------------------------------------------------

extern "C" {
    pub fn tox_callback_self_connection_status(
        tox: *mut Tox,
        callback: Option<tox_self_connection_status_cb>,
    );
    pub fn tox_callback_friend_request(tox: *mut Tox, callback: Option<tox_friend_request_cb>);
    pub fn tox_callback_friend_message(tox: *mut Tox, callback: Option<tox_friend_message_cb>);
    pub fn tox_callback_friend_name(tox: *mut Tox, callback: Option<tox_friend_name_cb>);
    pub fn tox_callback_friend_status_message(
        tox: *mut Tox,
        callback: Option<tox_friend_status_message_cb>,
    );
    pub fn tox_callback_friend_status(tox: *mut Tox, callback: Option<tox_friend_status_cb>);
    pub fn tox_callback_friend_connection_status(
        tox: *mut Tox,
        callback: Option<tox_friend_connection_status_cb>,
    );
    pub fn tox_callback_friend_typing(tox: *mut Tox, callback: Option<tox_friend_typing_cb>);
    pub fn tox_callback_friend_read_receipt(
        tox: *mut Tox,
        callback: Option<tox_friend_read_receipt_cb>,
    );
    pub fn tox_callback_file_recv_control(
        tox: *mut Tox,
        callback: Option<tox_file_recv_control_cb>,
    );
    pub fn tox_callback_file_chunk_request(
        tox: *mut Tox,
        callback: Option<tox_file_chunk_request_cb>,
    );
    pub fn tox_callback_file_recv(tox: *mut Tox, callback: Option<tox_file_recv_cb>);
    pub fn tox_callback_file_recv_chunk(tox: *mut Tox, callback: Option<tox_file_recv_chunk_cb>);
}
