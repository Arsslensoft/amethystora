//! Two copies of the program in one process, talking over real sockets and real TLS on the loopback: one is the
//! computer, the other plays a phone (it announces the packet types a phone does). The transcript is the
//! spec's: connect, pair with the key compared, ping, battery, a phone's notification and the reply to it, files
//! each way (pictures to Pictures/Phone), the phone's signal, the clipboard, a call, texts, unpairing; then the negative cases, with a hostile peer made from the
//! crate's own parts. Everything is one test, because the main context of a thread has one owner.

use std::path::Path;
use std::time::Duration;

use amethystora_connect::net::{self, LineReader, Trust};
use amethystora_connect::notify::Notice;
use amethystora_connect::packet::{self, Packet};
use amethystora_connect::{identity, payload, x509};
use gio::prelude::*;
use serde_json::json;

mod common;

use common::*;

#[test]
fn the_whole_transcript() {
    let root = std::env::temp_dir().join(format!("amethystora-connect-loopback-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    glib::MainContext::default().block_on(scenario(&root));
    let _ = std::fs::remove_dir_all(&root);
}

async fn scenario(root: &Path) {
    let computer = make(root, "Studio laptop", "laptop", false, true);
    let phone = make(root, "Sam phone", "phone", true, true);
    let (cid, pid) = (computer.id(), phone.id());
    assert_ne!(cid, pid);

    // --- Finding each other: a datagram from the phone makes the computer call it ---------------------------
    let udp = computer.service.udp_port().expect("the computer listens for datagrams");
    phone.service.announce_to_port("127.0.0.1".parse().unwrap(), 0, udp);
    until("the computer to call the phone", || computer.service.device(&pid).is_some_and(|d| d.connected()) && phone.service.device(&cid).is_some_and(|d| d.connected())).await;
    let seen_by_computer = computer.device(&pid);
    let seen_by_phone = phone.device(&cid);
    assert!(!seen_by_computer.paired() && !seen_by_phone.paired());
    assert_eq!(prop::<String>(&seen_by_computer, "Name"), "Sam phone");
    assert_eq!(prop::<String>(&seen_by_computer, "Type"), "phone");
    assert_eq!(prop::<Vec<String>>(&seen_by_computer, "Features"), Vec::<String>::new(), "nothing is offered before a pairing");
    assert!(computer.service.recorded.borrow().iter().any(|s| s.name == "DeviceAdded"));

    // A datagram from a stranger's address, a made-up one, or about ourselves, is nothing
    computer.service.announce_to_port("127.0.0.1".parse().unwrap(), 0, udp);
    pause(200).await;
    assert_eq!(computer.service.devices.borrow().len(), 1);

    // --- Nothing but pairing before a pairing --------------------------------------------------------------
    seen_by_phone.link().unwrap().send(&Packet::new("kdeconnect.ping", json!({ "message": "hello?" })));
    until("the unpaired answer", || !computer.saw("kdeconnect.ping").is_empty()).await;
    until("the computer to say it is not paired", || phone.saw(packet::PAIR).iter().any(|p| p.bool("pair") == Some(false))).await;
    assert!(computer.notices.last(&format!("ping:{pid}")).is_none(), "a packet of an unpaired device does nothing");
    seen_by_phone.link().unwrap().send(&Packet::new("kdeconnect.ping", json!({})));
    pause(150).await;
    assert_eq!(phone.saw(packet::PAIR).len(), 1, "told once on a connection");

    // --- Pairing, the computer asking ---------------------------------------------------------------------
    computer.service.pair(&pid).unwrap();
    until("the phone's prompt", || prop::<String>(&seen_by_phone, "Pairing") == "incoming").await;
    assert_eq!(prop::<String>(&seen_by_computer, "Pairing"), "outgoing");
    let key_here = prop::<String>(&seen_by_computer, "PairingKey");
    let key_there = prop::<String>(&seen_by_phone, "PairingKey");
    assert_eq!(key_here.len(), 8);
    assert_eq!(key_here, key_there, "both screens show the same key");
    assert!(key_here.chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)));
    assert!(prop::<i64>(&seen_by_phone, "PairingDeadline") > packet::now_secs());
    let prompt = prompt_of(&phone, &cid).expect("the person is asked");
    assert!(!prompt.urgent, "an ordinary notification, not a critical one");
    assert!(prompt.body.contains(&key_there[..4]));
    assert_eq!(prompt.actions.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), ["accept", "reject"]);
    assert!(prompt_of(&computer, &pid).is_some_and(|n| n.actions[0].0 == "cancel"));
    assert!(!store_has(&phone, &cid), "nothing is kept before the person says yes");

    // The person presses Accept on the phone's notification
    phone.notices.press(&prompt.key, "accept");
    until("the pairing", || seen_by_computer.paired() && seen_by_phone.paired()).await;
    assert!(store_has(&computer, &pid) && store_has(&phone, &cid));
    assert!(phone.notices.closed.borrow().contains(&prompt.key), "the prompt goes");
    assert_eq!(prop::<String>(&seen_by_computer, "PairingKey"), "");
    assert_eq!(prop::<i64>(&seen_by_computer, "PairingDeadline"), 0);
    {
        use std::os::unix::fs::PermissionsExt;
        let file = computer.dir.join("config/devices.json");
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(computer.dir.join("config")).unwrap().permissions().mode() & 0o777, 0o700);
    }
    until("the connect-time requests", || !phone.saw("kdeconnect.battery.request").is_empty() && !phone.saw("kdeconnect.sms.request_conversations").is_empty()).await;
    assert!(!phone.saw("kdeconnect.notification.request").is_empty());
    assert!(!phone.saw("kdeconnect.contacts.request_all_uids_timestamps").is_empty());
    assert_eq!(prop::<Vec<String>>(&seen_by_computer, "Features"), ["ping", "battery", "notifications", "share", "clipboard", "sms", "telephony", "connectivity"]);

    // --- Ping ---------------------------------------------------------------------------------------------
    computer.service.ping(&pid).unwrap();
    until("the ping", || !phone.saw("kdeconnect.ping").is_empty()).await;
    seen_by_phone.link().unwrap().send(&Packet::new("kdeconnect.ping", json!({ "message": "can you see this?" })));
    until("the ping notification", || computer.notices.last(&format!("ping:{pid}")).is_some()).await;
    assert_eq!(computer.notices.last(&format!("ping:{pid}")).unwrap().body, "can you see this?");

    // --- Battery ------------------------------------------------------------------------------------------
    phone.service.send_to(&seen_by_phone, "kdeconnect.battery", json!({ "currentCharge": 62, "isCharging": true, "thresholdEvent": 0 }));
    until("the battery", || prop::<i32>(&seen_by_computer, "BatteryLevel") == 62).await;
    assert!(prop::<bool>(&seen_by_computer, "BatteryCharging"));
    phone.service.send_to(&seen_by_phone, "kdeconnect.battery", json!({ "currentCharge": 14, "isCharging": false, "thresholdEvent": 1 }));
    until("the low battery alert", || computer.notices.last(&format!("battery:{pid}")).is_some()).await;
    assert!(computer.notices.last(&format!("battery:{pid}")).unwrap().body.contains("14"));

    // --- The phone's signal (spec-phone-files-first.md 2.1, V-CR-1 to 9) -------------------------------------
    connectivity(&computer, &phone, &pid).await;

    // --- A phone's notification, its reply, its buttons, its dismissal ------------------------------------
    let phone_id = "0|org.example.chat|55|null|10123";
    let body = json!({ "id": phone_id, "appName": "Example Chat", "title": "Sam <b>", "text": "Are you coming tonight? Code 123456.", "isClearable": true, "silent": false,
        "requestReplyId": "5b1c2f9e-3d44-4a0a-9c53-0a5d0f6a7e11", "actions": ["Like"] });
    phone.service.send_to(&seen_by_phone, "kdeconnect.notification", body.clone());
    let key = format!("n:{pid}:{phone_id}");
    until("the phone's notification", || computer.notices.last(&key).is_some()).await;
    let shown: Notice = computer.notices.last(&key).unwrap();
    assert_eq!(shown.summary, "Example Chat");
    assert_eq!(shown.body, "Sam <b>: Are you coming tonight? Code 123456.", "the notification is text; the server's markup is its escaping to do");
    let ids: Vec<&str> = shown.actions.iter().map(|a| a.0.as_str()).collect();
    assert_eq!(ids, ["default", "reply", "action:0", "dismiss"]);
    assert_eq!(shown.sound, None, "a chat app's notification has the server's own sound");
    let before = computer.notices.raised.borrow().len();
    phone.service.send_to(&seen_by_phone, "kdeconnect.notification", body.clone());
    pause(150).await;
    assert_eq!(computer.notices.raised.borrow().len(), before, "the same notification again does not ring twice");
    let repliable = computer.service.repliable(&pid).unwrap();
    assert_eq!(repliable.len(), 1);
    assert!(!repliable[0].contains_key("icon"), "no icon was sent");
    let our_id = repliable[0]["id"].get::<String>().unwrap();
    computer.service.reply_to_notification(&pid, &our_id, "On my way.").unwrap();
    until("the reply", || !phone.saw("kdeconnect.notification.reply").is_empty()).await;
    let reply = &phone.saw("kdeconnect.notification.reply")[0];
    assert_eq!(reply.str("requestReplyId"), Some("5b1c2f9e-3d44-4a0a-9c53-0a5d0f6a7e11"));
    assert_eq!(reply.str("message"), Some("On my way."));
    assert!(computer.service.reply_to_notification(&pid, &our_id, "  ").is_err());
    computer.notices.press(&key, "action:0");
    until("the button", || !phone.saw("kdeconnect.notification.action").is_empty()).await;
    assert_eq!(phone.saw("kdeconnect.notification.action")[0].str("key"), Some(phone_id));
    assert_eq!(phone.saw("kdeconnect.notification.action")[0].str("action"), Some("Like"));
    computer.notices.press(&key, "reply");
    until("the app to be started for the reply", || std::fs::read_to_string(computer.dir.join("app.log")).is_ok_and(|l| l.contains("reply"))).await;
    assert!(std::fs::read_to_string(computer.dir.join("app.log")).unwrap().contains(&format!("reply -- {pid} {phone_id}")));
    // Closing it here closes it on the phone
    computer.notices.dismiss(&key);
    until("the dismissal", || !phone.saw("kdeconnect.notification.request").iter().all(|p| p.str("cancel").is_none())).await;
    assert!(phone.saw("kdeconnect.notification.request").iter().any(|p| p.str("cancel") == Some(phone_id)));
    // And the phone closing it closes it here
    let other = json!({ "id": "other", "appName": "Mail", "title": "Hello", "text": "World", "isClearable": true });
    phone.service.send_to(&seen_by_phone, "kdeconnect.notification", other);
    until("the second notification", || computer.notices.last(&format!("n:{pid}:other")).is_some()).await;
    phone.service.send_to(&seen_by_phone, "kdeconnect.notification", json!({ "id": "other", "isCancel": true }));
    until("the phone's cancel", || computer.notices.closed.borrow().contains(&format!("n:{pid}:other"))).await;
    assert_eq!(computer.notices.last(&format!("n:{pid}:other")).unwrap().sound, None);
    // A text message rings with the phone's own sound in the desktop's sound theme
    let text = "0|com.google.android.apps.messaging|3|null|10123";
    phone.service.send_to(&seen_by_phone, "kdeconnect.notification", json!({ "id": text, "appName": "Messages", "title": "Sam", "text": "Running late", "isClearable": true }));
    until("the text message's notification", || computer.notices.last(&format!("n:{pid}:{text}")).is_some()).await;
    assert_eq!(computer.notices.last(&format!("n:{pid}:{text}")).unwrap().sound, Some("message-new-instant-phone"));
    // A notification whose icon the program already keeps: the reply dialog is given the path of that PNG
    {
        let hash = "0123456789abcdef0123456789abcdef";
        let kept = computer.service.icons_dir(&pid).join(format!("{hash}.png"));
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::write(&kept, b"\x89PNG\r\n\x1a\n").unwrap();
        phone.service.send_to(&seen_by_phone, "kdeconnect.notification", json!({ "id": "with-icon", "appName": "Chat", "title": "Lin", "text": "Hi",
            "isClearable": true, "requestReplyId": "6b1c2f9e-3d44-4a0a-9c53-0a5d0f6a7e12", "payloadHash": hash }));
        until("the notification with an icon", || computer.notices.last(&format!("n:{pid}:with-icon")).is_some()).await;
        let listed = computer.service.repliable(&pid).unwrap();
        let entry = listed.iter().find(|n| n["title"].get::<String>().as_deref() == Some("Lin")).expect("the notification");
        assert_eq!(entry["icon"].get::<String>().as_deref(), kept.to_str());
        phone.service.send_to(&seen_by_phone, "kdeconnect.notification", json!({ "id": "with-icon", "isCancel": true }));
        until("the icon's notification to go", || computer.notices.closed.borrow().contains(&format!("n:{pid}:with-icon"))).await;
    }
    // Silent catch-up notifications and ongoing ones are not shown
    phone.service.send_to(&seen_by_phone, "kdeconnect.notification", json!({ "id": "quiet", "appName": "Mail", "title": "Old", "text": "News", "silent": true }));
    phone.service.send_to(&seen_by_phone, "kdeconnect.notification", json!({ "id": "ongoing", "appName": "Player", "title": "Now playing", "isClearable": false }));
    pause(200).await;
    assert!(computer.notices.last(&format!("n:{pid}:quiet")).is_none() && computer.notices.last(&format!("n:{pid}:ongoing")).is_none());

    // --- Files, the computer to the phone -----------------------------------------------------------------
    let content = pattern(3 * 1024 * 1024 + 123);
    let source = root.join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("holiday photo.jpg"), &content).unwrap();
    std::fs::write(source.join("notes.txt"), b"hello").unwrap();
    let ids = computer.service.send_files(&pid, &[file_uri(&source.join("holiday photo.jpg"))]).unwrap();
    until("the file to arrive", || state_of(&computer, "finished", &ids[0]) && !phone.notices.raised_with_prefix(&format!("got:{cid}")).is_empty()).await;
    // A picture goes to Pictures/Phone, with no folder of the sender's
    let landed = phone.pictures().join("Phone/holiday photo.jpg");
    assert_eq!(std::fs::read(&landed).unwrap(), content);
    assert!(signals(&phone, "Transfer").iter().any(|a| a.child_value(5).str() == Some("finished") && a.child_value(2).get::<bool>() == Some(true)));
    assert!(find_in(&phone.pictures().join("Phone"), ".part-").is_empty(), "no partial file is left");
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&landed).unwrap().permissions().mode() & 0o777, 0o600, "never executable");
        assert_eq!(std::fs::metadata(phone.pictures().join("Phone")).unwrap().permissions().mode() & 0o777, 0o700);
    }
    assert!(!phone.downloads().join("Phone/Studio laptop/holiday photo.jpg").exists());
    let got = phone.notices.raised_with_prefix(&format!("got:{cid}")).pop().unwrap();
    assert_eq!(got.actions.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), ["open", "show"]);
    // The same file again is a second file, not an overwrite
    let again = computer.service.send_files(&pid, &[file_uri(&source.join("holiday photo.jpg"))]).unwrap();
    until("the second copy", || state_of(&computer, "finished", &again[0])).await;
    until("the second file", || phone.pictures().join("Phone/holiday photo (1).jpg").exists()).await;
    assert_eq!(std::fs::read(&landed).unwrap(), content);
    // Several files are one batch: the announcements come one after the other, in order
    let both = computer.service.send_files(&pid, &[file_uri(&source.join("notes.txt")), file_uri(&source.join("holiday photo.jpg"))]).unwrap();
    until("the batch", || both.iter().all(|id| state_of(&computer, "finished", id))).await;
    assert!(!phone.saw("kdeconnect.share.request.update").is_empty());
    until("the batch notice", || !phone.notices.raised_with_prefix(&format!("batch:{cid}")).is_empty()).await;
    assert!(phone.downloads().join("Phone/Studio laptop/notes.txt").exists(), "what is not a picture goes where it went");
    // What cannot be sent is refused at once
    assert!(computer.service.send_files(&pid, &[file_uri(&source)]).is_err(), "a folder");
    assert!(computer.service.send_files(&pid, &["https://example.org/x".into()]).is_err());
    assert!(computer.service.send_files(&pid, &["file:///nonexistent/file".into()]).is_err());
    // A transfer can be cancelled, and leaves nothing behind
    std::fs::write(source.join("big.bin"), vec![7u8; 40 * 1024 * 1024]).unwrap();
    let big = computer.service.send_files(&pid, &[file_uri(&source.join("big.bin"))]).unwrap();
    until("the big file to start", || state_of(&computer, "started", &big[0])).await;
    computer.service.cancel_transfer(&pid, &big[0]).unwrap();
    until("the cancel", || state_of(&computer, "cancelled", &big[0])).await;
    until("the partial file to go", || find_in(&phone.downloads().join("Phone/Studio laptop"), ".part-").is_empty()).await;
    pause(300).await;
    assert!(!phone.downloads().join("Phone/Studio laptop/big.bin").exists());

    // --- Files, the phone to the computer ------------------------------------------------------------------
    let from_phone = pattern(1024 * 1024 + 7);
    std::fs::write(source.join("from phone.png"), &from_phone).unwrap();
    let pids = phone.service.send_files(&cid, &[file_uri(&source.join("from phone.png"))]).unwrap();
    until("the file from the phone", || state_of(&phone, "finished", &pids[0]) && computer.pictures().join("Phone/from phone.png").exists()).await;
    assert_eq!(std::fs::read(computer.pictures().join("Phone/from phone.png")).unwrap(), from_phone);
    photo_folder(&computer, &phone, &cid, &source).await;

    // A hostile name stays in its folder: a phone that announces "..\..\.config/autostart/evil.sh"
    {
        let offer = payload::offer(&(0..=0)).unwrap();
        let port = offer.port;
        let link = seen_by_phone.link().unwrap();
        let pin = phone.service.pin_of(&cid).unwrap();
        let cert = phone.service.me.tls.clone();
        let evil = b"#!/bin/sh\necho owned\n".to_vec();
        let size = evil.len() as u64;
        let mut at = 0;
        let task = glib::spawn_future_local(async move {
            let _ = offer
                .serve(&cert, pin, size, &mut |buf| {
                    let n = buf.len().min(evil.len() - at);
                    buf[..n].copy_from_slice(&evil[at..at + n]);
                    at += n;
                    Ok(n)
                }, &|_| {})
                .await;
        });
        link.send(&Packet::new("kdeconnect.share.request", json!({ "filename": "..\\..\\.config/autostart/evil.sh" })).with_payload(size, port));
        until("the hostile file", || computer.downloads().join("Phone/Sam phone/evil.sh").exists()).await;
        task.await.ok();
        assert!(!computer.dir.join(".config").exists() && !computer.downloads().join(".config").exists());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(computer.downloads().join("Phone/Sam phone/evil.sh")).unwrap().permissions().mode() & 0o777, 0o600);
    }
    // A phone announcing more than the space there is gets refused (the connection is made and closed)
    {
        let offer = payload::offer(&(0..=0)).unwrap();
        let port = offer.port;
        let link = seen_by_phone.link().unwrap();
        link.send(&Packet::new("kdeconnect.share.request", json!({ "filename": "huge.bin" })).with_payload(u64::MAX / 4, port));
        let pin = phone.service.pin_of(&cid).unwrap();
        let cert = phone.service.me.tls.clone();
        // The computer connects and goes away at once; what the phone then writes is lost, which is all a refusal is
        let _ = offer.serve(&cert, pin, 1024, &mut |buf| Ok(buf.len().min(1024)), &|_| {}).await;
        until("the refusal", || computer.notices.raised_with_prefix(&format!("fail:{pid}")).iter().any(|n| n.body.contains("huge.bin"))).await;
        assert!(!computer.downloads().join("Phone/Sam phone/huge.bin").exists());
    }

    // --- Links and text ------------------------------------------------------------------------------------
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "url": "https://example.org/page" }));
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "text": "Meet at the <north> gate at 9." }));
    until("the two notices", || computer.notices.raised_with_prefix(&format!("text:{pid}")).len() == 2).await;
    let notices = computer.notices.raised_with_prefix(&format!("text:{pid}"));
    assert_eq!(notices[0].actions.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), ["open", "show"]);
    assert_eq!(notices[1].actions.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), ["show"]);
    assert_eq!(notices[1].body, "Meet at the <north> gate at 9.");
    computer.notices.press(&notices[1].key, "show");
    until("the text window", || std::fs::read_to_string(computer.dir.join("app.log")).is_ok_and(|l| l.contains("text")) ).await;
    assert!(std::fs::read_to_string(computer.dir.join("app.log")).unwrap().contains(&format!("text -- {pid} -- Meet at the <north> gate at 9.")));
    computer.service.send_url(&pid, "https://example.org/").unwrap();
    until("the link", || phone.saw("kdeconnect.share.request").iter().any(|p| p.str("url") == Some("https://example.org/"))).await;
    assert!(computer.service.send_url(&pid, "not a link").is_err());
    assert!(computer.service.send_url(&pid, "javascript:\nalert(1)").is_err());
    computer.service.send_text(&pid, "some text").unwrap();
    until("the text", || phone.saw("kdeconnect.share.request").iter().any(|p| p.str("text") == Some("some text"))).await;
    // A web link goes to the shell's module only while a connection of the module's that is a client has taken the
    // links (TakeLinks): registered but not taking (the screen locked, the extension off), the program says it itself
    use amethystora_connect::dbus::{self as bus, Role};
    let module = ":1.9999".to_string();
    let notices = |n: usize| computer.notices.raised_with_prefix(&format!("text:{pid}")).len() == n;
    let said = computer.notices.raised_with_prefix(&format!("text:{pid}")).len();
    computer.service.clients.borrow_mut().insert(module.clone(), Role::Module);
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "url": "https://example.org/locked" }));
    until("the program's own notice while the module takes nothing", || notices(said + 1)).await;
    assert!(signals(&computer, "UrlReceived").is_empty(), "nothing is sent to a module that has not taken the links");
    // Taking them, for its own connection only: an app's connection, or one that is not a client, takes nothing
    bus::set_link_taker(&computer.service, Some(":1.42"), Role::App, true);
    assert!(!bus::links_taken(&computer.service));
    bus::set_link_taker(&computer.service, Some(&module), Role::Module, true);
    assert!(bus::links_taken(&computer.service));
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "url": "https://example.org/pill" }));
    until("the link for the pill", || signals(&computer, "UrlReceived").len() == 1).await;
    let link = signals(&computer, "UrlReceived").remove(0);
    assert_eq!(link.child_value(0).str(), Some(pid.as_str()));
    assert_eq!(link.child_value(1).str(), Some("https://example.org/pill"));
    assert!(notices(said + 1), "no notice of the program's own for a link the pill shows");
    // What the module would not take (no host, a space, not a web address), and text, are the program's to say
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "url": "https://example.org/a b" }));
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "url": "javascript:alert(1)" }));
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "text": "still a notification" }));
    until("three notices", || notices(said + 4)).await;
    assert_eq!(signals(&computer, "UrlReceived").len(), 1, "only a link the module takes goes to the pill");
    // The module lets go (it stops), or its connection goes: the program says links itself again
    bus::set_link_taker(&computer.service, Some(&module), Role::Module, false);
    assert!(!bus::links_taken(&computer.service));
    bus::set_link_taker(&computer.service, Some(&module), Role::Module, true);
    computer.service.clients.borrow_mut().remove(&module);
    assert!(!bus::links_taken(&computer.service), "a connection that is no client takes nothing");
    phone.service.send_to(&seen_by_phone, "kdeconnect.share.request", json!({ "url": "https://example.org/after" }));
    until("the program's own notice again", || notices(said + 5)).await;
    assert_eq!(signals(&computer, "UrlReceived").len(), 1);
    computer.service.link_takers.borrow_mut().clear();

    // --- The clipboard: two switches, off as shipped --------------------------------------------------------
    computer.service.push_clipboard("not sent: the switch is off");
    phone.service.send_to(&seen_by_phone, "kdeconnect.clipboard", json!({ "content": "not applied: the switch is off" }));
    pause(250).await;
    assert!(phone.saw("kdeconnect.clipboard").is_empty());
    assert!(signals(&computer, "ClipboardReceived").is_empty());
    computer.service.set_clipboard_switches(true, true);
    phone.service.set_clipboard_switches(true, true);
    computer.service.push_clipboard("copied on the computer");
    until("the clipboard to reach the phone", || signals(&phone, "ClipboardReceived").len() == 1).await;
    assert_eq!(signals(&phone, "ClipboardReceived")[0].child_value(1).str(), Some("copied on the computer"));
    // The module sets the phone's clipboard and says so: nothing goes back
    let before = computer.saw("kdeconnect.clipboard").len();
    phone.service.push_clipboard("copied on the computer");
    pause(250).await;
    assert_eq!(computer.saw("kdeconnect.clipboard").len(), before, "no echo");
    phone.service.send_to(&seen_by_phone, "kdeconnect.clipboard", json!({ "content": "copied on the phone" }));
    until("the phone's clipboard", || signals(&computer, "ClipboardReceived").len() == 1).await;
    // At connection, an older text does not replace a newer one
    phone.service.send_to(&seen_by_phone, "kdeconnect.clipboard.connect", json!({ "content": "very old", "timestamp": 1000 }));
    pause(250).await;
    assert_eq!(signals(&computer, "ClipboardReceived").len(), 1);
    computer.service.set_clipboard_switches(false, false);

    // --- A call ---------------------------------------------------------------------------------------------
    phone.service.send_to(&seen_by_phone, "kdeconnect.telephony", json!({ "event": "ringing", "contactName": "Alex Example", "phoneNumber": "+15550100" }));
    until("the ringing", || computer.notices.last(&format!("call:{pid}")).is_some()).await;
    let ringing = computer.notices.last(&format!("call:{pid}")).unwrap();
    assert!(ringing.urgent && ringing.summary == "Alex Example" && ringing.actions[0].0 == "mute");
    computer.notices.press(&format!("call:{pid}"), "mute");
    until("the mute", || !phone.saw("kdeconnect.telephony.request_mute").is_empty()).await;
    phone.service.send_to(&seen_by_phone, "kdeconnect.telephony", json!({ "event": "ringing", "contactName": "Alex Example", "phoneNumber": "+15550100", "isCancel": true }));
    until("the call to end", || computer.notices.closed.borrow().contains(&format!("call:{pid}"))).await;
    let states: Vec<String> = signals(&computer, "Call").iter().map(|a| a.child_value(0).str().unwrap().to_string()).collect();
    assert_eq!(states, ["ringing", "ended"]);
    // The live pill shows the ringing phone too: the module's connection is sent the same signal
    assert!(amethystora_connect::dbus::audience(amethystora_connect::dbus::Role::Module, "Call", signals(&computer, "Call").first()));

    // --- Contacts and texts ---------------------------------------------------------------------------------
    phone.service.send_to(&seen_by_phone, "kdeconnect.contacts.response_uids_timestamps", json!({ "uids": ["205"], "205": 1761000000000i64 }));
    until("the cards to be asked for", || !phone.saw("kdeconnect.contacts.request_vcards_by_uid").is_empty()).await;
    phone.service.send_to(&seen_by_phone, "kdeconnect.contacts.response_vcards", json!({ "uids": ["205"], "205": "BEGIN:VCARD\nVERSION:2.1\nFN:Alex Example\nTEL;CELL:+1 555 0100\nX-KDECONNECT-TIMESTAMP:1761000000000\nEND:VCARD" }));
    until("the contact", || computer.dir.join("state/contacts-").as_os_str().len() > 0 && find_in(&computer.dir.join("state"), "contacts-").len() == 1).await;
    phone.service.send_to(&seen_by_phone, "kdeconnect.sms.messages", json!({ "version": 2, "messages": [
        { "_id": 900, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": "See you at 8", "date": 1761253000000i64, "type": 1, "read": 0 },
        { "_id": 880, "thread_id": 7, "addresses": [ { "address": "+15550102" } ], "body": "Thanks!", "date": 1761200000000i64, "type": 2, "read": 1 } ] }));
    until("the conversations", || computer.service.list_conversations(&pid, 10).map(|l| l.len()).unwrap_or(0) == 2).await;
    let conversations = computer.service.list_conversations(&pid, 10).unwrap();
    assert_eq!(conversations[0]["name"].get::<String>().as_deref(), Some("Alex Example"), "the contact's name");
    assert_eq!(conversations[0]["thread_id"].get::<i64>(), Some(42));
    assert_eq!(conversations[0]["unread"].get::<bool>(), Some(true));
    assert!(emitted(&computer, "ConversationsChanged") > 0 && emitted(&computer, "MessagesChanged") > 0);
    assert_eq!(computer.service.list_messages(&pid, 42, 10).unwrap().len(), 1);
    // A code from your phone (R7-23): nothing is looked at while the switch is off, even for a text that came just now
    let now = packet::now_ms();
    let text = |id: i64, date: i64, body: &str| json!({ "version": 2, "messages": [ { "_id": id, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": body, "date": date, "type": 1, "read": 0 } ] });
    phone.service.send_to(&seen_by_phone, "kdeconnect.sms.messages", text(901, now, "Your code is 111222"));
    until("the text", || computer.service.list_messages(&pid, 42, 10).map(|l| l.len()).unwrap_or(0) == 2).await;
    assert_eq!(emitted(&computer, "Code"), 0, "off: no text is looked at");
    computer.service.codes.set(true);
    phone.service.send_to(&seen_by_phone, "kdeconnect.sms.messages", text(902, now, "Your code is 482913"));
    until("the code", || emitted(&computer, "Code") == 1).await;
    assert_eq!(signals(&computer, "Code")[0].child_value(0).str(), Some("482913"), "the digits and nothing else");
    // An old text, a text without a code, and the same text again say nothing
    phone.service.send_to(&seen_by_phone, "kdeconnect.sms.messages", text(903, now - 10 * 60_000, "Your code is 333444"));
    phone.service.send_to(&seen_by_phone, "kdeconnect.sms.messages", text(904, now, "Order 555666 has shipped"));
    phone.service.send_to(&seen_by_phone, "kdeconnect.sms.messages", text(902, now, "Your code is 482913"));
    until("the texts", || computer.service.list_messages(&pid, 42, 10).map(|l| l.len()).unwrap_or(0) == 5).await;
    assert_eq!(emitted(&computer, "Code"), 1);
    computer.service.codes.set(false);
    computer.service.request_messages(&pid, 42, 0, 0).unwrap();
    computer.service.send_message(&pid, &[], "Running late, start without me.", 42).unwrap();
    until("the text to go", || !phone.saw("kdeconnect.sms.request").is_empty()).await;
    let sent = &phone.saw("kdeconnect.sms.request")[0];
    assert_eq!(sent.body["addresses"][0]["address"], "+15550100");
    assert_eq!(sent.str("messageBody"), Some("Running late, start without me."));
    assert!(computer.service.send_message(&pid, &["+1".into(), "+2".into()], "group", 0).is_err(), "groups are read-only");
    assert!(computer.service.send_message(&pid, &["+1".into()], "  ", 0).is_err());
    assert!(!phone.saw("kdeconnect.sms.request_conversation").is_empty());

    // --- A hostile peer made from the crate's own parts ------------------------------------------------------
    hostile(&computer, &pid).await;

    // --- Unpairing ------------------------------------------------------------------------------------------
    assert!(computer.device(&pid).st.borrow().signal.is_some());
    computer.service.unpair(&pid).unwrap();
    until("the unpairing", || !seen_by_computer_exists(&computer, &pid) || !computer.device(&pid).paired()).await;
    assert!(computer.service.device(&pid).is_none_or(|d| d.st.borrow().signal.is_none() && prop::<i32>(&d, "SignalStrength") == -1), "away, the bars go");
    until("the phone to know", || !seen_by_phone.paired()).await;
    assert!(!store_has(&computer, &pid) && !store_has(&phone, &cid));
    assert!(find_in(&computer.dir.join("state"), "contacts-").is_empty(), "everything kept for the phone goes");
    assert!(phone.notices.raised_with_prefix(&format!("info:{cid}")).iter().any(|n| n.summary.contains("unpaired")), "the person is told the phone unpaired it");
    assert!(computer.service.ping(&pid).is_err());

    // --- And pairing again, the phone asking, the computer's person refusing -------------------------------
    let seen_by_computer = computer.service.device(&pid);
    if seen_by_computer.is_some() {
        phone.service.pair(&cid).unwrap();
        until("the computer's prompt", || seen_by_computer.as_ref().is_some_and(|d| prop::<String>(d, "Pairing") == "incoming")).await;
        let prompt = prompt_of(&computer, &pid).unwrap();
        assert_eq!(prompt.actions.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), ["accept", "reject"]);
        computer.notices.press(&prompt.key, "reject");
        until("the refusal to reach the phone", || phone.notices.raised_with_prefix(&format!("info:{cid}")).iter().any(|n| n.summary.contains("did not accept"))).await;
        assert!(!store_has(&computer, &pid) && !store_has(&phone, &cid));
    }
}

/// The phone's connectivity reports: each vector of the spec in turn, a property change only when a value changed,
/// and nothing from a phone that did not list the report
async fn connectivity(computer: &Peer, phone: &Peer, pid: &str) {
    let cid = computer.id();
    let seen_by_computer = computer.device(pid);
    let seen_by_phone = phone.device(&cid);
    assert_eq!(prop::<i32>(&seen_by_computer, "SignalStrength"), -1, "nothing before the first report");
    assert_eq!(prop::<String>(&seen_by_computer, "NetworkType"), "");
    let changes = || {
        computer
            .service
            .recorded
            .borrow()
            .iter()
            .filter(|s| s.name == "PropertiesChanged" && s.device.as_deref() == Some(pid))
            .filter(|s| {
                let changed = s.args.as_ref().unwrap().child_value(1).get::<std::collections::HashMap<String, glib::Variant>>().unwrap();
                ["SignalStrength", "NetworkType"].iter().any(|k| changed.contains_key(*k))
            })
            .count()
    };
    let mut sent = computer.saw("kdeconnect.connectivity_report").len();
    let mut twenty = serde_json::Map::new();
    for i in 1..=20 {
        twenty.insert(i.to_string(), json!({ "networkType": format!("N{i}"), "signalStrength": if i <= 8 { 1 } else { 4 } }));
    }
    let vectors: Vec<(&str, serde_json::Value, i32, &str, bool)> = vec![
        ("V-CR-1", json!({"signalStrengths": {"6": {"networkType": "LTE", "signalStrength": 3}}}), 3, "LTE", true),
        ("V-CR-1 again", json!({"signalStrengths": {"6": {"networkType": "LTE", "signalStrength": 3}}}), 3, "LTE", false),
        ("V-CR-2", json!({"signalStrengths": {"1": {"networkType": "5G", "signalStrength": 2}, "2": {"networkType": "LTE", "signalStrength": 4}}}), 4, "LTE", true),
        ("V-CR-3", json!({"signalStrengths": {}}), -1, "", true),
        ("V-CR-4", json!({"signalStrengths": {"1": {"networkType": "LTE", "signalStrength": 7}}}), -1, "", false),
        ("V-CR-5", json!({"signalStrengths": {"1": {"networkType": "LTE", "signalStrength": "3"}}}), -1, "", false),
        ("V-CR-6", json!({"signalStrengths": {"1": {"networkType": "\u{202e}ETL\u{0007}", "signalStrength": 2}}}), 2, "Unknown", true),
        ("V-CR-7", json!({"signalStrengths": {"1": {"networkType": "4G", "signalStrength": 2.0}}}), 2, "4G", true),
        ("V-CR-8, no map", json!({}), 2, "4G", false),
        ("V-CR-8, not an object", json!({"signalStrengths": ["LTE"]}), 2, "4G", false),
        ("V-CR-9", json!({ "signalStrengths": twenty }), 1, "N1", true),
    ];
    for (name, body, strength, network, change) in vectors {
        let before = changes();
        phone.service.send_to(&seen_by_phone, "kdeconnect.connectivity_report", body);
        sent += 1;
        until(name, || computer.saw("kdeconnect.connectivity_report").len() == sent).await;
        assert_eq!(prop::<i32>(&seen_by_computer, "SignalStrength"), strength, "{name}");
        assert_eq!(prop::<String>(&seen_by_computer, "NetworkType"), network, "{name}");
        assert_eq!(changes(), before + usize::from(change), "{name}: a PropertiesChanged only when a value changed");
    }
    assert!(prop::<Vec<String>>(&seen_by_computer, "Features").contains(&"connectivity".to_string()));
    // A phone that did not list the report as one it sends: no feature, and a report it sends anyway is dropped
    seen_by_computer.st.borrow_mut().outgoing.retain(|c| c != "kdeconnect.connectivity_report");
    assert!(!prop::<Vec<String>>(&seen_by_computer, "Features").contains(&"connectivity".to_string()));
    phone.service.send_to(&seen_by_phone, "kdeconnect.connectivity_report", json!({"signalStrengths": {"1": {"networkType": "LTE", "signalStrength": 4}}}));
    sent += 1;
    until("the unlisted report", || computer.saw("kdeconnect.connectivity_report").len() == sent).await;
    assert_eq!(prop::<i32>(&seen_by_computer, "SignalStrength"), 1, "dropped");
    seen_by_computer.st.borrow_mut().outgoing.push("kdeconnect.connectivity_report".into());
}

/// Where pictures go: the setting's two values, the property that says it, and a name that tries to leave the folder
async fn photo_folder(computer: &Peer, phone: &Peer, cid: &str, source: &Path) {
    use amethystora_connect::service::PhotoFolder;
    use std::os::unix::fs::PermissionsExt;
    let pictures = computer.pictures().join("Phone");
    assert_eq!(computer.service.photo_folder_path().as_deref(), Some(pictures.as_path()));
    assert_eq!(std::fs::metadata(&pictures).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(pictures.join("from phone.png")).unwrap().permissions().mode() & 0o777, 0o600);
    let photo_folder_said = || -> Vec<String> {
        computer
            .service
            .recorded
            .borrow()
            .iter()
            .filter(|s| s.name == "PropertiesChanged" && s.device.is_none())
            .filter_map(|s| s.args.as_ref().unwrap().child_value(1).get::<std::collections::HashMap<String, glib::Variant>>())
            .filter_map(|changed| changed.get("PhotoFolder").and_then(|v| v.get::<String>()))
            .collect()
    };
    // Downloads: pictures go with the phone's other files, in its own folder
    computer.service.set_photo_folder(PhotoFolder::Downloads);
    assert_eq!(photo_folder_said().last().map(String::as_str), computer.downloads().join("Phone").to_str());
    std::fs::write(source.join("second.jpg"), b"jpeg").unwrap();
    let ids = phone.service.send_files(cid, &[file_uri(&source.join("second.jpg"))]).unwrap();
    until("the picture to land in Downloads", || state_of(phone, "finished", &ids[0]) && computer.downloads().join("Phone/Sam phone/second.jpg").exists()).await;
    assert!(!pictures.join("second.jpg").exists());
    computer.service.set_photo_folder(PhotoFolder::Pictures);
    assert_eq!(photo_folder_said().last().map(String::as_str), pictures.to_str());
    // A picture named to climb out lands in Pictures/Phone and nowhere else
    let offer = payload::offer(&(0..=0)).unwrap();
    let port = offer.port;
    let link = phone.device(cid).link().unwrap();
    let pin = phone.service.pin_of(cid).unwrap();
    let cert = phone.service.me.tls.clone();
    let task = glib::spawn_future_local(async move {
        let mut given = false;
        let _ = offer
            .serve(
                &cert,
                pin,
                4,
                &mut |buf| {
                    if given {
                        return Ok(0);
                    }
                    given = true;
                    buf[..4].copy_from_slice(b"evil");
                    Ok(4)
                },
                &|_| {},
            )
            .await;
    });
    link.send(&Packet::new("kdeconnect.share.request", json!({ "filename": "../../evil.jpg" })).with_payload(4, port));
    until("the climbing picture", || pictures.join("evil.jpg").exists()).await;
    task.await.ok();
    assert!(!computer.dir.join("evil.jpg").exists() && !computer.pictures().join("evil.jpg").exists());
    assert_eq!(std::fs::metadata(pictures.join("evil.jpg")).unwrap().permissions().mode() & 0o777, 0o600);
}

fn seen_by_computer_exists(computer: &Peer, pid: &str) -> bool {
    computer.service.device(pid).is_some()
}

fn store_has(peer: &Peer, id: &str) -> bool {
    peer.service.store.borrow().paired.contains_key(id)
}

/// Things a stranger on the network can do, and what must come of them
async fn hostile(computer: &Peer, paired_phone: &str) {
    let port = computer.port();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let devices_before = computer.service.devices.borrow().len();

    // Garbage instead of an identity, a line longer than the limit, and an identity that is not one: all closed
    for garbage in [b"GET / HTTP/1.1\r\n\r\n".to_vec(), vec![b'x'; 20_000], b"{\"type\":\"kdeconnect.ping\",\"body\":{}}\n".to_vec(), format!("{{\"type\":\"kdeconnect.identity\",\"body\":{{\"deviceId\":\"short\",\"protocolVersion\":8}}}}\n").into_bytes()] {
        let connection = net::connect(address, 0).await.unwrap();
        net::write_all(&connection.output_stream(), garbage).await.ok();
        let mut reader = LineReader::new(connection.input_stream());
        let outcome = net::timed(Duration::from_secs(5), reader.next(1024)).await;
        assert!(matches!(outcome, Ok(None) | Err(_)), "the computer closes the connection");
    }
    assert_eq!(computer.service.devices.borrow().len(), devices_before);

    // An identity that names another device inside TLS than before it
    let stranger = "a".repeat(32);
    let other = "b".repeat(32);
    let made = x509::generate(&stranger, packet::now_secs()).unwrap();
    let certificate = gio::TlsCertificate::from_pem(&format!("{}{}", made.certificate_pem, made.key_pem)).unwrap();
    let connection = net::connect(address, 0).await.unwrap();
    let local = identity::Local::new(&stranger, "Stranger", "phone");
    net::write_all(&connection.output_stream(), local.packet(1716, true, None).to_line()).await.unwrap();
    let base: gio::IOStream = connection.upcast();
    // Whoever opened the connection is the TLS server
    let secured = net::secure_as_server(&base, &certificate, Trust::FirstUse).await.unwrap();
    let swapped = identity::Local::new(&other, "Stranger", "phone");
    net::write_all(&secured.tls.output_stream(), swapped.packet_in_tls().to_line()).await.unwrap();
    let mut reader = LineReader::new(secured.tls.input_stream());
    // It sends its own identity first, then the computer notices and closes
    let first = net::timed(Duration::from_secs(5), reader.next(65536)).await;
    assert!(matches!(first, Ok(Some(_))));
    let end = net::timed(Duration::from_secs(5), reader.next(65536)).await;
    assert!(matches!(end, Ok(None) | Err(_)), "a changed identity ends the connection");
    pause(100).await;
    assert!(computer.service.device(&other).is_none() && computer.service.device(&stranger).is_none());

    // Something that claims to be the paired phone with another certificate is refused, and the pairing is kept
    let forged = x509::generate(paired_phone, packet::now_secs()).unwrap();
    let certificate = gio::TlsCertificate::from_pem(&format!("{}{}", forged.certificate_pem, forged.key_pem)).unwrap();
    let connection = net::connect(address, 0).await.unwrap();
    let local = identity::Local::new(paired_phone, "Sam phone", "phone");
    net::write_all(&connection.output_stream(), local.packet(1716, true, None).to_line()).await.unwrap();
    let base: gio::IOStream = connection.upcast();
    // The computer is the TLS client here and checks the certificate against its pin: the refusal may come as a
    // failed handshake or as a closed stream
    let attempt = net::secure_as_server(&base, &certificate, Trust::FirstUse).await;
    if let Ok(secured) = attempt {
        let mut reader = LineReader::new(secured.tls.input_stream());
        let outcome = net::timed(Duration::from_secs(5), reader.next(65536)).await;
        assert!(matches!(outcome, Ok(None) | Err(_)), "{outcome:?}");
    }
    until("the person to be told", || computer.notices.raised_with_prefix(&format!("info:{paired_phone}")).iter().any(|n| n.summary.contains("claimed"))).await;
    assert!(store_has(computer, paired_phone), "the pin stays");
}
