//! Where a button of this window is, for the Manual's Show me (R7-61), which puts a ring round it from the shell. Only the
//! window knows: the accessibility tree GTK offers counts each widget from its parent's contents, leaving out every padding
//! on the way, so that its answer is off by those. It answers one question, and presses nothing.
//!
//! The question is open to every program of the account on the session bus, so it is answered only as far as the guides need:
//! a name that is one of this app's own messages (its catalog, never a title or a note a person wrote), the button it is on
//! (never a label in a page's text), and only the part of it that is in view.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::prelude::*;

pub const PATH: &str = "/org/amethystora/Where";
const INTERFACE: &str = r#"<node>
  <interface name="org.amethystora.Where">
    <method name="Find">
      <arg type="s" direction="in" name="name"/>
      <arg type="d" direction="out" name="x"/>
      <arg type="d" direction="out" name="y"/>
      <arg type="d" direction="out" name="width"/>
      <arg type="d" direction="out" name="height"/>
    </method>
  </interface>
</node>"#;
const NOT_FOUND: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);

/// Every message the app's own catalog lists, which is what the English file `locale/en.json` holds
fn messages(folder: &Path) -> HashSet<String> {
    std::fs::read_to_string(folder.join("en.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text).ok())
        .map(|map| map.into_iter().map(|(key, _)| key).collect())
        .unwrap_or_default()
}

/// The first button on screen with one of these words on it, in the order of the window
fn button_with(top: &gtk::Widget, wanted: &[String]) -> Option<gtk::Widget> {
    let mut child = top.first_child();
    while let Some(widget) = child {
        if widget.is_drawable() {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                if wanted.iter().any(|name| *name == label.text()) {
                    if let Some(button) = widget.ancestor(gtk::Button::static_type()) {
                        return Some(button);
                    }
                }
            }
            if let Some(found) = button_with(&widget, wanted) {
                return Some(found);
            }
        }
        child = widget.next_sibling();
    }
    None
}

/// The widget's place in the window, less what a scrolled window around it hides; None when none of it is in view
fn visible_bounds(widget: &gtk::Widget, window: &gtk::Window) -> Option<gtk::graphene::Rect> {
    let mut bounds = widget.compute_bounds(window)?;
    let mut parent = widget.parent();
    while let Some(ancestor) = parent {
        if ancestor.is::<gtk::ScrolledWindow>() {
            bounds = bounds.intersection(&ancestor.compute_bounds(window)?)?;
        }
        parent = ancestor.parent();
    }
    (bounds.width() > 0.0 && bounds.height() > 0.0).then_some(bounds)
}

/// In pixels from the corner of the window's surface, its shadow included, which is what the shell knows as its buffer. Nothing
/// has a size of none when nothing was found.
fn find(window: &gtk::Window, name: &str) -> (f64, f64, f64, f64) {
    let wanted = [crate::i18n::dynamic(name), name.to_string()];
    let Some(widget) = button_with(window.upcast_ref(), &wanted) else { return NOT_FOUND };
    let Some(bounds) = visible_bounds(&widget, window) else { return NOT_FOUND };
    let (x, y) = window.surface_transform();
    (f64::from(bounds.x()) + x, f64::from(bounds.y()) + y, f64::from(bounds.width()), f64::from(bounds.height()))
}

/// On the app's own connection, for as long as the program runs: a window's whole cost, until the shell asks, is this object.
/// `catalogs` is the folder of the app's catalogs, whose English file lists the only names that are answered.
pub fn offer(app: &impl IsA<gio::Application>, window: &gtk::Window, catalogs: PathBuf) {
    let Some(bus) = app.as_ref().dbus_connection() else { return };
    let Ok(node) = gio::DBusNodeInfo::for_xml(INTERFACE) else { return };
    let Some(interface) = node.lookup_interface("org.amethystora.Where") else { return };
    let weak = window.downgrade();
    // Read when first asked, and kept: nothing is read for a window nobody asks about
    let known: RefCell<Option<HashSet<String>>> = RefCell::new(None);
    let _ = bus
        .register_object(PATH, &interface)
        .method_call(move |_, _, _, _, method, params, invocation| {
            let name = params.child_value(0).str().unwrap_or_default().to_string();
            match weak.upgrade() {
                Some(window) if method == "Find" => {
                    let listed = known.borrow_mut().get_or_insert_with(|| messages(&catalogs)).contains(&name);
                    let answer = if listed { find(&window, &name) } else { NOT_FOUND };
                    invocation.return_value(Some(&answer.to_variant()));
                }
                _ => invocation.return_dbus_error("org.amethystora.Where.Error", "not here"),
            }
        })
        .build();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_apps_own_messages_are_answered() {
        let folder = std::env::temp_dir().join(format!("where-is-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a folder");
        std::fs::write(folder.join("en.json"), r#"{"Appearance": "Appearance", "Status": "Status"}"#).expect("a catalog");
        let known = messages(&folder);
        assert!(known.contains("Appearance") && known.contains("Status"));
        // What a person wrote (a note's title, a file's name) is no message of the app's
        assert!(!known.contains("Taxes 2025"));
        // No catalog, no answer
        assert!(messages(&folder.join("missing")).is_empty());
        std::fs::remove_dir_all(&folder).expect("cleaned up");
    }
}
