//! Back in time, visibly (round 7, R7-28): a folder's past as windows receding. The folder as it was at the point chosen is
//! the front one, and it is the very list the window always shows, with its ticks and Put back; the earlier points stand
//! behind it, each a little smaller and fainter, and the timeline stays at the side. The wheel or Page Up and Page Down walk
//! the timeline, and a window behind is a click away.
//!
//! It reads nothing of its own: a point is one `points` gave, and the folder is read through `ls` when the walk settles on
//! it, as choosing a point always did. Nothing moves but the windows' transforms, for the length of one animation; with
//! animations off they simply stand in their new places. Leaving it is the window as it was.

use std::cell::{Cell, RefCell};
use std::rc::Weak;
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{adw, glib, graphene, gsk, gtk, pango, ui};
use gtk::subclass::prelude::*;

use crate::Backups;

/// How many windows stand behind the front one
pub const BEHIND: usize = 3;
/// How far up each window behind stands, and how much smaller and fainter it is
const STEP: f32 = 22.0;
const SMALLER: f32 = 0.05;
const FAINTER: f64 = 0.24;
const MOTION: u32 = 260;
/// A walk this still is one that has arrived: only then is the folder read
const SETTLE: Duration = Duration::from_millis(180);

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Receding {
        pub front: RefCell<Option<gtk::Widget>>,
        pub behind: RefCell<Vec<gtk::Box>>,
        /// How far the windows behind still have to come: 1 just after a step older, -1 after a step newer, then 0
        pub offset: Cell<f64>,
        pub animation: RefCell<Option<adw::TimedAnimation>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Receding {
        const NAME: &'static str = "AmethystoraBackupsReceding";
        type Type = super::Receding;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Receding {
        fn dispose(&self) {
            self.animation.take();
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Receding {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let room = (STEP * BEHIND as f32) as i32;
            let Some(front) = self.front.borrow().clone() else { return (room, room, -1, -1) };
            let (min, nat, _, _) = front.measure(orientation, if orientation == gtk::Orientation::Vertical { for_size } else { -1 });
            if orientation == gtk::Orientation::Vertical { (min + room, nat + room, -1, -1) } else { (min, nat, -1, -1) }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let room = STEP * BEHIND as f32;
            let card = (height as f32 - room).max(1.0) as i32;
            let offset = self.offset.get() as f32;
            for (at, window) in self.behind.borrow().iter().enumerate() {
                let depth = at as f32 + 1.0 + offset;
                let scale = (1.0 - SMALLER * depth).max(0.1);
                let x = width as f32 * (1.0 - scale) / 2.0;
                let y = room - STEP * depth;
                window.set_opacity((1.0 - FAINTER * depth as f64).clamp(0.0, 1.0));
                let transform = gsk::Transform::new().translate(&graphene::Point::new(x, y)).scale(scale, scale);
                window.allocate(width, card, -1, Some(transform));
            }
            if let Some(front) = self.front.borrow().as_ref() {
                front.allocate(width, card, -1, Some(gsk::Transform::new().translate(&graphene::Point::new(0.0, room))));
            }
        }
    }
}

glib::wrapper! {
    /// The windows of the past: the deepest drawn first, the front one last
    pub struct Receding(ObjectSubclass<imp::Receding>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Receding {
    pub fn new(me: &Weak<Backups>) -> Receding {
        let this: Receding = glib::Object::new();
        this.set_vexpand(true);
        this.set_hexpand(true);
        this.add_css_class("receding");
        // The wheel walks the past wherever it is over these windows: the list keeps its scrollbar and its keys
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        let travelled = Cell::new(0.0);
        let weak = me.clone();
        wheel.connect_scroll(move |_, _, dy| {
            travelled.set(travelled.get() + dy);
            if travelled.get().abs() >= 1.0 {
                let steps = travelled.get().trunc() as i32;
                travelled.set(0.0);
                if let Some(backups) = weak.upgrade() {
                    backups.walk_past(steps);
                }
            }
            glib::Propagation::Stop
        });
        this.add_controller(wheel);
        this
    }

    /// What stands in front, which may already be there: the same widget stays put, with its scroll and selection
    pub fn set_front(&self, front: Option<&gtk::Widget>) {
        let imp = self.imp();
        if imp.front.borrow().as_ref() == front {
            return;
        }
        if let Some(old) = imp.front.take() {
            old.unparent();
        }
        if let Some(front) = front {
            front.insert_before(self, None::<&gtk::Widget>);
            imp.front.replace(Some(front.clone()));
        }
        self.queue_allocate();
    }

    /// The windows behind, nearest first, each named for its point, with what a click on it does
    pub fn set_behind(&self, names: &[String], me: &Weak<Backups>) {
        let imp = self.imp();
        for old in imp.behind.borrow_mut().drain(..) {
            old.unparent();
        }
        let front = imp.front.borrow().clone();
        for (at, name) in names.iter().enumerate().take(BEHIND) {
            let window = ui::vbox(0, &["past-window"]);
            let title = ui::line(name, &["past-title"]);
            title.set_ellipsize(pango::EllipsizeMode::End);
            window.append(&title);
            let click = gtk::GestureClick::new();
            let weak = me.clone();
            click.connect_released(move |_, _, _, _| {
                if let Some(backups) = weak.upgrade() {
                    backups.walk_past(at as i32 + 1);
                }
            });
            window.add_controller(click);
            // The deepest first: each new one goes behind the one before it, and all behind the front
            let before = imp.behind.borrow().last().map(|nearer: &gtk::Box| nearer.clone().upcast::<gtk::Widget>()).or_else(|| front.clone());
            window.insert_before(self, before.as_ref());
            imp.behind.borrow_mut().push(window);
        }
        self.queue_allocate();
    }

    /// One walk's step: the windows behind come forward (older) or go back (newer), or stand there with animations off
    pub fn stepped(&self, older: bool) {
        let imp = self.imp();
        let target = adw::CallbackAnimationTarget::new(glib::clone!(
            #[weak(rename_to = this)]
            self,
            move |value| {
                this.imp().offset.set(value);
                this.queue_allocate();
            }
        ));
        let from = if older { 1.0 } else { -1.0 };
        let animation = adw::TimedAnimation::new(self, from, 0.0, MOTION, target);
        animation.set_easing(adw::Easing::EaseOutCubic);
        animation.play();
        imp.animation.replace(Some(animation));
    }
}

impl Backups {
    /// Into the view, or out of it to the window as it was. The whole thing is this one flag
    pub(crate) fn set_past(&self, on: bool) {
        self.state.borrow_mut().past = on;
        if !on {
            // Leaving is leaving at once: a walk not yet settled does not move the point afterwards
            if let Some(timer) = self.past_timer.take() {
                timer.remove();
            }
            self.past_target.replace(None);
            self.parts.past.set_front(None);
        }
        self.render();
    }

    /// `steps` along the timeline, older when more than nothing: the windows move now, and the folder is read once the walk
    /// has settled, however fast the wheel went
    pub(crate) fn walk_past(&self, steps: i32) {
        let next = {
            let state = self.state.borrow();
            let Some(timeline) = state.timeline.as_ref() else { return };
            let at = self.past_target.borrow().clone().unwrap_or_else(|| state.point.clone());
            let here = timeline.points.iter().position(|point| point.id == at).unwrap_or(0) as i32;
            let last = timeline.points.len() as i32 - 1;
            let there = (here + steps).clamp(0, last.max(0));
            if there == here {
                return;
            }
            timeline.points[there as usize].id.clone()
        };
        self.past_target.replace(Some(next.clone()));
        self.parts.past.stepped(steps > 0);
        self.show_behind(&next);
        if let Some(timer) = self.past_timer.take() {
            timer.remove();
        }
        let weak = self.me.clone();
        let timer = glib::timeout_add_local_once(SETTLE, move || {
            let Some(this) = weak.upgrade() else { return };
            this.past_timer.take();
            this.past_target.replace(None);
            let id = next.clone();
            glib::spawn_future_local(async move {
                this.choose_point_quietly(&id).await;
            });
        });
        self.past_timer.replace(Some(timer));
    }

    /// The windows behind `point`: the next points of the timeline, each by its day and time
    pub(crate) fn show_behind(&self, point: &str) {
        let names: Vec<String> = {
            let state = self.state.borrow();
            let Some(timeline) = state.timeline.as_ref() else { return };
            let here = timeline.points.iter().position(|one| one.id == point).unwrap_or(0);
            timeline.points.iter().skip(here + 1).take(BEHIND).map(|one| crate::page::moment_of(one.time)).collect()
        };
        self.parts.past.set_behind(&names, &self.me);
    }
}
