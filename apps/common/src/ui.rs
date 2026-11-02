//! Small builders for the pieces every page is made of. The CSS classes are the ones the Electron pages
//! gave the same pieces, so that each app's stylesheet, ported from its own, styles them as it did.
//! Nothing here has words of its own: every message is the app's, where check-translations.js finds it.

use std::cell::Cell;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::icon::Icon;

/// GTK's name for GNOME's Reduced Motion, `ame desktop comfort reduce-motion`, which it follows live from the portal.
/// By name, since the crate's own accessor needs GTK 4.22 to build
pub const REDUCED_MOTION: &str = "gtk-interface-reduced-motion";

/// Whether things may move: `gtk-enable-animations` is on and nobody asked for reduced motion, which it says nothing of
pub fn animations_on() -> bool {
    let Some(settings) = gtk::Settings::default() else { return true };
    settings.is_gtk_enable_animations()
        && (settings.find_property(REDUCED_MOTION).is_none()
            || glib::EnumValue::from_value(&settings.property_value(REDUCED_MOTION)).is_none_or(|(_, motion)| motion.value() == 0))
}

/// Text that wraps to the width it is given, starting where the line starts
pub fn label(text: &str, classes: &[&str]) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        // As wide as the whole line would be, and wrapping only when there is not room for it
        .natural_wrap_mode(gtk::NaturalWrapMode::None)
        .xalign(0.0)
        // Filling the line rather than at its natural width, which GTK measures without the stylesheet's
        // letter-spacing: at its natural width a spaced-out title would wrap its last word
        .halign(gtk::Align::Fill)
        .build();
    no_hyphens(&label);
    for class in classes {
        label.add_css_class(class);
    }
    label
}

/// A word broken at the end of a line is broken without a hyphen, as Chromium broke it: Pango adds one
pub fn no_hyphens(label: &gtk::Label) {
    let attributes = label.attributes().unwrap_or_default();
    attributes.insert(gtk::pango::AttrInt::new_insert_hyphens(false));
    label.set_attributes(Some(&attributes));
}

/// Makes the accessible objects of a label's links as soon as the label is in a window, for a label whose markup
/// has links. GTK 4.22 otherwise makes them when a screen reader first reaches the label, from inside the label's
/// own registration with the accessibility bus, which registers the label a second time under a second path: the
/// first outlives the label, and the next screen reader to ask for the window's objects (GetItems) aborts the app
/// once the label has gone, as a page that is left goes. Made first, they are only looked up then. Not before the
/// label is in a window: GTK expects no accessible of a widget outside one. Call it once the markup is set.
pub fn link_accessibles(label: &gtk::Label) {
    if !label.uses_markup() || !label.label().contains("<a ") {
        return;
    }
    label.connect_root_notify(|label| {
        if label.root().is_some() {
            // GTK names none of them, so a screen reader's list of links would be blank: each takes its words
            let mut link = label.first_accessible_child();
            for words in link_words(&label.label()) {
                let Some(this) = link else { break };
                this.update_property(&[gtk::accessible::Property::Label(&words)]);
                link = this.next_accessible_sibling();
            }
        }
    });
}

/// The words of each link in a label's markup, in order, as they read on the screen
fn link_words(markup: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut rest = markup;
    while let Some(start) = rest.find("<a ") {
        let Some(open) = rest[start..].find('>') else { break };
        let inside = &rest[start + open + 1..];
        let Some(end) = inside.find("</a>") else { break };
        // Parsed on its own, for what is inside it (code, an entity) to come out as text
        words.push(gtk::pango::parse_markup(&inside[..end], '\0').map(|(_, text, _)| text.to_string()).unwrap_or_default());
        rest = &inside[end + 4..];
    }
    words
}

/// Text on one line that never wraps, for a button or a state
pub fn line(text: &str, classes: &[&str]) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    for class in classes {
        label.add_css_class(class);
    }
    label
}

/// Pango markup that wraps, for a sentence with code or a link in it
pub fn markup(markup: &str, classes: &[&str]) -> gtk::Label {
    let label = self::label("", classes);
    label.set_markup(markup);
    link_accessibles(&label);
    label
}

pub fn vbox(spacing: i32, classes: &[&str]) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, spacing);
    for class in classes {
        container.add_css_class(class);
    }
    container
}

pub fn hbox(spacing: i32, classes: &[&str]) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, spacing);
    for class in classes {
        container.add_css_class(class);
    }
    container
}

/// Lines of children that wrap, as `display: flex; flex-wrap: wrap` did
pub fn wrap(spacing: i32, line_spacing: i32, classes: &[&str]) -> adw::WrapBox {
    let container = adw::WrapBox::builder().child_spacing(spacing).line_spacing(line_spacing).build();
    for class in classes {
        container.add_css_class(class);
    }
    container
}

/// Takes every child out of a box, to build it again
/// Work done a little at a time, between frames, so that a long page fills in while the window goes on answering: at
/// most `per_frame` steps a frame, and fewer when they take longer than a frame can spare. It ends with `on`.
pub fn in_frames(on: &impl IsA<gtk::Widget>, per_frame: usize, steps: std::collections::VecDeque<Box<dyn FnOnce()>>) {
    if steps.is_empty() {
        return;
    }
    let steps = std::cell::RefCell::new(steps);
    on.as_ref().add_tick_callback(move |widget, _| {
        if widget.root().is_none() {
            return glib::ControlFlow::Break;
        }
        let began = std::time::Instant::now();
        for _ in 0..per_frame {
            let step = steps.borrow_mut().pop_front();
            match step {
                Some(step) => step(),
                None => break,
            }
            if began.elapsed() > std::time::Duration::from_millis(5) {
                break;
            }
        }
        if steps.borrow().is_empty() { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
    });
}

/// A search's results, drawn once the typing has paused: laying out a page takes longer than a key takes to press.
/// A key that acts on the results asks `cancel` first, and draws them at once if something was waiting.
#[derive(Default)]
pub struct Pause(std::rc::Rc<Cell<Option<glib::SourceId>>>);

impl Pause {
    pub fn after_typing(&self, then: impl FnOnce() + 'static) {
        self.cancel();
        let slot = self.0.clone();
        let source = glib::timeout_add_local_once(std::time::Duration::from_millis(180), move || {
            slot.set(None);
            then();
        });
        self.0.set(Some(source));
    }

    /// Whether something was waiting
    pub fn cancel(&self) -> bool {
        let waiting = self.0.take();
        let was = waiting.is_some();
        if let Some(source) = waiting {
            source.remove();
        }
        was
    }
}

pub fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

pub fn clear_wrap(container: &adw::WrapBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// A button with an icon before its words, as the pages' `.btn` and `.link` were
pub fn button(text: &str, icon: Option<Icon>, classes: &[&str], on_click: impl Fn() + 'static) -> gtk::Button {
    let content = hbox(0, &["content"]);
    if let Some(icon) = icon {
        content.append(&icon);
    }
    let words = line(text, &[]);
    content.append(&words);
    let button = gtk::Button::builder().child(&content).build();
    for class in classes {
        button.add_css_class(class);
    }
    button.connect_clicked(move |_| on_click());
    button
}

/// The words of a button made by `button`, to change them in place
pub fn button_words(button: &gtk::Button) -> Option<gtk::Label> {
    let content = button.child()?;
    let mut child = content.first_child();
    while let Some(widget) = child {
        if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
            return Some(label);
        }
        child = widget.next_sibling();
    }
    None
}

/// "Copy", then "Copied" for a moment and the command on the clipboard
pub fn copy_button(command: &str, icon: Icon, copy: &str, copied: &str, label: &str, classes: &[&str]) -> gtk::Button {
    let command = command.to_string();
    let (copy, copied) = (copy.to_string(), copied.to_string());
    let button = button(&copy, Some(icon), classes, || {});
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.connect_clicked(move |button| {
        crate::process::copy(&command);
        if let Some(words) = button_words(button) {
            words.set_label(&copied);
            let copy = copy.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || words.set_label(&copy));
        }
    });
    button
}

/// Children joined in a box, skipping those that are not there
pub fn append_all(container: &gtk::Box, children: impl IntoIterator<Item = Option<gtk::Widget>>) {
    for child in children.into_iter().flatten() {
        container.append(&child);
    }
}

/// Some(widget) as a gtk::Widget, for append_all
pub fn some<W: IsA<gtk::Widget>>(widget: &W) -> Option<gtk::Widget> {
    Some(widget.clone().upcast())
}

// --- At most so wide ------------------------------------------------------------------------------------

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Limit {
        pub max: Cell<i32>,
        pub center: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Limit {
        const NAME: &'static str = "AmethystoraLimit";
        type Type = super::Limit;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("limit");
        }
    }

    impl ObjectImpl for Limit {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl Limit {
        fn max(&self) -> i32 {
            (self.max.get() as f64 * crate::icon::zoom()).round() as i32
        }
    }

    impl WidgetImpl for Limit {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.obj().first_child() else { return (0, 0, -1, -1) };
            if orientation == gtk::Orientation::Horizontal {
                let (min, nat, _, _) = child.measure(orientation, for_size);
                (min.min(self.max()), nat.min(self.max()).max(min.min(self.max())), -1, -1)
            } else {
                let width = if for_size < 0 { -1 } else { for_size.min(self.max()) };
                child.measure(orientation, width)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.obj().first_child() else { return };
            let inner = width.min(self.max());
            let rtl = self.obj().direction() == gtk::TextDirection::Rtl;
            let x = if self.center.get() { (width - inner) / 2 } else if rtl { width - inner } else { 0 };
            child.size_allocate(&gtk::Allocation::new(x, 0, inner, height), baseline);
        }
    }
}

glib::wrapper! {
    /// A child at most so many pixels wide, as `max-width` was: at the start of the line, or centred
    pub struct Limit(ObjectSubclass<imp::Limit>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Limit {
    pub fn new(child: &impl IsA<gtk::Widget>, max: i32, center: bool) -> Limit {
        let limit: Limit = glib::Object::new();
        limit.imp().max.set(max);
        limit.imp().center.set(center);
        child.set_parent(&limit);
        limit
    }
}

/// `child` at most `max` pixels wide, at the start of the line
pub fn limit(child: &impl IsA<gtk::Widget>, max: i32) -> Limit {
    Limit::new(child, max, false)
}

/// ...and centred
pub fn limit_center(child: &impl IsA<gtk::Widget>, max: i32) -> Limit {
    Limit::new(child, max, true)
}

// --- A progress bar ----------------------------------------------------------------------------------------

mod bar {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct Bar {
        pub fill: RefCell<Option<gtk::Box>>,
        /// What it shows, 0 to 1, or None while nothing is known of how far along it is
        pub target: Cell<Option<f64>>,
        pub shown: Cell<f64>,
        pub from: Cell<f64>,
        pub since: Cell<i64>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
        pub slide: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Bar {
        const NAME: &'static str = "AmethystoraBar";
        type Type = super::Bar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("bar");
            klass.set_accessible_role(gtk::AccessibleRole::ProgressBar);
        }
    }

    impl ObjectImpl for Bar {
        fn constructed(&self) {
            self.parent_constructed();
            let fill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            fill.add_css_class("fill");
            fill.set_parent(&*self.obj());
            self.fill.replace(Some(fill));
            self.obj().set_overflow(gtk::Overflow::Hidden);
        }

        fn dispose(&self) {
            if let Some(tick) = self.tick.take() {
                tick.remove();
            }
            if let Some(fill) = self.fill.take() {
                fill.unparent();
            }
        }
    }

    impl WidgetImpl for Bar {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Vertical {
                // Its height is the stylesheet's min-height
                (0, 0, -1, -1)
            } else {
                (0, 0, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(fill) = self.fill.borrow().clone() else { return };
            let (x, w) = match self.target.get() {
                Some(_) => (0.0, self.shown.get() * width as f64),
                // 34% wide, sliding from -100% to 300% of its own width
                None => {
                    let w = 0.34 * width as f64;
                    (-w + self.slide.get() * 4.0 * w, w)
                }
            };
            let (x, w) = (x.round() as i32, (w.round() as i32).max(0));
            if self.obj().direction() == gtk::TextDirection::Rtl {
                fill.size_allocate(&gtk::Allocation::new(width - x - w, 0, w, height), baseline);
            } else {
                fill.size_allocate(&gtk::Allocation::new(x, 0, w, height), baseline);
            }
        }
    }
}

glib::wrapper! {
    /// The pages' `.bar`: a trough and a fill the stylesheet colours, which moves to a new value over 0.4 s,
    /// and slides across while nothing is known of how far along it is
    pub struct Bar(ObjectSubclass<bar::Bar>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Bar {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Bar {
    pub fn new() -> Bar {
        Bar::default()
    }

    /// 0 to 1, or None for "somewhere along"
    pub fn set_fraction(&self, fraction: Option<f64>) {
        let imp = self.imp();
        let fraction = fraction.map(|f| f.clamp(0.0, 1.0));
        if imp.target.get() == fraction && imp.tick.borrow().is_some() == fraction.is_none() {
            return;
        }
        if fraction.is_none() {
            self.add_css_class("indeterminate");
        } else {
            self.remove_css_class("indeterminate");
        }
        if let Some(value) = fraction {
            self.update_property(&[gtk::accessible::Property::ValueNow(value * 100.0)]);
        }
        let was_indeterminate = imp.target.get().is_none() && imp.tick.borrow().is_some();
        imp.from.set(if was_indeterminate { 0.0 } else { imp.shown.get() });
        imp.target.set(fraction);
        imp.since.set(-1);
        if let Some(tick) = imp.tick.take() {
            tick.remove();
        }
        let animations = animations_on();
        if !animations {
            imp.shown.set(fraction.unwrap_or(0.0));
            imp.slide.set(0.375);
            self.queue_allocate();
            return;
        }
        let id = self.add_tick_callback(|bar, clock| {
            let imp = bar.imp();
            if imp.since.get() < 0 {
                imp.since.set(clock.frame_time());
            }
            let seconds = (clock.frame_time() - imp.since.get()) as f64 / 1_000_000.0;
            match imp.target.get() {
                Some(target) => {
                    // transition: width 0.4s ease
                    let progress = crate::gem::cubic_bezier(0.25, 0.1, 0.25, 1.0, (seconds / 0.4).min(1.0));
                    imp.shown.set(imp.from.get() + (target - imp.from.get()) * progress);
                    bar.queue_allocate();
                    if seconds >= 0.4 {
                        imp.tick.take();
                        return glib::ControlFlow::Break;
                    }
                }
                None => {
                    // animation: slide 1.7s cubic-bezier(0.45, 0.05, 0.55, 0.95) infinite
                    imp.slide.set(crate::gem::cubic_bezier(0.45, 0.05, 0.55, 0.95, (seconds % 1.7) / 1.7));
                    bar.queue_allocate();
                }
            }
            glib::ControlFlow::Continue
        });
        imp.tick.replace(Some(id));
    }
}

#[cfg(test)]
mod tests {
    use super::link_words;

    #[test]
    fn a_link_is_named_by_its_words() {
        let markup = r#"See <a href="updates.md#a">Rolling <tt>back</tt></a>, <a href="b.md" title="x">Q&amp;A</a> and <b>more</b>."#;
        assert_eq!(link_words(markup), ["Rolling back", "Q&A"]);
        assert!(link_words("no links").is_empty());
        assert!(link_words("<a href=\"x\">never closed").is_empty());
    }
}
