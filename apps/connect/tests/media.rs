//! The phone, a little further (plans/ui-ideas/spec-phone.md): ringing the phone, music from either side and the phone
//! as a slide clicker, between two copies of the program on the loopback (the second plays a phone that lists the
//! three plugins), and the computer's own players on a session bus of its own, with a stand-in MPRIS player.
//! Everything is one test, because the main context of a thread has one owner.

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use amethystora_connect::media::{self, KEYS, MPRIS, MPRIS_REQUEST, PRESENTER, RING};
use amethystora_connect::packet::Packet;
use amethystora_connect::{dbus, service::Service};
use common::*;
use glib::variant::ToVariant;
use serde_json::json;

#[test]
fn ring_music_and_the_clicker() {
    let root = std::env::temp_dir().join(format!("amethystora-connect-media-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    glib::MainContext::default().block_on(scenario(&root));
    let _ = std::fs::remove_dir_all(&root);
}

/// A phone that also rings, has players, and is a presenter
fn media_phone(root: &Path) -> Peer {
    make_with(root, "Sam phone", "phone", true, false, |c| {
        let (mut incoming, mut outgoing) = phone_capabilities();
        incoming.extend([RING, MPRIS, MPRIS_REQUEST].map(str::to_string));
        outgoing.extend([MPRIS, MPRIS_REQUEST, KEYS, PRESENTER].map(str::to_string));
        c.capabilities = Some((incoming, outgoing));
    })
}

fn features(peer: &Peer, id: &str) -> Vec<String> {
    prop::<Vec<String>>(&peer.device(id), "Features")
}

fn players(peer: &Peer, id: &str) -> Vec<HashMap<String, glib::Variant>> {
    prop::<Vec<HashMap<String, glib::Variant>>>(&peer.device(id), "Players")
}

/// The computer let the phone go when a switch changed: it is called again, as a phone would come back by itself
async fn reconnected(computer: &Peer, phone: &Peer) {
    let pid = phone.id();
    until("the phone to be let go", || !computer.device(&pid).connected()).await;
    // A call to the same address is made at most once in two seconds (limits::ATTEMPT_SPACING): tried until it is
    for _ in 0..20 {
        computer.service.connect_to(&format!("127.0.0.1:{}", phone.port())).unwrap();
        pause(500).await;
        if computer.device(&pid).connected() && phone.device(&computer.id()).connected() {
            return;
        }
    }
    panic!("the phone did not come back");
}

fn clicks(peer: &Peer) -> Vec<(String, String)> {
    signals(peer, "Clicker").iter().map(|a| (a.child_value(0).str().unwrap().to_string(), a.child_value(1).str().unwrap().to_string())).collect()
}

async fn scenario(root: &Path) {
    let computer = make(root, "Studio laptop", "laptop", false, false);
    let phone = media_phone(root);
    let (cid, pid) = (computer.id(), phone.id());
    connect_and_pair(&computer, &phone).await;
    let link = || phone.device(&cid).link().expect("the phone's connection");

    // --- R5-29: ringing, always offered -------------------------------------------------------------------------------
    assert!(features(&computer, &pid).contains(&"ring".to_string()));
    let ours = phone.device(&cid).st.borrow().outgoing.clone();
    assert!(ours.iter().any(|c| c == RING), "the computer lists the ring as a packet it sends");
    computer.service.ring(&pid).unwrap();
    until("the phone to be rung", || phone.saw(RING).len() == 1).await;
    assert_eq!(phone.saw(RING)[0].body.len(), 0, "the request has no fields");

    // --- Off as shipped: neither music nor the clicker is listed, offered or heard ----------------------------------
    let theirs = phone.device(&cid).st.borrow().incoming.clone();
    for kind in [MPRIS, MPRIS_REQUEST, PRESENTER, KEYS] {
        assert!(!theirs.iter().any(|c| c == kind), "{kind} is listed while off");
    }
    let f = features(&computer, &pid);
    assert!(!f.contains(&"music".to_string()) && !f.contains(&"clicker".to_string()), "{f:?}");
    link().send(&Packet::new(MPRIS, json!({ "playerList": ["Music"] })));
    link().send(&Packet::new(KEYS, json!({ "specialKey": 9 })));
    until("the computer to hear them", || !computer.saw(MPRIS).is_empty() && !computer.saw(KEYS).is_empty()).await;
    pause(150).await;
    assert!(players(&computer, &pid).is_empty());
    assert!(phone.saw(MPRIS_REQUEST).is_empty(), "nothing is asked of the phone's players");
    assert!(computer.service.media_action(&pid, "Music", "Play").is_err());
    assert!(clicks(&computer).is_empty());
    assert!(!computer.service.watching_players());

    // --- R5-30: music on. The phone is let go and comes back with the media packets listed ---------------------------
    computer.service.set_media_switches(true, false);
    reconnected(&computer, &phone).await;
    until("the phone to see the new list", || phone.device(&cid).takes(MPRIS)).await;
    assert!(features(&computer, &pid).contains(&"music".to_string()));
    until("the computer to ask for the phone's players", || phone.saw(MPRIS_REQUEST).iter().any(|p| p.bool("requestPlayerList") == Some(true))).await;
    link().send(&Packet::new(MPRIS, json!({ "playerList": ["Music", "Podcasts"] })));
    until("the state of each to be asked", || phone.saw(MPRIS_REQUEST).iter().filter(|p| p.bool("requestNowPlaying") == Some(true)).count() == 2).await;
    link().send(&Packet::new(MPRIS, json!({ "player": "Music", "title": "Night Drive", "artist": "The\u{202e} Band", "isPlaying": true, "canPause": true, "canGoNext": true, "length": 240000, "pos": 61000 })));
    until("the player in the property", || players(&computer, &pid).iter().any(|p| p["title"].str() == Some("Night Drive"))).await;
    let music = players(&computer, &pid).into_iter().find(|p| p["player"].str() == Some("Music")).unwrap();
    assert_eq!(music["artist"].str(), Some("The Band"), "text from the phone is cleaned");
    assert_eq!((music["playing"].get::<bool>(), music["length"].get::<i64>(), music["position"].get::<i64>()), (Some(true), Some(240000), Some(61000)));
    computer.service.media_action(&pid, "Music", "PlayPause").unwrap();
    until("the action to reach the phone", || phone.saw(MPRIS_REQUEST).iter().any(|p| p.str("action") == Some("PlayPause") && p.str("player") == Some("Music"))).await;
    assert!(computer.service.media_action(&pid, "Music", "Raise").is_err(), "only the six actions");
    assert!(computer.service.media_action(&pid, "Radio", "Play").is_err(), "only the phone's players");

    // --- R5-30: the computer's players on the phone, on a bus of its own ---------------------------------------------
    computers_players(&computer, &phone).await;

    // --- R5-31: the clicker on. Only while presenting, and only the three keys ----------------------------------------
    computer.service.set_media_switches(true, true);
    reconnected(&computer, &phone).await;
    until("the phone to see the presenter listed", || phone.device(&cid).takes(PRESENTER)).await;
    assert!(!phone.device(&cid).takes(KEYS), "the remote keyboard stays unlisted");
    assert!(features(&computer, &pid).contains(&"clicker".to_string()));
    link().send(&Packet::new(KEYS, json!({ "specialKey": 9 })));
    until("the key to be heard", || computer.saw(KEYS).len() >= 2).await;
    pause(150).await;
    assert!(clicks(&computer).is_empty(), "not presenting: no click");
    // The module says it presents (as its connection would on the bus)
    present(&computer.service, true);
    for (body, expected) in [
        (json!({ "specialKey": 9 }), Some("next")),
        (json!({ "specialKey": 8 }), Some("previous")),
        (json!({ "key": "b" }), Some("blank")),
        (json!({ "key": "x" }), None),
        (json!({ "specialKey": 9, "ctrl": true }), None),
        (json!({ "dx": 5.0, "dy": 2.0 }), None),
    ] {
        let before = clicks(&computer).len();
        let heard = computer.saw(KEYS).len();
        link().send(&Packet::new(KEYS, body.clone()));
        until("the key to be heard", || computer.saw(KEYS).len() > heard).await;
        pause(200).await;
        let now = clicks(&computer);
        match expected {
            Some(word) => assert_eq!(now.last(), Some(&(pid.clone(), word.to_string())), "{body}"),
            None => assert_eq!(now.len(), before, "{body} made a click"),
        }
    }
    // The presenter's pointer is never a click
    link().send(&Packet::new(PRESENTER, json!({ "dx": 3.0, "dy": 1.0 })));
    // Faster than one in 150 ms is one
    let before = clicks(&computer).len();
    link().send(&Packet::new(KEYS, json!({ "specialKey": 9 })));
    link().send(&Packet::new(KEYS, json!({ "specialKey": 9 })));
    pause(300).await;
    assert_eq!(clicks(&computer).len(), before + 1);
    // Presenting ends: nothing again
    present(&computer.service, false);
    let before = clicks(&computer).len();
    link().send(&Packet::new(KEYS, json!({ "specialKey": 9 })));
    pause(300).await;
    assert_eq!(clicks(&computer).len(), before);
    // The clicker off while presenting: nothing either
    present(&computer.service, true);
    computer.service.set_media_switches(true, false);
    reconnected(&computer, &phone).await;
    let link = phone.device(&cid).link().unwrap();
    let before = clicks(&computer).len();
    link.send(&Packet::new(KEYS, json!({ "specialKey": 9 })));
    pause(300).await;
    assert_eq!(clicks(&computer).len(), before);

    // --- Both off: nothing is watched, the players go ------------------------------------------------------------------
    computer.service.set_media_switches(false, false);
    reconnected(&computer, &phone).await;
    assert!(players(&computer, &pid).is_empty());
    assert!(!computer.service.watching_players());
    let _ = media::ACTIONS;
}

/// What the module's connection does with Presenting, without a bus: it is a client of the module's role, and says so
fn present(service: &Rc<Service>, on: bool) {
    let name = ":1.module";
    service.clients.borrow_mut().insert(name.to_string(), dbus::Role::Module);
    dbus::set_presenter(service, Some(name), dbus::Role::Module, on);
    assert_eq!(dbus::presenting(service), on);
}

const PLAYER_XML: &str = "<node>
  <interface name='org.mpris.MediaPlayer2'><property name='Identity' type='s' access='read'/></interface>
  <interface name='org.mpris.MediaPlayer2.Player'>
    <method name='Play'/><method name='Pause'/><method name='PlayPause'/><method name='Stop'/><method name='Next'/><method name='Previous'/>
    <method name='Seek'><arg type='x' direction='in'/></method>
    <property name='PlaybackStatus' type='s' access='read'/>
    <property name='Metadata' type='a{sv}' access='read'/>
    <property name='Position' type='x' access='read'/>
    <property name='CanPlay' type='b' access='read'/><property name='CanPause' type='b' access='read'/>
    <property name='CanGoNext' type='b' access='read'/><property name='CanGoPrevious' type='b' access='read'/>
    <property name='CanSeek' type='b' access='read'/>
  </interface>
</node>";

fn metadata(title: &str) -> glib::Variant {
    let mut m: HashMap<String, glib::Variant> = HashMap::new();
    m.insert("xesam:title".into(), title.to_variant());
    m.insert("xesam:artist".into(), vec!["Ada"].to_variant());
    m.insert("mpris:length".into(), 180_000_000i64.to_variant());
    m.to_variant()
}

async fn computers_players(computer: &Peer, phone: &Peer) {
    let Some(daemon) = Daemon::start() else {
        eprintln!("dbus-daemon is not here: the computer's players are not tried");
        return;
    };
    let cid = computer.id();
    let link = phone.device(&cid).link().unwrap();
    let server = daemon.connect().await;
    dbus::export(&computer.service, &server).unwrap();

    // A stand-in player: it says what it plays and writes down what it is told
    let player_conn = daemon.connect().await;
    let title = Rc::new(RefCell::new("Song A".to_string()));
    let calls: Rc<RefCell<Vec<String>>> = Rc::default();
    let node = gio::DBusNodeInfo::for_xml(PLAYER_XML).unwrap();
    let mut registrations = Vec::new();
    for interface in ["org.mpris.MediaPlayer2", "org.mpris.MediaPlayer2.Player"] {
        let (title, calls) = (title.clone(), calls.clone());
        registrations.push(
            player_conn
                .register_object("/org/mpris/MediaPlayer2", &node.lookup_interface(interface).unwrap())
                .method_call(move |_, _, _, _, method, _, invocation| {
                    calls.borrow_mut().push(method.to_string());
                    invocation.return_value(None);
                })
                .property(move |_, _, _, _, name| match name {
                    "Identity" => "Test Player".to_variant(),
                    "PlaybackStatus" => "Playing".to_variant(),
                    "Metadata" => metadata(&title.borrow()),
                    "Position" => 42_000_000i64.to_variant(),
                    _ => true.to_variant(),
                })
                .build()
                .unwrap(),
        );
    }
    let owner = gio::bus_own_name_on_connection(&player_conn, "org.mpris.MediaPlayer2.test", gio::BusNameOwnerFlags::NONE, |_, _| {}, |_, _| {});
    pause(300).await;

    let lists = || phone.saw(MPRIS).into_iter().filter(|p| p.body.contains_key("playerList")).collect::<Vec<_>>();
    link.send(&Packet::new(MPRIS_REQUEST, json!({ "requestPlayerList": true })));
    until("the computer's players on the phone", || lists().iter().any(|p| p.body["playerList"] == json!(["Test Player"]))).await;
    assert!(computer.service.watching_players());
    assert_eq!(lists().last().unwrap().bool("supportAlbumArtPayload"), Some(false), "no album art is offered");
    let states = || phone.saw(MPRIS).into_iter().filter(|p| p.str("player") == Some("Test Player")).collect::<Vec<_>>();
    link.send(&Packet::new(MPRIS_REQUEST, json!({ "player": "Test Player", "requestNowPlaying": true })));
    until("the player's state", || states().iter().any(|p| p.str("title") == Some("Song A"))).await;
    let state = states().last().unwrap().clone();
    assert_eq!((state.bool("isPlaying"), state.int("pos"), state.int("length"), state.str("artist")), (Some(true), Some(42_000), Some(180_000), Some("Ada")));
    assert!(state.body.get("albumArtUrl").is_none());
    link.send(&Packet::new(MPRIS_REQUEST, json!({ "player": "Test Player", "action": "PlayPause" })));
    link.send(&Packet::new(MPRIS_REQUEST, json!({ "player": "Test Player", "action": "Quit" })));
    until("the player to be told", || calls.borrow().iter().any(|c| c == "PlayPause")).await;
    pause(200).await;
    assert_eq!(calls.borrow().iter().filter(|c| !c.starts_with("Get")).count(), 1, "only the known action: {:?}", calls.borrow());
    // The player changes track: the phone is told by itself
    *title.borrow_mut() = "Song B".into();
    let mut changed: HashMap<String, glib::Variant> = HashMap::new();
    changed.insert("Metadata".into(), metadata("Song B"));
    player_conn
        .emit_signal(None, "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", "PropertiesChanged", Some(&("org.mpris.MediaPlayer2.Player", changed, Vec::<String>::new()).to_variant()))
        .unwrap();
    until("the new track on the phone", || states().iter().any(|p| p.str("title") == Some("Song B"))).await;
    // The player goes: the list is sent again, empty
    gio::bus_unown_name(owner);
    until("the list without it", || lists().last().is_some_and(|p| p.body["playerList"] == json!([]))).await;
    let _ = registrations;
    // Music off: nothing is watched any more
    computer.service.set_media_switches(false, false);
    assert!(!computer.service.watching_players());
    reconnected(computer, phone).await;
    computer.service.set_media_switches(true, false);
    reconnected(computer, phone).await;
    // The program is taken off the test bus before the bus goes
    *computer.service.bus.borrow_mut() = None;
    computer.service.sync_players();
    drop(server);
}
