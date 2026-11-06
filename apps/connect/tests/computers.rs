//! Computer to computer (plans/beyond/spec-r4-later.md, part A): two copies of the program that are both computers.
//! With the switch `computers` off, as the image ships, they neither call nor answer each other; on, they pair with the
//! key compared, send a file and share the clipboard; off again, the other computer leaves the bus and its pairing is
//! kept; a paired computer that calls itself a phone is still a computer; on again, it is back without a new prompt.
//! One test, because the main context of a thread has one owner.

use std::path::Path;

mod common;

use common::*;

#[test]
fn two_computers() {
    let root = std::env::temp_dir().join(format!("amethystora-connect-computers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    glib::MainContext::default().block_on(scenario(&root));
    let _ = std::fs::remove_dir_all(&root);
}

async fn scenario(root: &Path) {
    let desk = make(root, "Desk PC", "desktop", false, true);
    let laptop = make(root, "Studio laptop", "laptop", false, true);
    let (did, lid) = (desk.id(), laptop.id());
    assert!(!desk.service.computers.get() && !laptop.service.computers.get(), "off as the image ships");

    // --- Off: a datagram, and a call, come to nothing --------------------------------------------------------
    let udp = desk.service.udp_port().expect("the desk listens for datagrams");
    laptop.service.announce_to_port("127.0.0.1".parse().unwrap(), 0, udp);
    desk.service.connect_to(&format!("127.0.0.1:{}", laptop.port())).unwrap();
    pause(600).await;
    assert!(desk.service.device(&lid).is_none(), "the desk lists no computer while off");
    assert!(laptop.service.device(&did).is_none(), "nor does the laptop");

    // --- On, on both: they find each other and pair -----------------------------------------------------------
    // Calls to one address are two seconds apart at least (limits::ATTEMPT_SPACING)
    pause(2100).await;
    desk.service.set_computers(true);
    laptop.service.set_computers(true);
    laptop.service.announce_to_port("127.0.0.1".parse().unwrap(), 0, udp);
    until("the two computers to connect", || desk.service.device(&lid).is_some_and(|d| d.connected()) && laptop.service.device(&did).is_some_and(|d| d.connected())).await;
    let laptop_here = desk.device(&lid);
    let desk_there = laptop.device(&did);
    assert_eq!(prop::<String>(&laptop_here, "Type"), "laptop");
    assert_eq!(prop::<String>(&desk_there, "Type"), "desktop");

    desk.service.pair(&lid).unwrap();
    until("the laptop's prompt", || prompt_of(&laptop, &did).is_some()).await;
    let prompt = prompt_of(&laptop, &did).unwrap();
    let key = prop::<String>(&laptop_here, "PairingKey");
    assert_eq!(key, prop::<String>(&desk_there, "PairingKey"), "both screens show the same key");
    assert!(prompt.body.contains(&key[..4]));
    assert!(prompt.body.contains("other computer"), "the prompt speaks of a computer: {}", prompt.body);
    assert!(prompt_of(&desk, &lid).is_some_and(|n| n.body.contains("other computer")));
    laptop.notices.press(&prompt.key, "accept");
    until("the pairing", || laptop_here.paired() && desk_there.paired()).await;
    let features = prop::<Vec<String>>(&laptop_here, "Features");
    assert!(features.iter().any(|f| f == "share") && features.iter().any(|f| f == "clipboard") && features.iter().any(|f| f == "ping"), "{features:?}");
    assert!(!features.iter().any(|f| f == "sms" || f == "telephony"), "a computer has no texts or calls: {features:?}");

    // --- A file from the laptop to the desk -------------------------------------------------------------------
    let source = root.join("source");
    std::fs::create_dir_all(&source).unwrap();
    let data = pattern(70_000);
    std::fs::write(source.join("report.pdf"), &data).unwrap();
    let ids = laptop.service.send_files(&did, &[file_uri(&source.join("report.pdf"))]).unwrap();
    let landed = desk.downloads().join("Phone/Studio laptop/report.pdf");
    // Send to nearby (R7-27): another computer's file waits for Accept, and nothing arrives by itself
    until("the desk's card", || !desk.notices.raised_with_prefix(&format!("ask:{lid}:")).is_empty()).await;
    pause(300).await;
    assert!(!landed.exists(), "nothing arrived before Accept");
    let card = desk.notices.raised_with_prefix(&format!("ask:{lid}:")).pop().unwrap();
    assert!(card.body.contains("report.pdf"), "the card says what: {}", card.body);
    desk.notices.press(&card.key, "accept");
    until("the file on the desk", || state_of(&laptop, "finished", &ids[0]) && landed.exists()).await;
    assert_eq!(std::fs::read(&landed).unwrap(), data);

    // A second file, declined: the laptop is sent away, and nothing lands
    std::fs::write(source.join("declined.txt"), b"no").unwrap();
    laptop.service.send_files(&did, &[file_uri(&source.join("declined.txt"))]).unwrap();
    until("the second card", || desk.notices.raised_with_prefix(&format!("ask:{lid}:")).len() > 1).await;
    let card = desk.notices.raised_with_prefix(&format!("ask:{lid}:")).pop().unwrap();
    desk.notices.press(&card.key, "decline");
    pause(500).await;
    assert!(!desk.downloads().join("Phone/Studio laptop/declined.txt").exists(), "a declined file never lands");

    // With the key `never` the file arrives as it did before
    desk.service.ask_first.set(amethystora_connect::ask::AskFirst::Never);
    std::fs::write(source.join("straight.txt"), b"yes").unwrap();
    let ids = laptop.service.send_files(&did, &[file_uri(&source.join("straight.txt"))]).unwrap();
    let straight = desk.downloads().join("Phone/Studio laptop/straight.txt");
    until("the file with no card", || state_of(&laptop, "finished", &ids[0]) && straight.exists()).await;
    desk.service.ask_first.set(amethystora_connect::ask::AskFirst::Computers);

    // --- The clipboard, only while the switches say so --------------------------------------------------------
    laptop.service.push_clipboard("not sent: the switch is off");
    pause(200).await;
    assert!(desk.saw("kdeconnect.clipboard").is_empty());
    laptop.service.set_clipboard_switches(true, false);
    desk.service.set_clipboard_switches(false, true);
    laptop.service.push_clipboard("copied on the laptop");
    until("the clipboard on the desk", || desk.saw("kdeconnect.clipboard").iter().any(|p| p.str("content") == Some("copied on the laptop"))).await;
    until("the desk to offer it", || signals(&desk, "ClipboardReceived").iter().any(|a| a.child_value(1).str() == Some("copied on the laptop"))).await;

    // --- Off on the desk: the laptop leaves its bus, its pairing stays -------------------------------------------
    let removed_before = emitted(&desk, "DeviceRemoved");
    desk.service.set_computers(false);
    assert!(desk.service.device(&lid).is_none(), "the laptop is off the desk's list");
    assert_eq!(emitted(&desk, "DeviceRemoved"), removed_before + 1);
    assert!(desk.service.store.borrow().paired.contains_key(&lid), "the pairing is kept");
    until("the laptop to see the desk go", || !desk_there.connected()).await;
    pause(300).await;
    assert!(desk.service.device(&lid).is_none(), "the closed connection did not bring it back");

    // A paired computer that calls itself a phone is still a computer: the laptop's own certificate and id, as a phone
    let impostor = make_with(root, "Studio laptop", "phone", false, false, |_| {});
    assert_eq!(impostor.id(), lid, "the same identity on disk");
    impostor.service.set_computers(true);
    impostor.service.connect_to(&format!("127.0.0.1:{}", desk.port())).unwrap();
    pause(600).await;
    assert!(desk.service.device(&lid).is_none(), "refused while computers are off");

    // --- On again: known at once, and the same caller is let in, so that it was the switch that refused it ----------
    let prompts_before = laptop.notices.raised_with_prefix(&format!("pair:{did}:")).len();
    desk.service.set_computers(true);
    assert!(desk.service.device(&lid).is_some_and(|d| d.paired()), "known again, paired");
    pause(2100).await;
    impostor.service.connect_to(&format!("127.0.0.1:{}", desk.port())).unwrap();
    until("the same caller to be let in with the switch on", || desk.service.device(&lid).is_some_and(|d| d.connected())).await;
    impostor.service.stop().await;
    until("it to go", || desk.service.device(&lid).is_some_and(|d| !d.connected())).await;
    drop(impostor);

    // --- Connected again to the laptop itself, without a prompt ------------------------------------------------------
    pause(2100).await;
    desk.service.connect_to(&format!("127.0.0.1:{}", laptop.port())).unwrap();
    until("the two to connect again", || desk.service.device(&lid).is_some_and(|d| d.connected() && d.paired())).await;
    pause(200).await;
    assert_eq!(laptop.notices.raised_with_prefix(&format!("pair:{did}:")).len(), prompts_before, "no new prompt");
}
