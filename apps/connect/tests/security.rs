//! What the security review of 2026-10-23 found, as tests (plans/amethystora-connect/review-security-opus.md): each
//! attack is made, from the outside, with the crate's own parts, against a copy of the program, and must fail.
//! Everything is one test, because the main context of a thread has one owner; the scenarios run one after the
//! other and have their own copy of the program each.

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use amethystora_connect::net::{self, LineReader, Trust};
use amethystora_connect::packet::{self, Packet};
use amethystora_connect::{dbus, identity, limits, payload, store, x509};
use common::*;
use gio::prelude::*;
use serde_json::json;

const X: &str = "4a8f2c7e_91d3_4b5a_8e6f_0c1d2e3f4a5b";

/// A phone made of the crate's parts: it opens the connection, so it is the TLS server
struct Raw {
    tls: gio::TlsConnection,
    reader: LineReader,
    der: Vec<u8>,
    _conn: gio::SocketConnection,
}

/// Connects to a port from an address of the loopback (127.0.0.N), if one is given
async fn tcp(port: u16, source: Option<&str>) -> Result<gio::SocketConnection, glib::Error> {
    let client = gio::SocketClient::new();
    if let Some(source) = source {
        client.set_local_address(Some(&gio::InetSocketAddress::new(&gio::InetAddress::from_string(source).unwrap(), 0)));
    }
    client.set_enable_proxy(false);
    client.connect_future(&gio::InetSocketAddress::new(&gio::InetAddress::from_string("127.0.0.1").unwrap(), port)).await
}

async fn raw_phone(port: u16, id: &str, name: &str, source: Option<&str>) -> Result<Raw, String> {
    let made = x509::generate(id, packet::now_secs()).unwrap();
    let cert = gio::TlsCertificate::from_pem(&format!("{}{}", made.certificate_pem, made.key_pem)).unwrap();
    let der = x509::pem_to_der(&made.certificate_pem).unwrap();
    let conn = tcp(port, source).await.map_err(|e| e.message().to_string())?;
    let local = identity::Local::new(id, name, "phone");
    net::write_all(&conn.output_stream(), local.packet(1716, true, None).to_line()).await.map_err(|e| e.to_string())?;
    let base: gio::IOStream = conn.clone().upcast();
    let secured = net::secure_as_server(&base, &cert, Trust::FirstUse).await.map_err(|e| e.to_string())?;
    net::write_all(&secured.tls.output_stream(), local.packet_in_tls().to_line()).await.map_err(|e| e.to_string())?;
    let mut reader = LineReader::new(secured.tls.input_stream());
    net::timed(Duration::from_secs(5), reader.next(65536)).await.map_err(|e| e.to_string())?.ok_or("closed before its identity")?;
    Ok(Raw { tls: secured.tls, reader, der, _conn: conn })
}

async fn send(raw: &Raw, p: Packet) {
    let _ = net::write_all(&raw.tls.output_stream(), p.to_line()).await;
}

fn pair_request(offset: i64) -> Packet {
    Packet::new("kdeconnect.pair", json!({ "pair": true, "timestamp": packet::now_secs() + offset }))
}

fn ids(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{:0>32}", format!("f{i}"))).collect()
}

#[test]
fn what_the_review_found() {
    let root = std::env::temp_dir().join(format!("amethystora-connect-security-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    glib::MainContext::default().block_on(async {
        takeover_of_a_pending_pairing(&root).await;
        a_request_goes_on_over_the_other_connection_of_the_same_phone(&root).await;
        prompt_flood_from_one_address(&root).await;
        prompt_flood_from_many_addresses(&root).await;
        forged_protocol_7_datagram(&root).await;
        a_second_connection_leaves_the_first_open(&root).await;
        a_phone_cannot_queue_files_without_end(&root).await;
        a_stalled_connection_does_not_starve_a_payload(&root).await;
        a_connection_that_is_not_the_pin_is_closed_when_the_phone_is_paired(&root).await;
        one_address_cannot_hold_the_unpaired_slots(&root).await;
        over_the_bus(&root).await;
    });
    let _ = std::fs::remove_dir_all(&root);
}

// --- 1. A second connection with the same id does not take a pending pairing over ------------------------------

async fn takeover_of_a_pending_pairing(root: &Path) {
    let computer = make(root, "Studio laptop", "laptop", false, false);
    let port = computer.port();

    // The phone asks, and the person is shown its key
    let honest = raw_phone(port, X, "Sam phone", None).await.unwrap();
    send(&honest, pair_request(0)).await;
    until("the prompt", || prompt_of(&computer, X).is_some()).await;
    let first = prompt_of(&computer, X).unwrap();
    assert!(first.key.starts_with(&format!("pair:{X}:")), "the prompt belongs to a connection: {}", first.key);
    assert!(!first.urgent);
    let closed_before = computer.notices.closed.borrow().len();

    // Someone else, with the same id and name, a moment later, with a certificate of its own
    let mut attacker = raw_phone(port, X, "Evil phone", None).await.unwrap();
    send(&attacker, pair_request(1)).await;
    // The second connection is taken (a phone that calls twice is no stranger), but its request is not answered and
    // not shown, and it changes nothing of the first's
    let answer = net::timed(Duration::from_secs(1), attacker.reader.next(65536)).await;
    assert!(matches!(answer, Err(net::Error::Timeout)), "nothing is said to it: {answer:?}");
    assert_eq!(computer.notices.raised_with_prefix(&format!("pair:{X}")).len(), 1, "no second prompt");
    assert_eq!(computer.device(X).name(), "Sam phone", "and the name the person was shown stays");
    assert!(!computer.notices.closed.borrow()[closed_before..].contains(&first.key), "the person's prompt stays");
    // The honest phone's connection is still there
    assert!(computer.service.device(X).is_some_and(|d| d.connected()));

    // A button of some other connection's prompt does nothing; the right one pins the honest phone
    computer.notices.press(&format!("pair:{X}:999999"), "accept");
    pause(100).await;
    assert!(!computer.service.store.borrow().paired.contains_key(X));
    computer.notices.press(&first.key, "accept");
    until("a pin", || computer.service.store.borrow().paired.contains_key(X)).await;
    let pin = computer.service.store.borrow().paired.get(X).map(|p| p.pin.clone());
    assert_eq!(pin.as_deref(), Some(&honest.der[..]), "the honest phone's certificate is the pin");
    assert_ne!(pin.as_deref(), Some(&attacker.der[..]));

    // The same for a request the person made: the computer asks, an impostor with the phone's id cannot take its place
    let computer = make(root, "Studio laptop two", "laptop", false, false);
    let port = computer.port();
    let other = "b".repeat(32);
    let honest = raw_phone(port, &other, "Sam phone", None).await.unwrap();
    until("the phone to be known", || computer.service.device(&other).is_some_and(|d| d.connected())).await;
    computer.service.pair(&other).unwrap();
    until("our request to be shown", || prompt_of(&computer, &other).is_some()).await;
    let ours = prompt_of(&computer, &other).unwrap();
    assert_eq!(ours.actions[0].0, "cancel");
    let mut attacker = raw_phone(port, &other, "Sam phone", None).await.unwrap();
    send(&attacker, Packet::new("kdeconnect.pair", json!({ "pair": true }))).await;
    let answer = net::timed(Duration::from_secs(1), attacker.reader.next(65536)).await;
    assert!(matches!(answer, Err(net::Error::Timeout)), "{answer:?}");
    assert!(!computer.service.store.borrow().paired.contains_key(&other), "an acceptance on another connection pins nothing");
    send(&honest, Packet::new("kdeconnect.pair", json!({ "pair": true }))).await;
    until("the pin", || computer.service.store.borrow().paired.contains_key(&other)).await;
    assert_eq!(computer.service.store.borrow().paired.get(&other).map(|p| p.pin.clone()).as_deref(), Some(&honest.der[..]));
    assert!(computer.notices.closed.borrow().contains(&ours.key));
}

// --- A phone that is called twice answers on either connection; one that only claims its id does not ------------------

async fn raw_phone_with(port: u16, id: &str, name: &str, cert_pem: &str) -> Raw {
    let cert = gio::TlsCertificate::from_pem(cert_pem).unwrap();
    let der = x509::pem_to_der(cert_pem).unwrap();
    let conn = tcp(port, None).await.unwrap();
    let local = identity::Local::new(id, name, "phone");
    net::write_all(&conn.output_stream(), local.packet(1716, true, None).to_line()).await.unwrap();
    let base: gio::IOStream = conn.clone().upcast();
    let secured = net::secure_as_server(&base, &cert, Trust::FirstUse).await.unwrap();
    net::write_all(&secured.tls.output_stream(), local.packet_in_tls().to_line()).await.unwrap();
    let mut reader = LineReader::new(secured.tls.input_stream());
    net::timed(Duration::from_secs(5), reader.next(65536)).await.unwrap().unwrap();
    Raw { tls: secured.tls, reader, der, _conn: conn }
}

async fn a_request_goes_on_over_the_other_connection_of_the_same_phone(root: &Path) {
    let id = "e".repeat(32);
    let made = x509::generate(&id, packet::now_secs()).unwrap();
    let pem = format!("{}{}", made.certificate_pem, made.key_pem);

    // We ask; the phone answers on its second connection (the same certificate)
    let computer = make(root, "Two connections", "laptop", false, false);
    let port = computer.port();
    let first = raw_phone_with(port, &id, "Sam phone", &pem).await;
    until("the phone", || computer.service.device(&id).is_some_and(|d| d.connected())).await;
    computer.service.pair(&id).unwrap();
    until("our request", || prompt_of(&computer, &id).is_some()).await;
    let second = raw_phone_with(port, &id, "Sam phone", &pem).await;
    send(&second, Packet::new("kdeconnect.pair", json!({ "pair": true }))).await;
    until("the pin", || computer.service.store.borrow().paired.contains_key(&id)).await;
    assert_eq!(computer.service.store.borrow().paired.get(&id).map(|p| p.pin.clone()).as_deref(), Some(&first.der[..]));
    assert_eq!(first.der, second.der);

    // The phone closes the connection the request went over: the request is carried on by the other, with the same key
    let id = "f".repeat(32);
    let made = x509::generate(&id, packet::now_secs()).unwrap();
    let pem = format!("{}{}", made.certificate_pem, made.key_pem);
    let computer = make(root, "Two connections again", "laptop", false, false);
    let port = computer.port();
    let first = raw_phone_with(port, &id, "Sam phone", &pem).await;
    send(&first, pair_request(0)).await;
    until("the prompt", || prompt_of(&computer, &id).is_some()).await;
    let before = prompt_of(&computer, &id).unwrap();
    let key = computer.service.device(&id).map(|d| d.st.borrow().pairing_key.clone()).unwrap();
    let second = raw_phone_with(port, &id, "Sam phone", &pem).await;
    let _ = first.tls.close(None::<&gio::Cancellable>);
    drop(first);
    until("the prompt to move", || computer.notices.raised_with_prefix(&format!("pair:{id}:")).len() == 2).await;
    let after = prompt_of(&computer, &id).unwrap();
    assert_ne!(after.key, before.key, "the prompt is for the connection that carries the request now");
    assert!(computer.notices.closed.borrow().contains(&before.key));
    assert_eq!(computer.service.device(&id).map(|d| d.st.borrow().pairing_key.clone()).unwrap(), key, "and the key is the same: it is the same phone");
    computer.notices.press(&before.key, "accept");
    pause(100).await;
    assert!(!computer.service.store.borrow().paired.contains_key(&id), "the old prompt answers nothing");
    computer.notices.press(&after.key, "accept");
    until("the pin", || computer.service.store.borrow().paired.contains_key(&id)).await;
    assert_eq!(computer.service.store.borrow().paired.get(&id).map(|p| p.pin.clone()).as_deref(), Some(&second.der[..]));

    // An impostor with the id and another certificate has no part in it
    let id = "9".repeat(32);
    let made = x509::generate(&id, packet::now_secs()).unwrap();
    let pem = format!("{}{}", made.certificate_pem, made.key_pem);
    let computer = make(root, "Two connections impostor", "laptop", false, false);
    let port = computer.port();
    let honest = raw_phone_with(port, &id, "Sam phone", &pem).await;
    send(&honest, pair_request(0)).await;
    until("the prompt", || prompt_of(&computer, &id).is_some()).await;
    let prompt = prompt_of(&computer, &id).unwrap();
    let impostor = raw_phone(port, &id, "Sam phone", None).await.unwrap();
    let _ = honest.tls.close(None::<&gio::Cancellable>);
    drop(honest);
    pause(400).await;
    assert!(computer.notices.closed.borrow().contains(&prompt.key), "with the honest connection gone the request is over");
    assert_eq!(computer.notices.raised_with_prefix(&format!("pair:{id}:")).len(), 1, "an impostor's connection does not inherit it");
    send(&impostor, pair_request(1)).await;
    until("a prompt of the impostor's own", || computer.notices.raised_with_prefix(&format!("pair:{id}:")).len() == 2).await;
    let theirs = prompt_of(&computer, &id).unwrap();
    assert_ne!(theirs.key, prompt.key);
}

// --- 4. Prompts: so many from one address, so many at once, none critical ---------------------------------------

async fn prompt_flood_from_one_address(root: &Path) {
    let computer = make(root, "Flood one", "laptop", false, false);
    let port = computer.port();
    let mut kept = Vec::new();
    for round in 0..3 {
        for id in ids(8) {
            if let Ok(raw) = raw_phone(port, &id, "Pixel 8", None).await {
                for k in 0..3 {
                    send(&raw, pair_request(round * 10 + k)).await;
                }
                kept.push(raw);
            }
        }
        pause(200).await;
    }
    let prompts = computer.notices.raised_with_prefix("pair:");
    assert!(!prompts.is_empty());
    assert!(prompts.len() <= amethystora_connect::limits::PAIR_REQUESTS_PER_MINUTE, "{} prompts from one address", prompts.len());
    assert!(prompts.iter().all(|n| !n.urgent), "none is critical");
}

async fn prompt_flood_from_many_addresses(root: &Path) {
    let computer = make(root, "Flood many", "laptop", false, false);
    let port = computer.port();
    let mut kept = Vec::new();
    for (n, id) in ids(8).into_iter().enumerate() {
        let source = format!("127.0.0.{}", 2 + n);
        if let Ok(raw) = raw_phone(port, &id, "Pixel 8", Some(&source)).await {
            send(&raw, pair_request(0)).await;
            kept.push(raw);
        }
        pause(50).await;
    }
    pause(300).await;
    let prompts = computer.notices.raised_with_prefix("pair:");
    assert_eq!(prompts.len(), amethystora_connect::limits::PAIR_PROMPTS_AT_ONCE, "eight addresses, three prompts at a time");
    assert!(prompts.iter().all(|n| !n.urgent));
}

// --- 5. A forged protocol 7 datagram does not rewrite a paired phone --------------------------------------------

async fn forged_protocol_7_datagram(root: &Path) {
    let computer = make(root, "Forged datagram", "laptop", false, true);
    let phone = make(root, "Sam phone", "phone", true, false);
    let (cid, pid) = (computer.id(), phone.id());
    connect_and_pair(&computer, &phone).await;
    // Both ends let go of each other
    for link in computer.device(&pid).st.borrow().links.iter() {
        link.close();
    }
    until("the computer to be alone", || !computer.device(&pid).connected() && !phone.device(&cid).connected()).await;

    // Someone who read the phone's id sends a datagram in its name: protocol 7, a name of its own, a made-up list
    // of what it does, and the address of a program that plays a phone of that version with the real certificate
    let listener = gio::SocketListener::new();
    let fake_port = listener.add_any_inet_port(None::<&glib::Object>).unwrap();
    let phone_cert = phone.service.me.tls.clone();
    let accepted = Rc::new(RefCell::new(false));
    let seen = accepted.clone();
    let acceptor = glib::spawn_future_local(async move {
        if let Ok((connection, _)) = listener.accept_future().await {
            // Protocol 7: read the plain identity, TLS as the client, send nothing inside
            let mut plain = LineReader::new(connection.input_stream());
            let _ = plain.next(8192).await;
            let base: gio::IOStream = connection.clone().upcast();
            if net::secure_as_client(&base, &phone_cert, Trust::FirstUse).await.is_ok() {
                *seen.borrow_mut() = true;
                glib::timeout_future(Duration::from_secs(2)).await;
            }
        }
    });
    // One attempt to one address in two seconds is all the computer makes: the one that paired was just now
    pause(2200).await;
    let forged = json!({ "id": 0, "type": "kdeconnect.identity", "body": { "deviceId": pid, "deviceName": "Evil", "deviceType": "phone", "protocolVersion": 7,
        "incomingCapabilities": ["kdeconnect.ping"], "outgoingCapabilities": ["kdeconnect.ping"], "tcpPort": fake_port } });
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut line = forged.to_string().into_bytes();
    line.push(b'\n');
    socket.send_to(&line, ("127.0.0.1", computer.service.udp_port().unwrap())).unwrap();
    until("the computer to call the made-up phone", || *accepted.borrow()).await;
    pause(400).await;
    acceptor.abort();

    let record = computer.service.store.borrow().paired.get(&pid).cloned().unwrap();
    assert_eq!(record.name, "Sam phone", "the paired phone keeps its name");
    assert_eq!(record.protocol, 8, "and its protocol");
    assert_ne!(record.incoming, vec!["kdeconnect.ping".to_string()]);
    assert_eq!(computer.device(&pid).name(), "Sam phone");
    assert!(!computer.device(&pid).connected(), "a phone that comes back with an older protocol than it paired with is not taken");
}

// --- A second connection of a paired phone does not close the first ---------------------------------------------

async fn a_second_connection_leaves_the_first_open(root: &Path) {
    let computer = make(root, "Two ways", "laptop", false, false);
    let phone = make(root, "Sam phone", "phone", true, false);
    connect_and_pair(&computer, &phone).await;
    let (cid, pid) = (computer.id(), phone.id());
    // Two devices that find each other at once each call the other. Fedora's KDE Connect takes the closing of the older
    // of two connections for the device leaving, so a connection that works is not closed because a newer one came.
    let first = computer.device(&pid).link().unwrap();
    phone.service.connect_to(&format!("127.0.0.1:{}", computer.port())).unwrap();
    until("the second connection", || computer.device(&pid).st.borrow().links.len() == 2).await;
    pause(500).await;
    assert!(!first.is_closed(), "the first connection stays");
    assert!(computer.device(&pid).connected() && phone.device(&cid).connected());
    // Past the limit the oldest goes, and the device stays connected
    // (a program calls one address once in two seconds)
    for _ in 1..limits::LINKS_PER_DEVICE {
        pause(2200).await;
        phone.service.connect_to(&format!("127.0.0.1:{}", computer.port())).unwrap();
    }
    until("the oldest to go", || first.is_closed()).await;
    assert!(computer.device(&pid).st.borrow().links.len() <= limits::LINKS_PER_DEVICE);
    assert!(computer.device(&pid).connected());
}

// --- 8. A phone cannot queue files without end ------------------------------------------------------------------

async fn a_phone_cannot_queue_files_without_end(root: &Path) {
    let computer = make(root, "Queue", "laptop", false, false);
    let phone = make(root, "Sam phone", "phone", true, false);
    connect_and_pair(&computer, &phone).await;
    let (cid, pid) = (computer.id(), phone.id());
    // A port that takes connections and says nothing: the files that are taken wait on it, the ones refused are seen to
    // call it once and give up
    let listener = gio::SocketListener::new();
    let port = listener.add_any_inet_port(None::<&glib::Object>).unwrap();
    let calls = Rc::new(std::cell::Cell::new(0usize));
    let counted = calls.clone();
    let held = Rc::new(RefCell::new(Vec::new()));
    let keep = held.clone();
    let counting = glib::spawn_future_local(async move {
        while let Ok((connection, _)) = listener.accept_future().await {
            counted.set(counted.get() + 1);
            keep.borrow_mut().push(connection);
        }
    });
    let link = phone.device(&cid).link().unwrap();
    for i in 0..80 {
        link.send(&Packet::new("kdeconnect.share.request", json!({ "filename": format!("file {i}.bin") })).with_payload(10, port));
    }
    pause(1500).await;
    let queued = computer.device(&pid).st.borrow().queued;
    assert_eq!(queued, 64, "no more than 64 are taken at a time");
    // Four are being fetched; the sixteen refused each called once
    assert!(calls.get() >= 4 + 16, "{} calls", calls.get());
    counting.abort();
}

// --- 11. A stalled connection does not keep the phone from its file ----------------------------------------------

async fn a_stalled_connection_does_not_starve_a_payload(root: &Path) {
    let (ours, _) = store::load_material(&root.join("payload-a"), packet::now_secs()).unwrap();
    let (theirs, _) = store::load_material(&root.join("payload-b"), packet::now_secs()).unwrap();
    let offer = payload::offer(&(0..=0)).unwrap();
    let port = offer.port;
    let content = pattern(300_000);
    let served = content.clone();
    let pin = Rc::new(theirs.der.clone());
    let cert = ours.tls.clone();
    let mut at = 0;
    let task = glib::spawn_future_local(async move {
        offer
            .serve(&cert, pin, served.len() as u64, &mut |buf| {
                let n = buf.len().min(served.len() - at);
                buf[..n].copy_from_slice(&served[at..at + n]);
                at += n;
                Ok(n)
            }, &|_| {})
            .await
    });
    // Strangers who connect and say nothing: more than there are handshakes at once
    let mut stalls = Vec::new();
    for _ in 0..20 {
        stalls.push(tcp(port, None).await.unwrap());
    }
    let received = Rc::new(RefCell::new(Vec::new()));
    let sink = received.clone();
    let started = std::time::Instant::now();
    let total = payload::fetch("127.0.0.1".parse().unwrap(), 0, port, &theirs.tls, Rc::new(ours.der.clone()), content.len() as u64, &mut |chunk| {
        sink.borrow_mut().extend_from_slice(chunk);
        Ok(())
    }, &|_| {})
    .await;
    assert!(total.is_ok(), "{total:?}");
    assert_eq!(*received.borrow(), content);
    assert!(started.elapsed() < Duration::from_secs(20), "{:?}", started.elapsed());
    assert!(task.await.unwrap().is_ok());
    drop(stalls);

    // A sender that keeps the connection open after the last byte: the download does not wait a minute for it
    let listener = gio::SocketListener::new();
    let port = listener.add_any_inet_port(None::<&glib::Object>).unwrap();
    let (cert, pin, bytes) = (ours.tls.clone(), Rc::new(theirs.der.clone()), content.clone());
    let sender = glib::spawn_future_local(async move {
        let (connection, _) = listener.accept_future().await.unwrap();
        let base: gio::IOStream = connection.upcast();
        let secured = net::secure_as_server(&base, &cert, Trust::Pin(pin)).await.unwrap();
        net::write_all(&secured.tls.output_stream(), bytes).await.unwrap();
        glib::timeout_future(Duration::from_secs(12)).await;
    });
    let started = std::time::Instant::now();
    let received = Rc::new(RefCell::new(Vec::new()));
    let sink = received.clone();
    let total = payload::fetch("127.0.0.1".parse().unwrap(), 0, port, &theirs.tls, Rc::new(ours.der.clone()), content.len() as u64, &mut |chunk| {
        sink.borrow_mut().extend_from_slice(chunk);
        Ok(())
    }, &|_| {})
    .await;
    assert_eq!(total.unwrap(), content.len() as u64);
    assert_eq!(*received.borrow(), content);
    assert!(started.elapsed() < Duration::from_secs(8), "a download that has its bytes does not wait for the sender to hang up: {:?}", started.elapsed());
    sender.abort();
}

// --- 2, 3 and 17 over a bus --------------------------------------------------------------------------------------

async fn call(client: &gio::DBusConnection, dest: &str, path: &str, interface: &str, method: &str, args: Option<glib::Variant>) -> Result<glib::Variant, glib::Error> {
    client.call_future(Some(dest), path, interface, method, args.as_ref(), None, gio::DBusCallFlags::NONE, 5000).await
}

const ROOT: &str = "/org/amethystora/Connect";
const IFACE: &str = "org.amethystora.Connect";
const DEVICE: &str = "org.amethystora.Connect.Device";

async fn property(client: &gio::DBusConnection, dest: &str, path: &str, interface: &str, name: &str) -> Result<glib::Variant, glib::Error> {
    let reply = call(client, dest, path, "org.freedesktop.DBus.Properties", "Get", Some((interface, name).to_variant())).await?;
    Ok(reply.child_value(0).as_variant().expect("a variant"))
}

fn denied(result: &Result<glib::Variant, glib::Error>) -> bool {
    result.as_ref().err().is_some_and(|e| e.message().contains("caller not allowed"))
}

async fn over_the_bus(root: &Path) {
    let Some(daemon) = Daemon::start() else {
        eprintln!("dbus-daemon is not here: the bus scenarios are skipped");
        return;
    };
    let me = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    let nowhere = "/nonexistent/program".to_string();
    let client = daemon.connect().await;

    // --- 2. Who may call
    // A caller that is neither the app nor the module
    let stranger = make_with(root, "Stranger test", "laptop", false, false, |c| {
        c.app_exe = nowhere.clone();
        c.module_exe = nowhere.clone();
    });
    let server = daemon.connect().await;
    dbus::export(&stranger.service, &server).unwrap();
    let dest = server.unique_name().unwrap().to_string();
    for method in ["GetDevices", "Announce"] {
        let result = call(&client, &dest, ROOT, IFACE, method, None).await;
        assert!(denied(&result), "{method}: {result:?}");
    }
    let result = call(&client, &dest, ROOT, IFACE, "PushClipboard", Some(("text",).to_variant())).await;
    assert!(denied(&result), "{result:?}");
    let result = call(&client, &dest, ROOT, IFACE, "ConnectTo", Some(("192.168.1.2:1716",).to_variant())).await;
    assert!(denied(&result), "{result:?}");
    // A stranger cannot take the links phones share away from the program's own notifications
    let result = call(&client, &dest, ROOT, IFACE, "TakeLinks", Some((true,).to_variant())).await;
    assert!(denied(&result), "{result:?}");
    assert!(!dbus::links_taken(&stranger.service));
    let name = property(&client, &dest, ROOT, IFACE, "Name").await.unwrap();
    assert_eq!(name.str(), Some(""), "a stranger reads nothing");
    assert_eq!(property(&client, &dest, ROOT, IFACE, "ClipboardSend").await.unwrap().get::<bool>(), Some(false));
    assert_eq!(property(&client, &dest, ROOT, IFACE, "PhotoFolder").await.unwrap().str(), Some(""), "nor where pictures go");

    // The app: everything
    let app = make_with(root, "App test", "laptop", false, false, |c| {
        c.app_exe = me.clone();
        c.module_exe = nowhere.clone();
    });
    let server = daemon.connect().await;
    dbus::export(&app.service, &server).unwrap();
    let dest = server.unique_name().unwrap().to_string();
    assert!(call(&client, &dest, ROOT, IFACE, "GetDevices", None).await.is_ok());
    assert!(call(&client, &dest, ROOT, IFACE, "Announce", None).await.is_ok());
    assert_eq!(property(&client, &dest, ROOT, IFACE, "Name").await.unwrap().str(), Some("App test"));
    let result = call(&client, &dest, ROOT, IFACE, "ConnectTo", Some(("8.8.8.8:1716",).to_variant())).await;
    assert!(result.is_err() && !denied(&result), "an address that is not home is refused for its own reason: {result:?}");

    // The module: the list, the clipboard, a ping and a link; the rest is the app's
    let module = make_with(root, "Module test", "laptop", false, false, |c| {
        c.app_exe = nowhere.clone();
        c.module_exe = me.clone();
    });
    let phone = make(root, "Sam phone", "phone", true, false);
    let server = daemon.connect().await;
    dbus::export(&module.service, &server).unwrap();
    let dest = server.unique_name().unwrap().to_string();
    module.service.connect_to(&format!("127.0.0.1:{}", phone.port())).unwrap();
    let pid = phone.id();
    until("the phone to be known", || module.service.device(&pid).is_some_and(|d| d.connected())).await;
    let listed = call(&client, &dest, ROOT, IFACE, "GetDevices", None).await.unwrap();
    let path = listed.child_value(0).child_value(0).str().unwrap().to_string();
    assert!(call(&client, &dest, ROOT, IFACE, "PushClipboard", Some(("text",).to_variant())).await.is_ok());
    // The module takes the links phones share, for its own connection, and lets go of them
    assert!(call(&client, &dest, ROOT, IFACE, "TakeLinks", Some((true,).to_variant())).await.is_ok());
    assert!(dbus::links_taken(&module.service), "the module's connection takes the links");
    assert!(call(&client, &dest, ROOT, IFACE, "TakeLinks", Some((false,).to_variant())).await.is_ok());
    assert!(!dbus::links_taken(&module.service), "and lets go of them");
    assert!(call(&client, &dest, ROOT, IFACE, "TakeLinks", Some(("yes",).to_variant())).await.is_err());
    assert_eq!(property(&client, &dest, ROOT, IFACE, "Name").await.unwrap().str(), Some("Module test"));
    assert_eq!(property(&client, &dest, &path, DEVICE, "Name").await.unwrap().str(), Some("Sam phone"));
    // The bar's capsule reads the phone's signal and where its pictures go
    assert_eq!(property(&client, &dest, &path, DEVICE, "SignalStrength").await.unwrap().get::<i32>(), Some(-1));
    assert_eq!(property(&client, &dest, &path, DEVICE, "NetworkType").await.unwrap().str(), Some(""));
    assert_eq!(property(&client, &dest, ROOT, IFACE, "PhotoFolder").await.unwrap().str(), module.dir.join("pictures/Phone").to_str());
    // MuteRinger: the live pill's chip for a phone ringing
    for method in ["Ping", "SendUrl", "MuteRinger"] {
        let args = (method == "SendUrl").then(|| ("https://example.org/",).to_variant());
        let result = call(&client, &dest, &path, DEVICE, method, args).await;
        assert!(result.is_err() && !denied(&result), "{method} is the module's, and fails here for want of a pairing: {result:?}");
    }
    for (path, interface, method, args) in [
        (ROOT, IFACE, "Announce", None),
        (ROOT, IFACE, "ConnectTo", Some(("127.0.0.1:1716",).to_variant())),
        (path.as_str(), DEVICE, "Pair", None),
        (path.as_str(), DEVICE, "AcceptPairing", None),
        (path.as_str(), DEVICE, "Unpair", None),
        (path.as_str(), DEVICE, "SendText", Some(("hello",).to_variant())),
        (path.as_str(), DEVICE, "SendFiles", Some((vec!["file:///etc/hostname".to_string()],).to_variant())),
        (path.as_str(), DEVICE, "GetRepliable", None),
        (path.as_str(), DEVICE, "ReplyToNotification", Some(("a", "b").to_variant())),
        (path.as_str(), DEVICE, "RequestConversations", None),
        (path.as_str(), DEVICE, "ListConversations", Some((10i32,).to_variant())),
        (path.as_str(), DEVICE, "ListMessages", Some((1i64, 10i32).to_variant())),
        (path.as_str(), DEVICE, "SendMessage", Some((vec!["+1".to_string()], "hi", 0i64).to_variant())),
    ] {
        let result = call(&client, &dest, path, interface, method, args).await;
        assert!(denied(&result), "{method} is the app's alone: {result:?}");
    }
    // 17. What the phone's clipboard holds goes to the module only: another program on the bus is not sent it
    module.service.set_clipboard_switches(true, true);
    phone.service.set_clipboard_switches(true, true);
    let log = root.join("monitor.log");
    let monitor = Command::new("timeout")
        .args(["8", "gdbus", "monitor", "--address", &daemon.address, "--dest", &dest])
        .stdout(std::fs::File::create(&log).unwrap())
        .stderr(Stdio::null())
        .spawn();
    let listening = monitor.is_ok();
    let heard = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = heard.clone();
    #[allow(deprecated)]
    let _subscription = client.signal_subscribe(Some(&dest), Some(IFACE), Some("ClipboardReceived"), Some(ROOT), None, gio::DBusSignalFlags::NONE, move |_, _, _, _, _, params| {
        sink.borrow_mut().push(params.child_value(1).str().unwrap_or("").to_string());
    });
    let heard_properties = Rc::new(std::cell::Cell::new(0));
    let count = heard_properties.clone();
    #[allow(deprecated)]
    let _properties = client.signal_subscribe(Some(&dest), Some("org.freedesktop.DBus.Properties"), Some("PropertiesChanged"), None, None, gio::DBusSignalFlags::NONE, move |_, _, _, _, _, _| {
        count.set(count.get() + 1);
    });
    pause(800).await;
    // The person pairs the two (the phone asks, and says yes here)
    phone.service.pair(&module.id()).unwrap();
    until("the prompt", || prompt_of(&module, &pid).is_some()).await;
    module.notices.press(&prompt_of(&module, &pid).unwrap().key, "accept");
    until("the pairing", || module.service.device(&pid).is_some_and(|d| d.paired()) && phone.service.device(&module.id()).is_some_and(|d| d.paired())).await;
    phone.service.push_clipboard("a password copied on the phone");
    until("the module to be sent it", || !heard.borrow().is_empty()).await;
    assert_eq!(heard.borrow()[0], "a password copied on the phone");
    pause(500).await;
    if let Ok(mut child) = monitor {
        let _ = child.kill();
        let _ = child.wait();
    }
    let seen = std::fs::read_to_string(&log).unwrap_or_default();
    // Nothing the phone link says is sent to a connection that did not call as the app or the module
    assert!(!seen.contains("a password copied on the phone"), "another program was sent the clipboard: {seen}");
    assert!(!seen.contains("Sam phone") && !seen.contains("PropertiesChanged") && !seen.contains("DeviceAdded"), "another program was told of the phones: {seen}");
    // And what it was sent to the module it did get
    assert!(heard_properties.get() > 0, "the module was sent the changes of the properties");
    let _ = listening;

    // --- 3. Connections: only from a network of the home zone, as firewalld says, and it is asked again when it changes
    let firewalld_conn = daemon.connect().await;
    let zones: Rc<RefCell<HashMap<String, String>>> = Rc::default();
    let table = zones.clone();
    let node = gio::DBusNodeInfo::for_xml(
        "<node><interface name='org.fedoraproject.FirewallD1.zone'><method name='getZoneOfInterface'><arg type='s' direction='in'/><arg type='s' direction='out'/></method></interface></node>",
    )
    .unwrap();
    firewalld_conn
        .register_object("/org/fedoraproject/FirewallD1", &node.lookup_interface("org.fedoraproject.FirewallD1.zone").unwrap())
        .method_call(move |_, _, _, _, _, params, invocation| {
            let interface = params.child_value(0).str().unwrap_or("").to_string();
            let zone = table.borrow().get(&interface).cloned().unwrap_or_default();
            invocation.return_value(Some(&(zone,).to_variant()));
        })
        .build()
        .unwrap();
    let owner = gio::bus_own_name_on_connection(&firewalld_conn, "org.fedoraproject.FirewallD1", gio::BusNameOwnerFlags::NONE, |_, _| {}, |_, _| {});
    pause(300).await;
    let fw_client = daemon.connect().await;
    let zoned = make_with(root, "Zoned", "laptop", false, false, |c| {
        c.home_interfaces = None;
        c.firewall_bus = Some(fw_client.clone());
    });
    let port = zoned.port();
    pause(500).await;
    // Not a home network (or no answer from the firewall): the connection is dropped before a handshake
    zones.borrow_mut().insert("lo".into(), "public".into());
    assert!(raw_phone(port, X, "Sam phone", None).await.is_err(), "an interface in no home zone is not talked to");
    assert!(zoned.service.devices.borrow().is_empty());
    // The firewall's zones change and it says so
    zones.borrow_mut().insert("lo".into(), "amethystora-home".into());
    firewalld_conn.emit_signal(None, "/org/fedoraproject/FirewallD1", "org.fedoraproject.FirewallD1", "Reloaded", None).unwrap();
    until("the zones to be asked again", || zoned.service.home.contains("lo")).await;
    assert!(raw_phone(port, X, "Sam phone", None).await.is_ok(), "now it is home");
    zones.borrow_mut().insert("lo".into(), "trusted".into());
    firewalld_conn.emit_signal(None, "/org/fedoraproject/FirewallD1", "org.fedoraproject.FirewallD1.zone", "ZoneOfInterfaceChanged", Some(&("trusted", "lo").to_variant())).unwrap();
    until("lo to leave the home zone", || !zoned.service.home.contains("lo")).await;
    assert!(raw_phone(port, &"c".repeat(32), "Sam phone", None).await.is_err());
    // A network is named home with no word of it to anyone but firewalld: the next connection finds it out
    zones.borrow_mut().insert("lo".into(), "amethystora-home".into());
    pause(2300).await;
    assert!(raw_phone(port, &"d".repeat(32), "Sam phone", None).await.is_ok(), "the zones are asked again when an address is not known to be home");
    // Before the connection goes: a name lost with no connection is more than the bindings take
    gio::bus_unown_name(owner);
}

#[test]
fn who_may_call_what() {
    use dbus::{Role, method_allowed, role_of_exe};
    assert_eq!(role_of_exe(dbus::APP_EXE, dbus::APP_EXE, dbus::MODULE_EXE), Role::App);
    assert_eq!(role_of_exe(dbus::MODULE_EXE, dbus::APP_EXE, dbus::MODULE_EXE), Role::Module);
    for other in ["/usr/bin/gdbus", "/usr/bin/gnome-shell (deleted)", "/home/ada/.local/bin/amethystora-phone", "", "/usr/lib/amethystora-phone/amethystora-phone/../x"] {
        assert_eq!(role_of_exe(other, dbus::APP_EXE, dbus::MODULE_EXE), Role::Other, "{other:?}");
    }
    for method in ["Pair", "AcceptPairing", "RejectPairing", "Unpair", "SendFiles", "SendText", "SendMessage", "ReplyToNotification", "GetRepliable", "ListMessages", "ListConversations", "Announce", "ConnectTo", "CancelTransfer"] {
        assert!(method_allowed(Role::App, method));
        assert!(!method_allowed(Role::Module, method), "{method}");
        assert!(!method_allowed(Role::Other, method));
    }
    // MuteRinger: the live pill's chip for a phone ringing
    // TakeLinks: the live pill says it shows the links phones share
    for method in ["GetDevices", "PushClipboard", "Ping", "SendUrl", "MuteRinger", "TakeLinks"] {
        assert!(method_allowed(Role::App, method) && method_allowed(Role::Module, method));
        assert!(!method_allowed(Role::Other, method));
    }
}

#[test]
fn the_module_hears_files_going_out_only() {
    use dbus::{Role, audience};
    let transfer = |incoming: bool| ("t1", "photo.jpg", incoming, 10i64, 100i64, "progress").to_variant();
    // The dock shows how far a drop on the phone got; what a phone sends in stays the app's
    assert!(audience(Role::Module, "Transfer", Some(&transfer(false))));
    assert!(!audience(Role::Module, "Transfer", Some(&transfer(true))));
    assert!(!audience(Role::Module, "Transfer", None));
    assert!(audience(Role::App, "Transfer", Some(&transfer(true))) && audience(Role::App, "Transfer", Some(&transfer(false))));
    assert!(!audience(Role::Other, "Transfer", Some(&transfer(false))));
    for signal in ["DeviceAdded", "DeviceRemoved", "PropertiesChanged", "ClipboardReceived"] {
        assert!(audience(Role::Module, signal, None), "{signal}");
    }
    for signal in ["PairingRequested", "NotificationPosted", "MessagesChanged"] {
        assert!(!audience(Role::Module, signal, None), "{signal}");
    }
    assert!(!audience(Role::App, "ClipboardReceived", None));
}

#[test]
fn the_live_pill_hears_calls_and_links() {
    use dbus::{Role, audience};
    let call = ("ringing", "+15550100", "Sam").to_variant();
    let url = ("phone1", "https://example.org/").to_variant();
    // A phone ringing is the app's and the module's; a shared link is the module's alone, and nobody else hears either
    assert!(audience(Role::Module, "Call", Some(&call)) && audience(Role::App, "Call", Some(&call)));
    assert!(!audience(Role::Other, "Call", Some(&call)));
    assert!(audience(Role::Module, "UrlReceived", Some(&url)));
    assert!(!audience(Role::App, "UrlReceived", Some(&url)));
    assert!(!audience(Role::Other, "UrlReceived", Some(&url)));
}

#[test]
fn a_code_reaches_the_module_alone_and_the_texts_never_do() {
    use dbus::{Role, audience, method_allowed};
    let code = ("482913",).to_variant();
    // The digits of a one-time code are the module's alone; no one else hears them
    assert!(audience(Role::Module, "Code", Some(&code)));
    assert!(!audience(Role::App, "Code", Some(&code)));
    assert!(!audience(Role::Other, "Code", Some(&code)));
    // And the shell still reads no text: neither the call nor the signal that a text came
    assert!(!method_allowed(Role::Module, "ListMessages") && !method_allowed(Role::Module, "ListConversations"));
    assert!(!audience(Role::Module, "MessagesChanged", None) && !audience(Role::Module, "ConversationsChanged", None));
}

// --- A connection made under a phone's id before it was paired holds no pairing --------------------------------

async fn a_connection_that_is_not_the_pin_is_closed_when_the_phone_is_paired(root: &Path) {
    let computer = make(root, "Pin holder", "laptop", false, false);
    let port = computer.port();

    // Someone who read the phone's id from its broadcast connects first, with a certificate of its own, and says nothing
    let mut attacker = raw_phone(port, X, "Sam phone", None).await.unwrap();
    until("the attacker's connection", || computer.service.device(X).is_some_and(|d| d.connected())).await;
    // The phone connects, asks, and the person accepts
    let honest = raw_phone(port, X, "Sam phone", None).await.unwrap();
    send(&honest, pair_request(0)).await;
    until("the prompt", || prompt_of(&computer, X).is_some()).await;
    assert!(computer.device(X).st.borrow().links.iter().any(|l| l.peer_der == attacker.der), "until then it is one of the device's connections");
    computer.notices.press(&prompt_of(&computer, X).unwrap().key, "accept");
    until("the pairing", || computer.device(X).paired()).await;

    // Paired: the pin is the phone's, and the other connection is closed, not left to speak for it
    assert_eq!(computer.service.store.borrow().paired.get(X).map(|p| p.pin.clone()).as_deref(), Some(&honest.der[..]));
    let answer = net::timed(Duration::from_secs(3), attacker.reader.next(65536)).await;
    assert!(!matches!(answer, Err(net::Error::Timeout)) && !matches!(answer, Ok(Some(_))), "the attacker's connection was closed: {answer:?}");
    until("only the pinned connection to remain", || computer.device(X).st.borrow().links.iter().all(|l| !l.is_closed() && l.peer_der == honest.der)).await;
    // What it sends now is taken by nothing
    send(&attacker, Packet::new("kdeconnect.ping", json!({ "message": "from the attacker" }))).await;
    pause(300).await;
    assert!(computer.saw("kdeconnect.ping").iter().all(|p| p.body["message"] != "from the attacker"));

    // And a connection whose certificate is not the pin of a paired device is closed at its first packet, whatever got it there
    computer.service.store.borrow_mut().paired.get_mut(X).unwrap().pin = vec![1, 2, 3];
    send(&honest, Packet::new("kdeconnect.ping", json!({ "message": "after the pin changed" }))).await;
    until("the connection to be closed", || !computer.device(X).connected()).await;
    assert!(computer.saw("kdeconnect.ping").iter().all(|p| p.body["message"] != "after the pin changed"));
}

// --- One neighbour cannot hold every slot of the unpaired ones ----------------------------------------------------

async fn one_address_cannot_hold_the_unpaired_slots(root: &Path) {
    let computer = make(root, "Slots", "laptop", false, false);
    let port = computer.port();
    let up = |id: &String| computer.service.device(id).is_some_and(|d| d.connected());
    let all = ids(6);

    // Five silent devices from one address: each makes room for the next by closing the oldest, so the newest are held,
    // and nothing is closed for being silent while there is room
    let mut kept = Vec::new();
    for id in &all[..5] {
        kept.push(raw_phone(port, id, "Pixel 8", None).await.unwrap());

        pause(150).await;
    }
    let held: Vec<bool> = all[..5].iter().map(&up).collect();
    assert_eq!(held, vec![false, false, false, true, true], "the oldest are let go, the newest {} kept", limits::UNPAIRED_PER_SOURCE);
    // Another address is not shut out by it
    let other = "7".repeat(32);
    let _elsewhere = raw_phone(port, &other, "Pixel 9", Some("127.0.0.9")).await.unwrap();
    until("the other address's phone", || up(&other)).await;
    pause(300).await;
    assert!(up(&all[3]) && up(&all[4]), "and what it takes costs this address nothing");

    // A device whose pairing waits is not the one to go: with two asking, a third from the same address is the one let go
    send(&kept[3], pair_request(0)).await;
    send(&kept[4], pair_request(1)).await;
    until("both prompts", || prompt_of(&computer, &all[3]).is_some() && prompt_of(&computer, &all[4]).is_some()).await;
    let third = raw_phone(port, &all[5], "Pixel 8", None).await.unwrap();
    pause(500).await;
    assert!(!up(&all[5]), "no silent connection to close: the new one is refused");
    assert!(up(&all[3]) && up(&all[4]), "the pairings that wait are kept");
    drop(third);

    // The same across addresses: nine devices from nine addresses, never more than eight held, the oldest let go
    let computer = make(root, "Slots many", "laptop", false, false);
    let port = computer.port();
    let mut kept = Vec::new();
    let many: Vec<String> = (0..9).map(|n| format!("{:0>32}", format!("c{n}"))).collect();
    for (n, id) in many.iter().enumerate() {
        kept.push(raw_phone(port, id, "Pixel 8", Some(&format!("127.0.1.{}", 1 + n))).await.unwrap());
        pause(150).await;
    }
    let held = many.iter().filter(|id| computer.service.device(id).is_some_and(|d| d.connected())).count();
    assert_eq!(held, limits::UNPAIRED_DEVICES);
    assert!(!computer.service.device(&many[0]).is_some_and(|d| d.connected()), "the oldest made room for the newest");
    assert!(computer.service.device(&many[8]).is_some_and(|d| d.connected()));
}
