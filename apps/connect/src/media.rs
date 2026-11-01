//! Music from either side, and the phone as a slide clicker (plans/ui-ideas/spec-phone.md, sections 0, 3 and 4): the
//! protocol's media packets and the three keys a presenter may send, read and written here. Nothing of the bus: the
//! computer's own players are `players.rs`'s, and what is done with a click is the shell's.
//!
//! The wire facts are the protocol's public documents' (the spec's section 0 says which); the names, the limits and
//! the choices are ours.

use std::collections::HashMap;

use glib::variant::ToVariant;
use serde_json::{Map, Value, json};

use crate::packet;
use crate::text;

/// Makes the phone ring, even on silent; a second one stops it
pub const RING: &str = "kdeconnect.findmyphone.request";
/// A device's players (`playerList`) or one player's state (`player`)
pub const MPRIS: &str = "kdeconnect.mpris";
/// Asks for the players or their state, and plays, pauses and skips
pub const MPRIS_REQUEST: &str = "kdeconnect.mpris.request";
/// The presenter's pointer: listed while the clicker is on, so that the phone offers its presenter, and never acted on
pub const PRESENTER: &str = "kdeconnect.presenter";
/// The protocol's only key packet, which the presenter's Next and Previous are taken to travel in (UNVERIFIED, the
/// spec's section 0). Never listed: listing it would offer the phone's remote touchpad and keyboard for this computer
pub const KEYS: &str = "kdeconnect.mousepad.request";

/// A phone's players kept, and the characters shown of a name, a title, an artist or an album
pub const PLAYERS_KEPT: usize = 8;
pub const TEXT_SHOWN: usize = 128;

/// What a player may be told to do, as both the protocol and MPRIS name it
pub const ACTIONS: &[&str] = &["Play", "Pause", "PlayPause", "Stop", "Next", "Previous"];

/// Text from a phone or a player as it may be shown: one clean line, cut
fn shown(text: &str) -> String {
    text::shorten(text::clean_line(text).trim(), TEXT_SHOWN)
}

// --- The phone's players ---------------------------------------------------------------------------------------------

/// One of the phone's players, as its last status said
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_previous: bool,
    /// Milliseconds; 0 when the phone has not said
    pub length: i64,
    pub position: i64,
}

impl Remote {
    /// The phone's status of it, field by field: a field it left out keeps its value
    fn apply(&mut self, body: &Map<String, Value>) {
        for (key, field) in [("title", &mut self.title), ("artist", &mut self.artist), ("album", &mut self.album)] {
            if let Some(value) = packet::str_of(body, key) {
                *field = shown(value);
            }
        }
        for (key, field) in [
            ("isPlaying", &mut self.playing),
            ("canPlay", &mut self.can_play),
            ("canPause", &mut self.can_pause),
            ("canGoNext", &mut self.can_next),
            ("canGoPrevious", &mut self.can_previous),
        ] {
            if let Some(value) = packet::bool_of(body, key) {
                *field = value;
            }
        }
        for (key, field) in [("length", &mut self.length), ("pos", &mut self.position)] {
            if let Some(value) = packet::int_of(body, key) {
                *field = value.max(0);
            }
        }
    }

    /// The device property's entry (`Players`, the contract)
    pub fn to_dict(&self) -> HashMap<String, glib::Variant> {
        let mut m = HashMap::new();
        for (key, value) in [("player", &self.name), ("title", &self.title), ("artist", &self.artist), ("album", &self.album)] {
            m.insert(key.to_string(), value.to_variant());
        }
        for (key, value) in [
            ("playing", self.playing),
            ("can_play", self.can_play),
            ("can_pause", self.can_pause),
            ("can_next", self.can_next),
            ("can_previous", self.can_previous),
        ] {
            m.insert(key.to_string(), value.to_variant());
        }
        m.insert("length".to_string(), self.length.to_variant());
        m.insert("position".to_string(), self.position.to_variant());
        m
    }
}

/// What a phone's media packet changed
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Heard {
    pub changed: bool,
    /// Players new in its list, whose state is to be asked for
    pub ask: Vec<String>,
}

/// A phone's players, in the order its list gives them
#[derive(Debug, Default)]
pub struct Phone {
    players: Vec<Remote>,
}

impl Phone {
    pub fn players(&self) -> &[Remote] {
        &self.players
    }

    pub fn knows(&self, name: &str) -> bool {
        self.players.iter().any(|p| p.name == name)
    }

    pub fn clear(&mut self) -> bool {
        let had = !self.players.is_empty();
        self.players.clear();
        had
    }

    /// A `kdeconnect.mpris` from the phone: its list of players, or one player's state, or both
    pub fn apply(&mut self, body: &Map<String, Value>) -> Heard {
        let mut heard = Heard::default();
        if let Some(list) = packet::strings_of(body, "playerList") {
            let mut names: Vec<String> = Vec::new();
            for name in list.iter().map(|n| shown(n)).filter(|n| !n.is_empty()) {
                if !names.contains(&name) && names.len() < PLAYERS_KEPT {
                    names.push(name);
                }
            }
            let mut kept = Vec::new();
            for name in &names {
                match self.players.iter().find(|p| &p.name == name) {
                    Some(p) => kept.push(p.clone()),
                    None => {
                        kept.push(Remote { name: name.clone(), ..Remote::default() });
                        heard.ask.push(name.clone());
                    }
                }
            }
            heard.changed = kept != self.players;
            self.players = kept;
        }
        if let Some(name) = packet::str_of(body, "player").map(shown).filter(|n| !n.is_empty()) {
            // A status for a player the list has not named yet is taken, while there is room
            if !self.knows(&name) && self.players.len() < PLAYERS_KEPT {
                self.players.push(Remote { name: name.clone(), ..Remote::default() });
                heard.changed = true;
            }
            if let Some(player) = self.players.iter_mut().find(|p| p.name == name) {
                let before = player.clone();
                player.apply(body);
                heard.changed |= *player != before;
            }
        }
        heard
    }
}

/// What we send the phone to ask for its players' state
pub fn ask_list() -> Value {
    json!({ "requestPlayerList": true })
}

pub fn ask_state(player: &str) -> Value {
    json!({ "player": player, "requestNowPlaying": true, "requestVolume": true })
}

/// An action for one of the phone's players, if it is one
pub fn action_request(player: &str, action: &str) -> Option<Value> {
    ACTIONS.contains(&action).then(|| json!({ "player": player, "action": action }))
}

// --- What the phone asks of the computer's players -------------------------------------------------------------------

/// A `kdeconnect.mpris.request` from the phone about this computer's players
#[derive(Debug, Default, PartialEq)]
pub struct Asked {
    pub player: Option<String>,
    pub list: bool,
    /// The player's state (now playing, or its volume: both are answered with the whole state)
    pub state: bool,
    pub action: Option<&'static str>,
    /// 0 to 1
    pub volume: Option<f64>,
    /// Relative, in microseconds
    pub seek: Option<i64>,
    /// Absolute, in milliseconds
    pub position: Option<i64>,
    pub loop_status: Option<&'static str>,
    pub shuffle: Option<bool>,
}

pub fn asked(body: &Map<String, Value>) -> Asked {
    let action = packet::str_of(body, "action").and_then(|a| ACTIONS.iter().copied().find(|known| *known == a));
    let loop_status = packet::str_of(body, "setLoopStatus").and_then(|s| ["None", "Track", "Playlist"].into_iter().find(|known| *known == s));
    Asked {
        player: packet::str_of(body, "player").map(str::to_string),
        list: packet::bool_of(body, "requestPlayerList").unwrap_or(false),
        state: packet::bool_of(body, "requestNowPlaying").unwrap_or(false) || packet::bool_of(body, "requestVolume").unwrap_or(false),
        action,
        volume: packet::int_of(body, "setVolume").filter(|v| (0..=100).contains(v)).map(|v| v as f64 / 100.0),
        // An hour either way is far more than any seek a phone's buttons make
        seek: packet::int_of(body, "Seek").filter(|s| s.abs() <= 3_600_000_000),
        position: packet::int_of(body, "SetPosition").filter(|p| *p >= 0),
        loop_status,
        shuffle: packet::bool_of(body, "setShuffle"),
    }
}

// --- The computer's players ------------------------------------------------------------------------------------------

/// One of this computer's MPRIS players, as its properties last said
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Local {
    pub identity: String,
    pub status: String,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub can_seek: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Microseconds, as MPRIS has them
    pub length: i64,
    pub position: i64,
    pub track: Option<String>,
    pub volume: Option<f64>,
    pub loop_status: Option<String>,
    pub shuffle: Option<bool>,
}

impl Local {
    /// Properties of `org.mpris.MediaPlayer2` or `org.mpris.MediaPlayer2.Player`, all of them or the changed ones
    pub fn apply(&mut self, props: &HashMap<String, glib::Variant>) {
        let text = |v: &glib::Variant| v.str().map(shown);
        for (name, value) in props {
            match name.as_str() {
                "Identity" => self.identity = text(value).unwrap_or_default(),
                "PlaybackStatus" => self.status = value.str().unwrap_or_default().to_string(),
                "CanPlay" => self.can_play = value.get().unwrap_or(false),
                "CanPause" => self.can_pause = value.get().unwrap_or(false),
                "CanGoNext" => self.can_next = value.get().unwrap_or(false),
                "CanGoPrevious" => self.can_previous = value.get().unwrap_or(false),
                "CanSeek" => self.can_seek = value.get().unwrap_or(false),
                "Volume" => self.volume = value.get::<f64>(),
                "LoopStatus" => self.loop_status = value.str().map(str::to_string),
                "Shuffle" => self.shuffle = value.get::<bool>(),
                "Position" => self.position = value.get::<i64>().unwrap_or(0),
                "Metadata" => self.metadata(value),
                _ => {}
            }
        }
    }

    fn metadata(&mut self, value: &glib::Variant) {
        let dict = glib::VariantDict::new(Some(value));
        let string = |key: &str| dict.lookup_value(key, None).and_then(|v| v.str().map(shown)).unwrap_or_default();
        self.title = string("xesam:title");
        self.album = string("xesam:album");
        self.artist = dict
            .lookup_value("xesam:artist", None)
            .and_then(|v| v.get::<Vec<String>>())
            .map(|a| shown(&a.join(", ")))
            .unwrap_or_default();
        // A length is an x by the specification, and some players send a t
        self.length = dict.lookup_value("mpris:length", None).and_then(|v| v.get::<i64>().or_else(|| v.get::<u64>().map(|u| u as i64))).unwrap_or(0).max(0);
        self.track = dict.lookup_value("mpris:trackid", None).and_then(|v| v.str().map(str::to_string));
    }

    /// The state the phone is sent: no album art, nothing of the computer's files
    pub fn status(&self, name: &str) -> Value {
        let mut body = json!({
            "player": name,
            "isPlaying": self.status == "Playing",
            "canPlay": self.can_play,
            "canPause": self.can_pause,
            "canGoNext": self.can_next,
            "canGoPrevious": self.can_previous,
            "canSeek": self.can_seek,
            "pos": self.position / 1000,
            "length": self.length / 1000,
            "title": self.title,
            "artist": self.artist,
            "album": self.album,
        });
        if let Some(map) = body.as_object_mut() {
            if let Some(volume) = self.volume {
                map.insert("volume".into(), json!((volume.clamp(0.0, 1.0) * 100.0).round() as i64));
            }
            if let Some(status) = &self.loop_status {
                map.insert("loopStatus".into(), json!(status));
            }
            if let Some(shuffle) = self.shuffle {
                map.insert("shuffle".into(), json!(shuffle));
            }
        }
        body
    }
}

/// The names the phone is given for the computer's players, by their bus names in order: each one's own name, a number
/// after it for the second of the same name, the bus name's last part for one with none
pub fn names(players: &[(String, String)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (bus, identity) in players {
        let base = if identity.is_empty() { bus.rsplit('.').next().unwrap_or(bus).to_string() } else { identity.clone() };
        let mut name = base.clone();
        let mut n = 2;
        while out.iter().any(|(_, taken)| taken == &name) {
            name = format!("{base} {n}");
            n += 1;
        }
        out.push((bus.clone(), name));
    }
    out
}

pub fn player_list(names: &[String]) -> Value {
    json!({ "playerList": names, "supportAlbumArtPayload": false })
}

// --- The clicker ---------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Next,
    Previous,
    Blank,
}

impl Click {
    /// The word of the contract's signal Clicker
    pub fn word(self) -> &'static str {
        match self {
            Click::Next => "next",
            Click::Previous => "previous",
            Click::Blank => "blank",
        }
    }
}

/// What a phone's packet is as a slide clicker: Next, Previous or Blank, and nothing else. A key with a modifier, any
/// pointer movement or button, any other key, and the presenter's pointer packet are none of the three.
pub fn clicker_of(kind: &str, body: &Map<String, Value>) -> Option<Click> {
    if kind != KEYS {
        return None;
    }
    const MODIFIERS_AND_BUTTONS: &[&str] = &["alt", "ctrl", "shift", "super", "singleclick", "doubleclick", "middleclick", "rightclick", "singlehold", "singlerelease", "scroll", "sendAck"];
    if MODIFIERS_AND_BUTTONS.iter().any(|k| packet::bool_of(body, k) == Some(true)) {
        return None;
    }
    let moved = |k: &str| body.get(k).and_then(Value::as_f64).is_some_and(|d| d != 0.0);
    if moved("dx") || moved("dy") {
        return None;
    }
    let key = packet::str_of(body, "key").unwrap_or("");
    match packet::int_of(body, "specialKey").unwrap_or(0) {
        // The table under kdeconnect.mousepad.echo: 9 Page_Down, 6 Right, 8 Page_Up, 4 Left
        9 | 6 if key.is_empty() => Some(Click::Next),
        8 | 4 if key.is_empty() => Some(Click::Previous),
        0 => match key {
            "b" | "B" | "." => Some(Click::Blank),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn a_phone_list_asks_for_the_new_players_only() {
        let mut phone = Phone::default();
        let heard = phone.apply(&body(json!({ "playerList": ["Music", "Podcasts", "Music", ""] })));
        assert_eq!(heard, Heard { changed: true, ask: vec!["Music".into(), "Podcasts".into()] });
        let again = phone.apply(&body(json!({ "playerList": ["Podcasts", "Music"] })));
        assert!(again.ask.is_empty());
        assert!(again.changed, "the order is the phone's");
        let fewer = phone.apply(&body(json!({ "playerList": ["Music"] })));
        assert!(fewer.changed && !phone.knows("Podcasts"));
    }

    #[test]
    fn a_status_updates_its_player_and_text_is_cleaned() {
        let mut phone = Phone::default();
        phone.apply(&body(json!({ "playerList": ["Music"] })));
        let heard = phone.apply(&body(json!({ "player": "Music", "title": "Night\u{202e} Drive\n", "artist": "The Band", "isPlaying": true, "length": 200000, "pos": 1000, "canGoNext": true })));
        assert!(heard.changed);
        let p = &phone.players()[0];
        assert_eq!((p.title.as_str(), p.artist.as_str(), p.playing, p.length, p.position, p.can_next), ("Night Drive", "The Band", true, 200000, 1000, true));
        // A field left out keeps its value; the same status again changes nothing
        assert!(!phone.apply(&body(json!({ "player": "Music", "isPlaying": true }))).changed);
        assert_eq!(phone.players()[0].title, "Night Drive");
        let long = "x".repeat(500);
        phone.apply(&body(json!({ "player": "Music", "title": long })));
        assert_eq!(phone.players()[0].title.chars().count(), TEXT_SHOWN);
    }

    #[test]
    fn at_most_eight_players_are_kept() {
        let mut phone = Phone::default();
        let list: Vec<String> = (0..20).map(|i| format!("p{i}")).collect();
        phone.apply(&body(json!({ "playerList": list })));
        assert_eq!(phone.players().len(), PLAYERS_KEPT);
        assert!(!phone.apply(&body(json!({ "player": "another", "title": "t" }))).changed);
    }

    #[test]
    fn only_the_six_actions_are_sent() {
        assert_eq!(action_request("Music", "PlayPause"), Some(json!({ "player": "Music", "action": "PlayPause" })));
        assert_eq!(action_request("Music", "Quit"), None);
        assert_eq!(action_request("Music", "playpause"), None);
    }

    #[test]
    fn the_phone_s_requests_are_read_with_their_limits() {
        let a = asked(&body(json!({ "player": "Firefox", "requestNowPlaying": true, "action": "Next", "setVolume": 40, "Seek": 5000000 })));
        assert_eq!(a.player.as_deref(), Some("Firefox"));
        assert!(a.state && !a.list);
        assert_eq!((a.action, a.volume, a.seek), (Some("Next"), Some(0.4), Some(5_000_000)));
        let b = asked(&body(json!({ "requestPlayerList": true, "action": "Raise", "setVolume": 300, "SetPosition": -1, "setLoopStatus": "Forever" })));
        assert!(b.list);
        assert_eq!((b.action, b.volume, b.position, b.loop_status), (None, None, None, None));
        assert!(asked(&body(json!({ "requestVolume": true }))).state);
    }

    #[test]
    fn a_local_player_is_read_from_its_properties() {
        let mut meta = HashMap::new();
        meta.insert("xesam:title".to_string(), "Episode 12".to_variant());
        meta.insert("xesam:artist".to_string(), vec!["Ada", "Grace"].to_variant());
        meta.insert("mpris:length".to_string(), 90_000_000i64.to_variant());
        meta.insert("mpris:trackid".to_string(), glib::variant::ObjectPath::try_from("/track/1").unwrap().to_variant());
        let mut props = HashMap::new();
        props.insert("Identity".to_string(), "Podcasts".to_variant());
        props.insert("PlaybackStatus".to_string(), "Playing".to_variant());
        props.insert("CanPause".to_string(), true.to_variant());
        props.insert("Volume".to_string(), 0.5f64.to_variant());
        props.insert("Position".to_string(), 3_000_000i64.to_variant());
        props.insert("Metadata".to_string(), meta.to_variant());
        let mut local = Local::default();
        local.apply(&props);
        assert_eq!(local.track.as_deref(), Some("/track/1"));
        let status = local.status("Podcasts");
        assert_eq!(status["title"], "Episode 12");
        assert_eq!(status["artist"], "Ada, Grace");
        assert_eq!((status["isPlaying"].as_bool(), status["length"].as_i64(), status["pos"].as_i64(), status["volume"].as_i64()), (Some(true), Some(90_000), Some(3000), Some(50)));
        assert!(status.get("albumArtUrl").is_none(), "nothing of the computer's files");
    }

    #[test]
    fn players_of_the_same_name_are_numbered() {
        let got = names(&[("org.mpris.MediaPlayer2.a".into(), "Player".into()), ("org.mpris.MediaPlayer2.b".into(), "Player".into()), ("org.mpris.MediaPlayer2.vlc".into(), String::new())]);
        let shown: Vec<&str> = got.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(shown, ["Player", "Player 2", "vlc"]);
    }

    #[test]
    fn the_clicker_takes_three_keys_and_nothing_else() {
        let k = |v: Value| clicker_of(KEYS, &body(v));
        assert_eq!(k(json!({ "specialKey": 9 })), Some(Click::Next));
        assert_eq!(k(json!({ "specialKey": 6 })), Some(Click::Next));
        assert_eq!(k(json!({ "specialKey": 8 })), Some(Click::Previous));
        assert_eq!(k(json!({ "specialKey": 4 })), Some(Click::Previous));
        assert_eq!(k(json!({ "key": "b" })), Some(Click::Blank));
        assert_eq!(k(json!({ "key": "." })), Some(Click::Blank));
        for refused in [
            json!({ "key": "x" }),
            json!({ "key": "bb" }),
            json!({ "specialKey": 12 }),
            json!({ "specialKey": 25 }),
            json!({ "specialKey": 14 }),
            json!({ "specialKey": 9, "ctrl": true }),
            json!({ "key": "b", "super": true }),
            json!({ "specialKey": 9, "key": "a" }),
            json!({ "dx": 3.5, "dy": 0 }),
            json!({ "singleclick": true }),
            json!({ "specialKey": 9, "dx": 1 }),
            json!({}),
        ] {
            assert_eq!(k(refused.clone()), None, "{refused}");
        }
        assert_eq!(clicker_of(PRESENTER, &body(json!({ "dx": 1, "dy": 1 }))), None);
        assert_eq!(clicker_of(PRESENTER, &body(json!({ "specialKey": 9 }))), None);
    }
}
