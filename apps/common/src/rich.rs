//! A paragraph of Markdown: a GtkLabel, which wraps, selects and follows links, with what Pango markup cannot
//! draw drawn behind its text: the rounded, padded background of inline code, the keycaps of a key, and the
//! chip of an attachment with its clip, as the pages' `code`, `kbd` and `a.attachment` were. Their colours
//! are the stylesheet's, read from probes that are styled but never shown (`.probe-code`, `.probe-key`,
//! `.probe-key-line`, `.probe-chip`, `.probe-chip-line`, `.probe-chip-icon`), so they follow the theme.
//!
//! The padding is room the text makes for itself: thin spaces either side, inside the range, which a copy of
//! the selection leaves out again.

use std::cell::RefCell;
use std::rc::Rc;

use crate::icon::IconData;
use crate::{glib, graphene, gsk, gtk, pango};
use gtk::prelude::*;
use gtk::subclass::prelude::*;

/// The room either side of inline code and of a key, part of the text the background is drawn behind
pub const PAD: &str = "\u{2009}";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Inline code, its background rounded by so many pixels
    Code(f32),
    Key,
    /// An attachment, clicked as a whole, with an icon at its start
    Chip,
}

/// A range of the label's text, in bytes, drawn as code, a key or a chip; a chip's click goes to `target`
#[derive(Debug, Clone)]
pub struct Deco {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
    pub target: Option<String>,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Rich {
        pub label: RefCell<Option<gtk::Label>>,
        pub decos: RefCell<Vec<Deco>>,
        pub probes: RefCell<Vec<gtk::Widget>>,
        pub chip_icon: RefCell<Option<&'static IconData>>,
        pub on_chip: RefCell<Option<Rc<dyn Fn(&str)>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Rich {
        const NAME: &'static str = "AmethystoraRichText";
        type Type = super::Rich;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("richtext");
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Rich {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Rich {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let Some(label) = self.label.borrow().clone() else { return };
            let decos = self.decos.borrow();
            if !decos.is_empty() {
                let probes = self.probes.borrow();
                let color = |i: usize| probes[i].color();
                let layout = label.layout();
                let (ox, oy) = label.layout_offsets();
                // The label's own offset inside this widget
                let origin = label.compute_point(&*widget, &graphene::Point::new(0.0, 0.0)).unwrap_or(graphene::Point::zero());
                for deco in decos.iter() {
                    for (i, rect) in rects(&layout, deco.start, deco.end).into_iter().enumerate() {
                        let x = origin.x() + ox as f32 + rect.0;
                        let y = origin.y() + oy as f32 + rect.1;
                        let (w, h) = (rect.2, rect.3);
                        match deco.kind {
                            Kind::Code(radius) => {
                                let bounds = graphene::Rect::new(x, y + 1.0, w, h - 2.0);
                                snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(bounds, radius));
                                snapshot.append_color(&color(0), &bounds);
                                snapshot.pop();
                            }
                            Kind::Key => {
                                // border: 1px, its bottom 2px; border-radius: 6px
                                let bounds = graphene::Rect::new(x, y, w, h);
                                snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(bounds, 6.0));
                                snapshot.append_color(&color(2), &bounds);
                                let inner = graphene::Rect::new(x + 1.0, y + 1.0, w - 2.0, h - 3.0);
                                snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(inner, 5.0));
                                snapshot.append_color(&color(1), &inner);
                                snapshot.pop();
                                snapshot.pop();
                            }
                            Kind::Chip => {
                                // border: 1px solid var(--line); border-radius: 8px; background: var(--control)
                                let bounds = graphene::Rect::new(x, y + 1.0, w, h - 2.0);
                                snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(bounds, 8.0));
                                snapshot.append_color(&color(4), &bounds);
                                let inner = graphene::Rect::new(x + 1.0, y + 2.0, w - 2.0, h - 4.0);
                                snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(inner, 7.0));
                                snapshot.append_color(&color(3), &inner);
                                snapshot.pop();
                                snapshot.pop();
                                // Its clip, 14 px, 9 px in from its start
                                if let (0, Some(icon)) = (i, *self.chip_icon.borrow()) {
                                    draw_icon(snapshot, icon, x + 9.0, y + (h - 14.0) / 2.0, 14.0, &color(5));
                                }
                            }
                        }
                    }
                }
            }
            widget.snapshot_child(&label, snapshot);
        }
    }
}

glib::wrapper! {
    pub struct Rich(ObjectSubclass<imp::Rich>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// A line icon at a place, in a colour, as icon::Icon draws one
fn draw_icon(snapshot: &gtk::Snapshot, icon: &IconData, x: f32, y: f32, size: f32, color: &crate::gdk::RGBA) {
    snapshot.save();
    snapshot.translate(&graphene::Point::new(x, y));
    snapshot.scale(size / icon.view, size / icon.view);
    for shape in icon.shapes {
        let Ok(path) = gsk::Path::parse(shape.d) else { continue };
        if shape.fill {
            snapshot.append_fill(&path, gsk::FillRule::Winding, color);
        }
        if shape.stroke {
            let stroke = gsk::Stroke::new(shape.width.unwrap_or(1.8));
            stroke.set_line_cap(gsk::LineCap::Round);
            stroke.set_line_join(gsk::LineJoin::Round);
            snapshot.append_stroke(&path, &stroke, color);
        }
    }
    snapshot.restore();
}

/// Where a range of the layout's text is, one rectangle per line it is on, in pixels from the layout's
/// origin: (x, y, width, height)
fn rects(layout: &pango::Layout, start: usize, end: usize) -> Vec<(f32, f32, f32, f32)> {
    let mut out = Vec::new();
    let mut iter = layout.iter();
    loop {
        if let Some(line) = iter.line_readonly() {
            let line_start = line.start_index() as usize;
            let line_end = line_start + line.length() as usize;
            if start < line_end && end > line_start {
                let (_, logical) = iter.line_extents();
                let from = start.max(line_start) as i32;
                let to = end.min(line_end) as i32;
                for pair in line.x_ranges(from, to).chunks(2) {
                    if let [x0, x1] = pair {
                        let scale = pango::SCALE as f32;
                        out.push(((*x0).min(*x1) as f32 / scale, logical.y() as f32 / scale, (x1 - x0).abs() as f32 / scale, logical.height() as f32 / scale));
                    }
                }
            }
        }
        if !iter.next_line() {
            break;
        }
    }
    out
}

/// The text as it was written: without the room made around code, keys and chips, and with the spaces and
/// hyphens kept from breaking in a table as the plain ones they are
pub fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| *c != '\u{2009}' && *c != '\u{200a}')
        .map(|c| match c {
            '\u{a0}' => ' ',
            '\u{2011}' => '-',
            other => other,
        })
        .collect()
}

impl Rich {
    /// A paragraph of `markup`, with `decos` in byte ranges of its text
    pub fn new(markup: &str, decos: Vec<Deco>, classes: &[&str]) -> Rich {
        let rich: Rich = glib::Object::new();
        let label = gtk::Label::builder()
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .natural_wrap_mode(gtk::NaturalWrapMode::None)
            .xalign(0.0)
            .halign(gtk::Align::Fill)
            .selectable(true)
            .use_markup(true)
            .build();
        label.set_markup(markup);
        crate::ui::no_hyphens(&label);
        crate::ui::link_accessibles(&label);
        for class in classes {
            label.add_css_class(class);
        }
        label.set_parent(&rich);
        // Copying gives the text as it was written
        label.connect_copy_clipboard(|label| {
            if let Some((start, end)) = label.selection_bounds() {
                let text: String = label.text().chars().skip(start as usize).take((end - start) as usize).collect();
                label.clipboard().set_text(&plain(&text));
                label.stop_signal_emission_by_name("copy-clipboard");
            }
        });
        let mut probes = Vec::new();
        for class in ["probe-code", "probe-key", "probe-key-line", "probe-chip", "probe-chip-line", "probe-chip-icon"] {
            let probe = gtk::Label::new(None);
            probe.add_css_class(class);
            probe.set_child_visible(false);
            probe.set_parent(&rich);
            probes.push(probe.upcast::<gtk::Widget>());
        }
        if decos.iter().any(|deco| deco.kind == Kind::Chip) {
            // A chip is clicked as a whole, wherever in it
            let click = gtk::GestureClick::new();
            // Ahead of the label inside: a press on a chip is claimed, or the label takes it to select its text and
            // this gesture never hears the release. A press anywhere else stays the label's
            click.set_propagation_phase(gtk::PropagationPhase::Capture);
            let weak = rich.downgrade();
            click.connect_pressed(move |gesture, _, x, y| {
                if weak.upgrade().is_some_and(|rich| rich.chip_at(x, y).is_some()) {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            });
            let weak = rich.downgrade();
            click.connect_released(move |_, _, x, y| {
                let Some(rich) = weak.upgrade() else { return };
                if let Some(target) = rich.chip_at(x, y) {
                    if let Some(on_chip) = rich.imp().on_chip.borrow().clone() {
                        on_chip(&target);
                    }
                }
            });
            rich.add_controller(click);
        }
        rich.imp().label.replace(Some(label));
        rich.imp().decos.replace(decos);
        rich.imp().probes.replace(probes);
        rich
    }

    pub fn label(&self) -> gtk::Label {
        self.imp().label.borrow().clone().expect("made with one")
    }

    /// The icon a chip starts with, and what a click on one does with its target
    pub fn set_chips(&self, icon: Option<&'static IconData>, on_chip: Rc<dyn Fn(&str)>) {
        self.imp().chip_icon.replace(icon);
        self.imp().on_chip.replace(Some(on_chip));
    }

    fn chip_at(&self, x: f64, y: f64) -> Option<String> {
        let label = self.label();
        let point = self.compute_point(&label, &graphene::Point::new(x as f32, y as f32))?;
        let (ox, oy) = label.layout_offsets();
        let layout = label.layout();
        let (inside, index, _) = layout.xy_to_index((point.x() as i32 - ox) * pango::SCALE, (point.y() as i32 - oy) * pango::SCALE);
        if !inside {
            return None;
        }
        let index = index as usize;
        self.imp().decos.borrow().iter().find(|d| d.kind == Kind::Chip && d.start <= index && index < d.end).and_then(|d| d.target.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_what_was_written() {
        assert_eq!(plain("run \u{2009}ame\u{a0}update\u{2009} or \u{2009}x\u{2011}y\u{2009}"), "run ame update or x-y");
    }
}
