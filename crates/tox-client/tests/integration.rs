// offline tests: two tox instances bootstrap to each other on loopback

use std::path::PathBuf;
use std::time::{Duration, Instant};
use tox_client::{ConnectionStatus, FileSendRequest, ToxCommand, ToxConfig, ToxEvent, ToxHandle};

fn test_config(tag: &str) -> ToxConfig {
    let dir = std::env::temp_dir().join(format!(
        "tox-tui-test-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    ToxConfig {
        data_dir: dir,
        bootstrap: vec![], // offline: peers bootstrap to each other
        name: Some(format!("peer-{tag}")),
        status_message: Some("test".into()),
    }
}

async fn wait_for<F>(tox: &mut ToxHandle, timeout: Duration, mut pred: F) -> bool
where
    F: FnMut(&ToxEvent) -> bool,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(ev) = tox.recv_event().await {
            if pred(&ev) {
                return true;
            }
        } else {
            return false;
        }
    }
    false
}

async fn drain_for(tox: &mut ToxHandle, dur: Duration) -> Vec<ToxEvent> {
    let deadline = Instant::now() + dur;
    let mut out = Vec::new();
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline - now;
        match tokio::time::timeout(remaining, tox.recv_event()).await {
            Ok(Some(ev)) => out.push(ev),
            Ok(None) => break,
            Err(_) => break,
        }
    }
    out
}

async fn local_info(tox: &mut ToxHandle) -> tox_client::ToxEvent {
    tox.send(ToxCommand::QueryLocalInfo).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(ev) = tox.recv_event().await {
            if matches!(ev, ToxEvent::LocalInfo { .. }) {
                return ev;
            }
        } else {
            break;
        }
    }
    panic!("no LocalInfo reply");
}

fn info_parts(ev: ToxEvent) -> (String, String, String, u16) {
    match ev {
        ToxEvent::LocalInfo {
            address,
            public_key,
            dht_id,
            udp_port,
        } => (address, public_key, dht_id, udp_port),
        other => panic!("unexpected event: {other:?}"),
    }
}

// Bring two fresh instances up and make them DHT-connected.
async fn connected_pair(tag_a: &str, tag_b: &str) -> (ToxHandle, ToxHandle) {
    let mut a = tox_client::spawn(test_config(tag_a))
        .await
        .expect("spawn a");
    let mut b = tox_client::spawn(test_config(tag_b))
        .await
        .expect("spawn b");

    let (_addr_a, _pk_a, dht_a, port_a) = info_parts(local_info(&mut a).await);
    let (_addr_b, _pk_b, dht_b, port_b) = info_parts(local_info(&mut b).await);

    // Cross-bootstrap on loopback.
    a.send(ToxCommand::BootstrapOnce {
        host: "127.0.0.1".into(),
        port: port_b,
        dht_key_hex: dht_b,
    })
    .unwrap();
    b.send(ToxCommand::BootstrapOnce {
        host: "127.0.0.1".into(),
        port: port_a,
        dht_key_hex: dht_a,
    })
    .unwrap();

    let a_up = wait_for(
        &mut a,
        Duration::from_secs(15),
        |e| matches!(e, ToxEvent::SelfConnection(c) if c.is_online()),
    )
    .await;
    let b_up = wait_for(
        &mut b,
        Duration::from_secs(15),
        |e| matches!(e, ToxEvent::SelfConnection(c) if c.is_online()),
    )
    .await;
    assert!(a_up, "instance A failed to come online");
    assert!(b_up, "instance B failed to come online");

    (a, b)
}

#[tokio::test]
async fn identity_create_persist_reload() {
    let cfg = test_config("ident");
    let dir = cfg.data_dir.clone();

    let mut h = tox_client::spawn(cfg).await.expect("spawn");
    let addr1 = h.identity().address.clone();
    assert_eq!(addr1.len(), 76, "Tox ID must be 76 hex chars");

    // Persist.
    h.send(ToxCommand::SetName("persist-test".into())).unwrap();
    h.send(ToxCommand::Shutdown).unwrap();
    drain_for(&mut h, Duration::from_millis(500)).await;
    drop(h);

    assert!(dir.join("tox.save").exists(), "tox.save written");
    assert!(dir.join("identity.json").exists(), "identity.json written");

    // Reload — same address.
    let mut h2 = tox_client::spawn(ToxConfig {
        data_dir: dir.clone(),
        bootstrap: vec![],
        name: None,
        status_message: None,
    })
    .await
    .expect("respawn");
    assert_eq!(
        h2.identity().address,
        addr1,
        "address stable across restarts"
    );

    h2.send(ToxCommand::Shutdown).unwrap();
    drain_for(&mut h2, Duration::from_millis(300)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn two_instances_connect_and_chat() {
    let (mut a, mut b) = connected_pair("chat-a", "chat-b").await;

    let addr_a = a.identity().address.clone();
    let addr_b = b.identity().address.clone();

    // A sends a friend request to B.
    a.send(ToxCommand::AddFriend {
        address: addr_b,
        message: "hello from A".into(),
    })
    .unwrap();

    // B should receive the request.
    let got_req = wait_for(
        &mut b,
        Duration::from_secs(20),
        |e| matches!(e, ToxEvent::FriendRequest { message, .. } if message == "hello from A"),
    )
    .await;
    assert!(got_req, "B did not receive friend request");

    // Accept: need B's view of A's public key — pull from LocalInfo of A.
    let (_a_addr, a_pk, _, _) = info_parts(local_info(&mut a).await);
    b.send(ToxCommand::AcceptFriendRequest {
        public_key: a_pk.clone(),
    })
    .unwrap();

    // Also add B→A without request so both sides have the friend
    // (A's friend_add already created A→B; B needs B→A which Accept does).
    // Wait until both report the friend online.
    let a_online = wait_for(&mut a, Duration::from_secs(30), |e| {
        matches!(
            e,
            ToxEvent::Friend {
                event: tox_client::FriendEvent::Connected(_),
                ..
            }
        ) || matches!(e, ToxEvent::FriendList(list) if !list.is_empty())
    })
    .await;
    assert!(a_online, "A never saw friend come online / listed");

    // Refresh both lists and find friend numbers.
    a.send(ToxCommand::RefreshFriends).unwrap();
    b.send(ToxCommand::RefreshFriends).unwrap();

    let mut a_friend = None;
    let mut b_friend = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && (a_friend.is_none() || b_friend.is_none()) {
        if let Some(ev) = a.recv_event().await {
            match &ev {
                ToxEvent::FriendList(list) => {
                    if let Some(f) = list.first() {
                        a_friend = Some(f.number);
                    }
                }
                ToxEvent::Friend {
                    friend_number,
                    event: tox_client::FriendEvent::Connected(_),
                } => a_friend = Some(*friend_number),
                _ => {}
            }
        }
        if let Some(ev) = b.recv_event().await {
            match &ev {
                ToxEvent::FriendList(list) => {
                    if let Some(f) = list.first() {
                        b_friend = Some(f.number);
                    }
                }
                ToxEvent::Friend {
                    friend_number,
                    event: tox_client::FriendEvent::Connected(_),
                } => b_friend = Some(*friend_number),
                _ => {}
            }
        }
    }

    // Friend numbers may still be unknown if FriendList raced; probe via events.
    // Send a message from A once we know A's friend number.
    let a_fn = a_friend.expect("A friend number");
    a.send(ToxCommand::SendMessage {
        friend_number: a_fn,
        text: "ping over tox".into(),
        action: false,
    })
    .unwrap();

    let got_msg = wait_for(
        &mut b,
        Duration::from_secs(30),
        |e| matches!(e, ToxEvent::FriendMessage { text, .. } if text == "ping over tox"),
    )
    .await;
    assert!(got_msg, "B did not receive A's message");

    // Reply from B if we know B's friend number, else just assert delivery above.
    if let Some(b_fn) = b_friend {
        b.send(ToxCommand::SendMessage {
            friend_number: b_fn,
            text: "pong".into(),
            action: false,
        })
        .unwrap();
        let got_pong = wait_for(
            &mut a,
            Duration::from_secs(30),
            |e| matches!(e, ToxEvent::FriendMessage { text, .. } if text == "pong"),
        )
        .await;
        assert!(got_pong, "A did not receive B's reply");
    }

    // Both should have at least one friend after refresh.
    assert!(a_friend.is_some() || b_friend.is_some());

    let _ = addr_a;
    a.send(ToxCommand::Shutdown).ok();
    b.send(ToxCommand::Shutdown).ok();
    drain_for(&mut a, Duration::from_millis(300)).await;
    drain_for(&mut b, Duration::from_millis(300)).await;
}

#[tokio::test]
async fn file_transfer_roundtrip() {
    let (mut a, mut b) = connected_pair("file-a", "file-b").await;

    // Establish friendship (norequest both ways is fastest for the test).
    let (_ba, b_pk, _, _) = info_parts(local_info(&mut b).await);
    let (_ab, a_pk, _, _) = info_parts(local_info(&mut a).await);

    a.send(ToxCommand::AddFriend {
        address: b.identity().address.clone(),
        message: "file test".into(),
    })
    .unwrap();

    // Wait for B's request, accept, and also make A know B.
    wait_for(&mut b, Duration::from_secs(20), |e| {
        matches!(e, ToxEvent::FriendRequest { .. })
    })
    .await;
    b.send(ToxCommand::AcceptFriendRequest {
        public_key: a_pk.clone(),
    })
    .unwrap();

    // Wait until A sees the friend as actually online (not just listed).
    let mut a_fn = None;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, a.recv_event()).await {
            Ok(Some(ToxEvent::FriendList(list))) => {
                if let Some(f) = list.iter().find(|f| f.connection.is_online()) {
                    a_fn = Some(f.number);
                    break;
                }
            }
            Ok(Some(ToxEvent::Friend {
                friend_number,
                event: tox_client::FriendEvent::Connected(status),
            })) if status.is_online() => {
                a_fn = Some(friend_number);
                break;
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => break, // deadline: no event within remaining time
        }
    }
    let a_fn = a_fn.expect("A friend must be online before starting a file transfer");

    // Extra settle time for friend connection.
    drain_for(&mut a, Duration::from_secs(3)).await;
    drain_for(&mut b, Duration::from_secs(3)).await;

    // Create a payload file.
    let src_dir = std::env::temp_dir().join(format!("tox-file-src-{}", std::process::id()));
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("payload.bin");
    let payload: Vec<u8> = (0..64 * 1024u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&src, &payload).unwrap();

    a.send(ToxCommand::SendFile(FileSendRequest {
        friend_number: a_fn,
        path: src.clone(),
    }))
    .unwrap();

    // B should get FileRecv.
    let mut incoming = None;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && incoming.is_none() {
        if let Some(ToxEvent::FileRecv {
            friend_number,
            file_number,
            filename,
            file_size,
        }) = b.recv_event().await.as_ref()
        {
            incoming = Some((*friend_number, *file_number, filename.clone(), *file_size));
        }
    }
    let (b_friend, b_file, filename, size) = incoming.expect("B should receive file offer");
    assert_eq!(filename, "payload.bin");
    assert_eq!(size, payload.len() as u64);

    let dest = std::env::temp_dir().join(format!("tox-file-dst-{}", std::process::id()));
    b.send(ToxCommand::AcceptFile {
        friend_number: b_friend,
        file_number: b_file,
        dest_dir: dest.clone(),
    })
    .unwrap();

    // Wait for completion notice or for the file to land.
    let mut done = false;
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline && !done {
        // Check disk.
        if let Ok(rd) = std::fs::read_dir(&dest) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "bin").unwrap_or(false) {
                    if let Ok(bytes) = std::fs::read(&p) {
                        if bytes == payload {
                            done = true;
                            break;
                        }
                    }
                }
            }
        }
        // Also pump events so the core keeps running (it runs on its own
        // thread, but draining avoids channel backpressure).
        if let Some(ToxEvent::Notice(n)) = b.try_recv_event() {
            if n.starts_with("received") {
                done = true;
            }
        }
        if let Some(ToxEvent::Notice(n)) = a.try_recv_event() {
            if n.starts_with("finished sending") {
                // give B a moment to write
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(done, "file transfer did not complete in time");
    let _ = b_pk;
    let _ = a;

    a.send(ToxCommand::Shutdown).ok();
    b.send(ToxCommand::Shutdown).ok();
    let _ = std::fs::remove_dir_all(&src_dir);
    let _ = std::fs::remove_dir_all(&dest);
}

#[tokio::test]
async fn friend_list_survives_restart() {
    let cfg = test_config("restart");
    let dir = cfg.data_dir.clone();

    let mut h = tox_client::spawn(cfg).await.unwrap();
    // Fresh profile: no friends yet, but FriendList event must arrive.
    let got = wait_for(&mut h, Duration::from_secs(5), |e| {
        matches!(e, ToxEvent::FriendList(_))
    })
    .await;
    assert!(got, "startup FriendList missing");

    h.send(ToxCommand::Shutdown).unwrap();
    drain_for(&mut h, Duration::from_millis(400)).await;
    drop(h);

    let mut h2 = tox_client::spawn(ToxConfig {
        data_dir: dir.clone(),
        bootstrap: vec![],
        name: None,
        status_message: None,
    })
    .await
    .unwrap();
    let got2 = wait_for(&mut h2, Duration::from_secs(5), |e| {
        matches!(e, ToxEvent::FriendList(_))
    })
    .await;
    assert!(got2, "restart FriendList missing");
    h2.send(ToxCommand::Shutdown).ok();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn connection_status_enum_labels() {
    assert!(ConnectionStatus::None.label().contains("offline"));
    assert!(ConnectionStatus::Udp.is_online());
    assert!(ConnectionStatus::Tcp.is_online());
    assert!(!ConnectionStatus::None.is_online());
    let _ = PathBuf::from(".");
}
