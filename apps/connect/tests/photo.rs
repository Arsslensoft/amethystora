//! Snap with your phone (R7-69, plans/amethystora-connect/spec-photo.md): the request is listed and sent only while the switch is on, only
//! to a phone whose app lists it, one at a time, and a cancel or a minute of silence drops it. One test, because the main context of a
//! thread has one owner.

use std::path::Path;

mod common;

use common::*;
use serde_json::json;

#[test]
fn snap_with_your_phone() {
    let root = std::env::temp_dir().join(format!("amethystora-connect-photo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    glib::MainContext::default().block_on(scenario(&root));
    let _ = std::fs::remove_dir_all(&root);
}

/// A phone, as an old install of the app still lists it: it takes the photo request
fn old_phone(root: &Path) -> Peer {
    make_with(root, "Old phone", "phone", true, false, |c| {
        let (mut incoming, outgoing) = phone_capabilities();
        incoming.push("kdeconnect.photo.request".into());
        c.capabilities = Some((incoming, outgoing));
    })
}

/// The computer let the phone go when a switch changed: it is called again, as a phone would come back by itself
async fn reconnected(computer: &Peer, phone: &Peer) {
    let pid = phone.id();
    until("the phone to be let go", || !computer.device(&pid).connected()).await;
    // A call to the same address is made at most once in two seconds: tried until it is
    for _ in 0..20 {
        computer.service.connect_to(&format!("127.0.0.1:{}", phone.port())).unwrap();
        pause(500).await;
        if computer.device(&pid).connected() && phone.device(&computer.id()).connected() {
            return;
        }
    }
    panic!("the phone did not come back");
}

async fn scenario(root: &Path) {
    // --- Off, as the image ships: not listed, not offered, not sent ----------------------------------------------------
    let computer = make(root, "Studio laptop", "laptop", false, false);
    let phone = old_phone(root);
    let (cid, pid) = (computer.id(), phone.id());
    connect_and_pair(&computer, &phone).await;
    let seen = computer.device(&pid);
    assert!(!prop::<Vec<String>>(&seen, "Features").iter().any(|f| f == "photo"), "off: nothing is offered");
    assert!(computer.service.request_photo(&pid).is_err(), "off: nothing is sent");
    assert!(!phone.device(&cid).takes("kdeconnect.photo"), "off: this computer does not list the photo it would get");

    // --- On: the phone is let go and comes back with the new list; the phone that takes the request is offered it -------
    computer.service.set_photo(true);
    reconnected(&computer, &phone).await;
    until("the phone to be offered it", || computer.service.device(&pid).is_some_and(|d| d.features().contains(&"photo"))).await;
    assert!(phone.device(&cid).takes("kdeconnect.photo"));
    assert!(prop::<Vec<String>>(&computer.device(&pid), "Features").iter().any(|f| f == "photo"));

    // A phone of a current app, which lists no such request, is offered nothing
    let current = make_with(root, "Current phone", "phone", true, false, |_| {});
    let current_id = current.id();
    // Calls to one address are two seconds apart at least
    pause(2100).await;
    connect_and_pair(&computer, &current).await;
    assert!(!prop::<Vec<String>>(&computer.device(&current_id), "Features").iter().any(|f| f == "photo"));
    assert!(computer.service.request_photo(&current_id).is_err(), "no request to a phone that does not list it");
    assert!(current.saw("kdeconnect.photo.request").is_empty());

    // --- A request: sent once, with an empty body; a second waits for the first ----------------------------------------------
    computer.service.request_photo(&pid).unwrap();
    until("the request at the phone", || !phone.saw("kdeconnect.photo.request").is_empty()).await;
    assert_eq!(phone.saw("kdeconnect.photo.request")[0].body.len(), 0, "no field, no path");
    assert!(computer.service.request_photo(&pid).is_err(), "one request waits for each phone");
    assert_eq!(phone.saw("kdeconnect.photo.request").len(), 1);
    assert!(signals(&computer, "Photo").iter().any(|a| a.child_value(0).str() == Some("asked")));
    let card = computer.notices.raised_with_prefix(&format!("photo:{pid}:")).pop().expect("the card with Cancel");
    assert!(!card.body.contains("is open"), "it says it asked, never that the camera is open: {}", card.body);

    // --- The person leaves the camera: the phone says so, and the request is dropped -------------------------------------------
    let to_computer = phone.device(&cid);
    assert!(phone.service.send_to(&to_computer, "kdeconnect.photo", json!({ "cancel": true })));
    until("the cancel to be heard", || signals(&computer, "Photo").iter().any(|a| a.child_value(0).str() == Some("cancelled"))).await;
    computer.service.request_photo(&pid).expect("a new request after the cancel");

    // --- Cancel on the card forgets the request here -----------------------------------------------------------------------
    let card = computer.notices.raised_with_prefix(&format!("photo:{pid}:")).pop().unwrap();
    computer.notices.press(&card.key, "cancel");
    computer.service.request_photo(&pid).expect("a new request after Cancel on the card");
    until("a third request at the phone", || phone.saw("kdeconnect.photo.request").len() == 3).await;

    // --- A cancel nobody asked for is nothing; a picture nobody asked for is refused, not fetched -----------------------------------
    let before = signals(&computer, "Photo").len();
    // (a request waits, so this cancel is heard; drop that one first, then send the stray)
    assert!(phone.service.send_to(&to_computer, "kdeconnect.photo", json!({ "cancel": true })));
    until("the cancel", || signals(&computer, "Photo").len() > before).await;
    let after = signals(&computer, "Photo").len();
    assert!(phone.service.send_to(&to_computer, "kdeconnect.photo", json!({ "cancel": true })));
    pause(300).await;
    assert_eq!(signals(&computer, "Photo").len(), after, "a cancel with no request waiting does nothing");
    assert!(find_in(&computer.downloads().join("Phone/Old phone"), "JPEG_").is_empty() && find_in(&computer.pictures(), "JPEG_").is_empty());

    // --- Off again: the request is not listed, and the phone is not offered it -----------------------------------------------------
    computer.service.set_photo(false);
    reconnected(&computer, &phone).await;
    until("the phone not to be offered it", || computer.service.device(&pid).is_some_and(|d| !d.features().contains(&"photo"))).await;
    assert!(computer.service.request_photo(&pid).is_err());
}
