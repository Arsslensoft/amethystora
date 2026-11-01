//! What Notes tells the desktop's search (the search module of Amethystora UI, while `ame desktop search notes` is on;
//! plans/ui-ideas/spec-later.md, R6-34): the titles of the notes and the open tasks whose title or tags hold every word
//! asked for, and nothing else, never a note's text. It is on the session bus as today.rs's object is, on the
//! connection Notes' application already has, at /org/amethystora/Notes/Search, only while the notes are open:
//! registered when they open, taken away when they lock and when Notes quits, so that locked or not running it says
//! nothing at all. Only GNOME Shell of this account may ask (caller.rs, as for Today); anyone else is answered as if
//! there were no such object. `Open` shows one of the items it found in Notes' window.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value;

use amethystora_ui::caller::Callers;
use amethystora_ui::glib::variant::ToVariant;
use amethystora_ui::{gio, glib};

use crate::data::{Fields, Kind, Store};
use crate::today::clean;

pub const PATH: &str = "/org/amethystora/Notes/Search";
pub const INTERFACE: &str = "org.amethystora.Notes.Search1";
/// At most this many items cross
const FOUND: usize = 8;
/// The words asked for, at most this long
const MOST_ASKED: usize = 200;
const NOT_THERE: &str = "org.freedesktop.DBus.Error.UnknownMethod";
const NOT_THERE_TEXT: &str = "Object does not exist at path “/org/amethystora/Notes/Search”";

const XML: &str = r#"<node>
  <interface name="org.amethystora.Notes.Search1">
    <method name="Find">
      <arg name="words" type="s" direction="in"/>
      <arg name="found" type="a(ssss)" direction="out"/>
    </method>
    <method name="Open">
      <arg name="kind" type="s" direction="in"/>
      <arg name="id" type="s" direction="in"/>
    </method>
  </interface>
</node>"#;

/// One item found: its id, its title as it may leave, "note" or "task", and where it is (a note's notebook, a task's
/// due day YYYY-MM-DD or its project), cleaned as a title is
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub kind: &'static str,
    pub place: String,
}

fn lower(text: &str) -> String {
    text.to_lowercase()
}

/// The notes (not in the trash) and the open tasks whose title, or a note's tag names, hold every word, notes first,
/// each group most recently changed first, at most eight. An item without a title is not listed.
pub fn find(store: &Store, words: &str) -> Vec<Hit> {
    let wanted: Vec<String> = words.chars().take(MOST_ASKED).collect::<String>().split_whitespace().map(lower).collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let holds = |text: &str| {
        let text = lower(text);
        wanted.iter().all(|word| text.contains(word.as_str()))
    };
    let title_of = |kind: Kind, id: &str| store.get(kind, id).map(|item| clean(item.title())).unwrap_or_default();
    let changed = |item: &&Value| -(item.num_or_zero("updated").max(item.num_or_zero("created")));
    let mut notes: Vec<&Value> = store
        .values(Kind::Notes)
        .filter(|note| !note.is("trashed") && !clean(note.title()).is_empty())
        .filter(|note| {
            let tags: Vec<String> = note.list("tags").iter().map(|tag| title_of(Kind::Tags, tag)).collect();
            holds(&format!("{} {}", note.title(), tags.join(" ")))
        })
        .collect();
    notes.sort_by(|a, b| changed(a).total_cmp(&changed(b)));
    let mut tasks: Vec<&Value> =
        store.values(Kind::Tasks).filter(|task| !task.is("done") && !clean(task.title()).is_empty() && holds(task.title())).collect();
    tasks.sort_by(|a, b| changed(a).total_cmp(&changed(b)));
    let mut found: Vec<Hit> = notes
        .into_iter()
        .map(|note| Hit {
            id: note.id().to_string(),
            title: clean(note.title()),
            kind: "note",
            place: note.opt("notebook").map(|id| title_of(Kind::Notebooks, id)).unwrap_or_default(),
        })
        .collect();
    found.extend(tasks.into_iter().map(|task| Hit {
        id: task.id().to_string(),
        title: clean(task.title()),
        kind: "task",
        place: match task.opt("due") {
            Some(due) => clean(due),
            None => task.opt("project").map(|id| title_of(Kind::Projects, id)).unwrap_or_default(),
        },
    }));
    found.truncate(FOUND);
    found
}

/// The answer, from the notes as they are now: None while they are locked
pub type Answer = Rc<dyn Fn(&str) -> Option<Vec<Hit>>>;
/// Shows an item in the window: false when it is not one the notes have
pub type Show = Rc<dyn Fn(&str, &str) -> bool>;

pub struct Search {
    connection: gio::DBusConnection,
    exe: String,
    name: Option<String>,
    answer: Answer,
    show: Show,
    callers: Callers,
    id: RefCell<Option<gio::RegistrationId>>,
}

impl Search {
    /// On `connection`, answering only callers that run `exe` and own `name`, when one is given
    pub fn new(connection: gio::DBusConnection, exe: &str, name: Option<&str>, answer: Answer, show: Show) -> Rc<Search> {
        Rc::new(Search {
            connection,
            exe: exe.to_string(),
            name: name.map(str::to_string),
            answer,
            show,
            callers: Callers::default(),
            id: RefCell::new(None),
        })
    }

    pub fn registered(&self) -> bool {
        self.id.borrow().is_some()
    }

    /// On the bus while the notes are open, off it otherwise
    pub fn set_open(self: &Rc<Self>, open: bool) {
        if open && !self.registered() {
            self.register();
        } else if !open {
            self.unregister();
        }
    }

    fn register(self: &Rc<Self>) {
        let node = gio::DBusNodeInfo::for_xml(XML).expect("the interface is valid XML");
        let interface = node.lookup_interface(INTERFACE).expect("the interface is in it");
        self.callers.watch(&self.connection);
        let weak = Rc::downgrade(self);
        let result = self
            .connection
            .register_object(PATH, &interface)
            .method_call(move |connection, sender, _, _, method, parameters, invocation| {
                let Some(this) = weak.upgrade() else {
                    return invocation.return_dbus_error(NOT_THERE, NOT_THERE_TEXT);
                };
                let sender = sender.unwrap_or_default().to_string();
                let method = method.to_string();
                glib::spawn_future_local(async move {
                    if !this.callers.check(&connection, &sender, &this.exe, this.name.as_deref()).await {
                        return invocation.return_dbus_error(NOT_THERE, NOT_THERE_TEXT);
                    }
                    match method.as_str() {
                        "Find" => {
                            let Some((words,)) = parameters.get::<(String,)>() else {
                                return invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "a string");
                            };
                            match (this.answer)(&words) {
                                Some(found) => {
                                    let found: Vec<(String, String, String, String)> =
                                        found.into_iter().map(|hit| (hit.id, hit.title, hit.kind.to_string(), hit.place)).collect();
                                    invocation.return_value(Some(&(found,).to_variant()))
                                }
                                None => invocation.return_dbus_error(NOT_THERE, NOT_THERE_TEXT),
                            }
                        }
                        "Open" => {
                            let Some((kind, id)) = parameters.get::<(String, String)>() else {
                                return invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "two strings");
                            };
                            if !matches!(kind.as_str(), "note" | "task") || !crate::data::is_id(&id) {
                                return invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "a note or a task");
                            }
                            if (this.show)(&kind, &id) {
                                invocation.return_value(None)
                            } else {
                                invocation.return_dbus_error(NOT_THERE, NOT_THERE_TEXT)
                            }
                        }
                        _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", "no such method"),
                    }
                });
            })
            .build();
        match result {
            Ok(id) => {
                self.id.replace(Some(id));
            }
            Err(error) => eprintln!("amethystora-notes: the search is not on the bus: {}", error.message()),
        }
    }

    pub fn unregister(&self) {
        if let Some(id) = self.id.take() {
            let _ = self.connection.unregister_object(id);
        }
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        self.unregister();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store() -> Store {
        let mut store = Store::empty();
        store.put(Kind::Notebooks, json!({"id": "a".repeat(32), "title": "Home"}));
        store.put(Kind::Tags, json!({"id": "b".repeat(32), "title": "garage"}));
        store.put(Kind::Notes, json!({"id": "c".repeat(32), "title": "Car: what they said", "notebook": "a".repeat(32),
            "tags": ["b".repeat(32)], "body": "the brakes need a garage visit", "updated": 2.0}));
        store.put(Kind::Notes, json!({"id": "d".repeat(32), "title": "Garage receipts", "trashed": 1, "updated": 3.0}));
        store.put(Kind::Notes, json!({"id": "e".repeat(32), "title": "Brakes", "body": "garage", "updated": 1.0}));
        store.put(Kind::Tasks, json!({"id": "f".repeat(32), "title": "Call the garage", "due": "2026-10-31", "updated": 1.0}));
        store.put(Kind::Tasks, json!({"id": "0".repeat(32), "title": "Garage: pay", "done": 1}));
        store
    }

    #[test]
    fn titles_and_tags_never_bodies() {
        let found = find(&store(), "GARAGE");
        let titles: Vec<&str> = found.iter().map(|hit| hit.title.as_str()).collect();
        // The tagged note and the open task; not the trashed note, the done task, or the note whose body says garage
        assert_eq!(titles, ["Car: what they said", "Call the garage"]);
        assert_eq!(found[0].kind, "note");
        assert_eq!(found[0].place, "Home");
        assert_eq!(found[1].kind, "task");
        assert_eq!(found[1].place, "2026-10-31");
    }

    #[test]
    fn every_word_and_at_most_eight() {
        assert_eq!(find(&store(), "call garage").len(), 1);
        assert!(find(&store(), "   ").is_empty());
        let mut many = Store::empty();
        for i in 0..20 {
            many.put(Kind::Notes, json!({"id": format!("{i:032}"), "title": format!("Note {i}")}));
        }
        assert_eq!(find(&many, "note").len(), FOUND);
    }
}
