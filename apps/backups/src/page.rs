//! The page of Amethystora Backups, as backups.js drew it. What it shows is what backend.rs read from
//! /usr/libexec/amethystora-restore: the points in time there are to go back to, newest first, a folder as
//! it was at one of them, the versions of a file, and what is set up. It sends back only those points and
//! paths, and one of three ways to put back, and backend.rs checks each again. The helper's errors come as
//! words such as "not-found", which are said here in the session's language. Everything is built as widgets
//! with text in them, never as markup from outside.
//!
//! The folder's entries and the timeline are list views made once (list.rs), which a render binds again;
//! everything else is built again from the state on each render, as render() rebuilt the page's DOM.

use std::rc::{Rc, Weak};

use amethystora_ui::frame::{self, NavButton};
use amethystora_ui::gem::Gem;
use amethystora_ui::i18n;
use amethystora_ui::{adw, glib, gtk, pango, process, t, ui};
use gtk::prelude::*;

use crate::backend::{self, Failure, Point, Versions};
use crate::icons::icon;
use crate::layout::{AutoGrid, BrandFlow, Flex, Most, Pretty, Shift, Tight, GROW};
use crate::list::{self, bdi, day_of, size, source_icon, source_label, time, when, Asker, Tick};
use crate::relative::{relative, Unit};
use crate::{Backups, Banner, BackupRun, Dialog, Folder, PageState, VersionsState};

/// How a button looks, as the page's button() was told
#[derive(Debug, Clone, Copy, Default)]
pub struct Look {
    pub icon: Option<&'static str>,
    pub small: bool,
    pub primary: bool,
    pub disabled: bool,
}

/// The parts of the window made once
pub struct Parts {
    pub root: gtk::Overlay,
    pub browse_nav: NavButton,
    pub status_nav: NavButton,
    pub status_dot: gtk::Box,
    pub topbar: gtk::Box,
    pub banner: gtk::Box,
    pub content: gtk::Box,
    /// The status page, and the pages with nothing to browse, which scroll
    pub page: gtk::ScrolledWindow,
    /// The folder and the timeline beside it, which fill the window and scroll inside
    pub browse: gtk::Box,
    pub unreachable: gtk::Box,
    pub head: gtk::Box,
    pub body: gtk::Box,
    pub selection: gtk::Box,
    pub list_card: gtk::Box,
    pub list_scroller: gtk::ScrolledWindow,
    pub files: list::Files,
    pub count: gtk::Label,
    pub points: list::Points,
    pub points_scroller: gtk::ScrolledWindow,
    pub sticky: Shift,
    pub sticky_label: gtk::Label,
    /// Back in time, visibly: made once, and in the window only while it is chosen (past.rs)
    pub past: crate::past::Receding,
}

fn upgrade(me: &Weak<Backups>) -> Option<Rc<Backups>> {
    me.upgrade()
}

/// A box with a role a screen reader is told, which only a new widget can be given
fn role_box(orientation: gtk::Orientation, spacing: i32, classes: &[&str], role: gtk::AccessibleRole) -> gtk::Box {
    let container = gtk::Box::builder().orientation(orientation).spacing(spacing).accessible_role(role).build();
    for class in classes {
        container.add_css_class(class);
    }
    container
}

/// A heading, which wraps as ui::label's text does; its role can only be given as it is made
fn heading(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .accessible_role(gtk::AccessibleRole::Heading)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .natural_wrap_mode(gtk::NaturalWrapMode::None)
        .xalign(0.0)
        .halign(gtk::Align::Fill)
        .css_classes([class])
        .build()
}

fn centered(label: gtk::Label) -> gtk::Label {
    label.set_justify(gtk::Justification::Center);
    label.set_xalign(0.5);
    label
}

/// A paragraph the page gave `text-wrap: pretty`, which never leaves one word alone on its last line
fn pretty(text: &str, classes: &[&str]) -> Pretty {
    Pretty::new(&ui::label(text, classes))
}

/// What the helper or this window named, said in the page's words
pub fn error_text(failure: &Failure) -> String {
    match failure.error.as_str() {
        "refused" => t!("That cannot be done from here now. Choose the folder again and try once more."),
        "outside" => t!("Only what is inside your home folder comes back here."),
        "not-found" => t!("That was not there then."),
        "point" => t!("That point in time is not there any more."),
        "no-backup" => t!("No backup is set up."),
        "repository" => t!("The backup could not be read."),
        _ => t!("That did not work."),
    }
}

/// Math.round, which rounds a half up rather than away from zero
fn js_round(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}

/// "3 hours ago", "yesterday"
pub fn ago(seconds: f64) -> String {
    let diff = seconds - i18n::now();
    if diff.abs() < 3600.0 {
        relative(js_round(diff / 60.0), Unit::Minute)
    } else if diff.abs() < 86400.0 {
        relative(js_round(diff / 3600.0), Unit::Hour)
    } else {
        i18n::days(js_round(diff / 86400.0))
    }
}

/// "Today, 02:00 PM", for the windows of the past
pub(crate) fn moment_of(seconds: f64) -> String {
    moment(seconds)
}

/// "Today, 02:00 PM": the day and the time of a point
fn moment(seconds: f64) -> String {
    format!("{}, {}", day_of(seconds), time(seconds))
}

/// What putting back came to, in a few sentences
pub fn put_back_text(result: &backend::PutBack, mode: &str) -> String {
    let changed = result.added + result.kept + result.replaced;
    let mut parts = Vec::new();
    if changed == 0.0 && result.skipped == 0.0 && result.same > 0.0 {
        parts.push(t!("Nothing needed putting back: it is the same in your folder now."));
    } else if changed > 0.0 || result.skipped > 0.0 {
        parts.push(t!("Put back."));
    }
    if result.kept > 0.0 {
        parts.push(t!(
            "{count, plural, one {The earlier version of # file is beside the newer one, with its date in its name.} other {The earlier versions of # files are beside the newer ones, with their date in their names.}}",
            count = result.kept
        ));
    }
    if mode == "replace" && result.replaced > 0.0 {
        parts.push(t!("What had the same name was replaced."));
    }
    if result.skipped > 0.0 {
        parts.push(t!("What differs now was left as it is."));
    }
    if !result.missing.is_empty() {
        parts.push(t!("{count, plural, one {# was not there then.} other {# were not there then.}}", count = result.missing.len()));
    }
    parts.join(" ")
}

impl Parts {
    pub fn new(me: &Weak<Backups>) -> Parts {
        // The wordmark of the boot splash and the login screen, the glowing stone as its "a"; "Backups" is
        // a long word in some languages, and goes under the wordmark rather than off the sidebar
        let made = amethystora_ui::brand::brand(&t!("Backups"), 21.0);
        let wordmark = made.first_child().expect("the wordmark");
        let product = wordmark.next_sibling().expect("the name");
        made.remove(&wordmark);
        made.remove(&product);
        let brand = BrandFlow::new(&wordmark, &product, 0.3 * 21.0, gtk::AccessibleRole::Img);
        brand.add_css_class("brand");
        brand.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Backups"))]);
        let holder = ui::hbox(0, &[]);
        holder.append(&brand);
        brand.set_hexpand(true);
        let sidebar = frame::Sidebar::new(&holder, 18);

        let nav = frame::nav(3, None, &t!("Backups"));
        let browse_nav = NavButton::new(icon("history", 20), &t!("Browse"), 12);
        let status_nav = NavButton::new(icon("archive", 20), &t!("Status"), 12);
        let status_dot = ui::hbox(0, &["nav-dot"]);
        status_dot.set_valign(gtk::Align::Center);
        status_dot.set_visible(false);
        status_nav.append(&status_dot);
        for (button, page) in [(&browse_nav, "browse"), (&status_nav, "status")] {
            let weak = me.clone();
            button.button.connect_clicked(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.show(page);
                }
            });
            nav.append(&button.button);
        }
        sidebar.body.append(&nav);

        let elsewhere = frame::nav(3, Some(&t!("Elsewhere")), &t!("Elsewhere"));
        elsewhere.add_css_class("elsewhere");
        let security = NavButton::new(icon("shield", 20), &t!("Security settings"), 12);
        let external = icon("external", 16).mirror_in_rtl();
        external.add_css_class("faint");
        security.append(&external);
        security.button.connect_clicked(|_| backend::security());
        elsewhere.append(&security.button);
        sidebar.body.append(&elsewhere);

        sidebar.foot.set_spacing(12);
        let manual = ui::button(&t!("Backups in the manual"), Some(icon("book", 18).mirror_in_rtl()), &["link", "quiet"], || process::manual("security#backups"));
        manual.set_halign(gtk::Align::Start);
        sidebar.foot.append(&manual);
        let terminal = frame::terminal(&t!("Also in a terminal:"), "ame backup");
        // The command reads left to right, at the start of its line as the page's inline <code> was: the
        // right in a right-to-left window
        if let Some(code) = terminal.last_child().and_downcast::<gtk::Label>().filter(|_| i18n::is_rtl_now()) {
            code.set_xalign(1.0);
        }
        sidebar.foot.append(&terminal);

        // The top bar: when the backup last succeeded and backing up now, with the window's own buttons
        let topbar_row = ui::hbox(0, &["topbar"]);
        let topbar = ui::hbox(14, &[]);
        topbar.set_hexpand(true);
        topbar.set_halign(gtk::Align::End);
        topbar.set_valign(gtk::Align::Center);
        topbar_row.append(&topbar);
        let controls = frame::controls();
        controls.set_valign(gtk::Align::Center);
        topbar_row.append(&controls);

        let banner = ui::vbox(0, &["banner-slot"]);
        banner.set_visible(false);
        let content = ui::vbox(0, &["view"]);
        content.set_vexpand(true);
        let page = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).vexpand(true).build();

        // --- Browse: the folder, and the timeline at the inline end ---------------------------------
        let ask: Asker = {
            let weak = me.clone();
            Rc::new(move |ask| {
                if let Some(this) = upgrade(&weak) {
                    this.asked(ask);
                }
            })
        };
        let files = list::Files::new(ask.clone(), {
            let weak = me.clone();
            Rc::new(move |name: &str| upgrade(&weak).is_some_and(|this| this.state.borrow().selected.iter().any(|s| s == name)))
        });
        let list_scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).vexpand(true).child(&files.view).build();
        let list_card = role_box(gtk::Orientation::Vertical, 0, &["file-list"], gtk::AccessibleRole::Group);
        list_card.set_overflow(gtk::Overflow::Hidden);
        list_card.set_vexpand(true);
        list_card.append(&list_scroller);

        let files_column = ui::vbox(14, &["files"]);
        files_column.set_hexpand(true);
        let unreachable = ui::vbox(0, &[]);
        let head = ui::vbox(0, &[]);
        let body = ui::vbox(0, &[]);
        body.set_vexpand(true);
        let selection = ui::vbox(0, &[]);
        for slot in [&unreachable, &head, &body, &selection] {
            files_column.append(slot);
        }

        let points = list::Points::new(ask, {
            let weak = me.clone();
            Rc::new(move || upgrade(&weak).map(|this| this.state.borrow().point.clone()).unwrap_or_default())
        });
        let points_scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).vexpand(true).child(&points.view).build();
        // The day's heading stays at the top while its points scroll under it, as position: sticky did
        let sticky_label = gtk::Label::builder().xalign(0.0).css_classes(["day-heading"]).build();
        sticky_label.set_ellipsize(pango::EllipsizeMode::End);
        let sticky = Shift::new(&sticky_label);
        sticky.add_css_class("sticky");
        sticky.set_valign(gtk::Align::Start);
        sticky.set_can_target(false);
        sticky.set_visible(false);
        let points_area = gtk::Overlay::new();
        points_area.set_child(Some(&points_scroller));
        points_area.add_overlay(&sticky);
        points_area.set_vexpand(true);
        {
            let weak = me.clone();
            points_scroller.vadjustment().connect_value_changed(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.update_sticky();
                }
            });
        }

        let timeline = role_box(gtk::Orientation::Vertical, 0, &["timeline"], gtk::AccessibleRole::Group);
        timeline.update_property(&[gtk::accessible::Property::Label(&t!("Points in time"))]);
        let timeline_head = ui::vbox(0, &["timeline-head"]);
        // In capitals the language's own way: GTK does not measure the stylesheet's text-transform
        timeline_head.append(&heading(&t!("Go back to").to_uppercase(), "h2"));
        let count = ui::label("", &["faint", "small"]);
        timeline_head.append(&count);
        timeline.append(&timeline_head);
        timeline.append(&points_area);
        let key = ui::wrap(14, 6, &["timeline-key", "small", "faint"]);
        for (glyph, words) in [("disk", t!("On this machine")), ("cloud", t!("In the backup"))] {
            let part = ui::hbox(6, &[]);
            part.append(&icon(glyph, 14));
            part.append(&ui::line(&words, &[]));
            key.append(&part);
        }
        timeline.append(&key);

        let browse = ui::hbox(0, &["browse"]);
        browse.append(&files_column);
        browse.append(&Tight::new(&timeline));
        browse.set_vexpand(true);
        browse.set_visible(false);
        content.append(&browse);
        content.append(&page);

        let main = ui::vbox(0, &["main"]);
        main.append(&topbar_row);
        main.append(&banner);
        main.append(&content);

        // As wide as the stylesheet makes it, whatever the words in it would like, and scrolling in a window
        // too short for it, as `overflow-y: auto` had it
        sidebar.root.set_vexpand(true);
        let column = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).propagate_natural_width(true).child(&sidebar.root).build();
        let framed = frame::with_sidebar(&Tight::new(&column), &main, -1, false);
        let root = gtk::Overlay::new();
        root.set_child(Some(&framed));

        Parts {
            root,
            browse_nav,
            status_nav,
            status_dot,
            topbar,
            banner,
            content,
            page,
            browse,
            unreachable,
            head,
            body,
            selection,
            list_card,
            list_scroller,
            files,
            count,
            points,
            points_scroller,
            sticky,
            sticky_label,
            past: crate::past::Receding::new(me),
        }
    }
}

/// Every line of text in it centred, as `.empty`'s text-align: center did to all that was in it, a callout's
/// words and a log's lines among them
fn center_text(widget: &gtk::Widget) {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        label.set_xalign(0.5);
        label.set_justify(gtk::Justification::Center);
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        center_text(&widget);
        child = widget.next_sibling();
    }
}

fn set_slot(slot: &gtk::Box, child: Option<gtk::Widget>) {
    ui::clear(slot);
    slot.set_visible(child.is_some());
    if let Some(child) = child {
        slot.append(&child);
    }
}

impl Backups {
    // --- Building blocks ----------------------------------------------------------------------------------

    pub(crate) fn button(&self, label: &str, look: Look, on_click: impl Fn(&Rc<Backups>) + 'static) -> gtk::Button {
        let mut classes = vec!["btn"];
        if look.small {
            classes.push("small");
        }
        if look.primary {
            classes.push("primary");
        }
        let weak = self.me.clone();
        let button = ui::button(label, look.icon.map(|name| icon(name, 16)), &classes, move || {
            if let Some(this) = upgrade(&weak) {
                on_click(&this);
            }
        });
        button.set_sensitive(!look.disabled);
        button.set_valign(gtk::Align::Center);
        button
    }

    fn link_button(&self, label: &str, name: Option<&'static str>, on_click: impl Fn(&Rc<Backups>) + 'static) -> gtk::Button {
        let weak = self.me.clone();
        let button = ui::button(label, name.map(|name| icon(name, 15).mirror_in_rtl()), &["link"], move || {
            if let Some(this) = upgrade(&weak) {
                on_click(&this);
            }
        });
        button.set_halign(gtk::Align::Start);
        button.set_valign(gtk::Align::Center);
        button
    }

    fn icon_button(&self, name: &'static str, size: i32, label: &str, on_click: impl Fn(&Rc<Backups>) + 'static) -> gtk::Button {
        let mark = icon(name, size);
        let button = gtk::Button::builder().child(&mark).css_classes(["plain", "icon-btn"]).valign(gtk::Align::Center).build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.set_tooltip_text(Some(label));
        let weak = self.me.clone();
        button.connect_clicked(move |_| {
            if let Some(this) = upgrade(&weak) {
                on_click(&this);
            }
        });
        button
    }

    fn callout(tone: &str, title: &str, text: &str, extra: Vec<Option<gtk::Widget>>) -> gtk::Box {
        let callout = ui::hbox(12, &["callout", tone]);
        let mark = icon(if tone == "check" || tone == "failed" { "alert" } else { "info" }, 18);
        mark.set_valign(gtk::Align::Start);
        callout.append(&mark);
        let body = ui::vbox(6, &["grow"]);
        body.set_hexpand(true);
        if !title.is_empty() {
            body.append(&ui::label(title, &["strong"]));
        }
        if !text.is_empty() {
            body.append(&pretty(text, &["p"]));
        }
        for child in extra.into_iter().flatten() {
            body.append(&child);
        }
        callout.append(&body);
        callout
    }

    fn spinner(text: &str) -> gtk::Box {
        let working = role_box(gtk::Orientation::Horizontal, 10, &["working"], gtk::AccessibleRole::Status);
        let mark = icon("spinner", 18);
        mark.set_spinning(true);
        working.append(&mark);
        if !text.is_empty() {
            working.append(&ui::label(text, &[]));
        }
        working
    }

    fn terminal_tag() -> gtk::Box {
        let tag = ui::hbox(5, &["tag"]);
        tag.set_tooltip_text(Some(&t!("Opens in a terminal, where it asks before it changes anything")));
        tag.set_valign(gtk::Align::Center);
        tag.append(&icon("terminal", 13));
        tag.append(&ui::line(&t!("Terminal"), &[]));
        tag
    }

    fn tag(text: &str, tone: &str) -> gtk::Widget {
        let classes: Vec<&str> = if tone.is_empty() { vec!["tag"] } else { vec!["tag", tone] };
        let tag = ui::line(text, &classes);
        tag.set_valign(gtk::Align::Center);
        tag.upcast()
    }

    /// What a program printed, as it printed it
    fn log_box(text: &str) -> Option<gtk::Widget> {
        (!text.is_empty()).then(|| Self::log_scroller(text).0.upcast())
    }

    /// A log at most 200 pixels tall, its padding included, which scrolls past that
    fn log_scroller(text: &str) -> (Most, gtk::Label) {
        let log = ui::label(text, &["log"]);
        log.set_selectable(true);
        log.set_valign(gtk::Align::Start);
        log.set_direction(gtk::TextDirection::Ltr);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .overlay_scrolling(false)
            .child(&log)
            .css_classes(["page-scroll"])
            .build();
        let most = Most::new(&scroller, &log, 200);
        most.add_css_class("log-scroller");
        (most, log)
    }

    fn actions(children: Vec<Option<gtk::Widget>>, classes: &[&str]) -> adw::WrapBox {
        let actions = ui::wrap(8, 8, classes);
        for child in children.into_iter().flatten() {
            child.set_valign(gtk::Align::Center);
            actions.append(&child);
        }
        actions
    }

    /// The column every page is, at most 920 pixels wide in the middle
    fn page_column(child: &gtk::Box) -> gtk::Widget {
        adw::Clamp::builder().maximum_size(920).tightening_threshold(920).child(child).build().upcast()
    }

    fn empty(children: Vec<Option<gtk::Widget>>) -> gtk::Widget {
        let empty = ui::vbox(8, &["empty"]);
        for child in children.into_iter().flatten() {
            child.set_halign(if child.is::<gtk::Label>() || child.is::<Pretty>() { gtk::Align::Fill } else { gtk::Align::Center });
            center_text(&child);
            empty.append(&child);
        }
        ui::limit_center(&empty, 480).upcast()
    }

    fn unreachable_callout(&self) -> Option<gtk::Widget> {
        let timeline = self.state.borrow().timeline.clone()?;
        (timeline.backup == "unreachable").then(|| {
            Self::callout(
                "check",
                &t!("The backup could not be read"),
                &t!("Only the snapshots on this machine are shown. The backup comes back once its repository can be reached."),
                vec![Self::log_box(timeline.detail.as_deref().unwrap_or(""))],
            )
            .upcast()
        })
    }

    pub(crate) fn current_point(&self) -> Option<Point> {
        let state = self.state.borrow();
        state.timeline.as_ref()?.points.iter().find(|point| point.id == state.point).cloned()
    }

    // --- Browse ---------------------------------------------------------------------------------------

    fn crumbs(&self) -> gtk::Widget {
        let folder = self.state.borrow().folder.clone();
        let crumbs = ui::wrap(2, 0, &["crumbs"]);
        crumbs.upcast_ref::<gtk::Widget>().update_property(&[gtk::accessible::Property::Label(&t!("Folder"))]);
        let crumb = |content: &gtk::Box, target: String| {
            let button = gtk::Button::builder().child(content).css_classes(["plain", "crumb"]).valign(gtk::Align::Center).build();
            let weak = self.me.clone();
            button.connect_clicked(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.open_folder(&target);
                }
            });
            button
        };
        let home = ui::hbox(6, &[]);
        home.append(&icon("home", 15));
        home.append(&ui::line(&t!("Home"), &[]));
        crumbs.append(&crumb(&home, String::new()));
        let parts: Vec<&str> = if folder.is_empty() { Vec::new() } else { folder.split('/').collect() };
        for (at, part) in parts.iter().enumerate() {
            let separator = icon("chevron", 14).mirror_in_rtl();
            separator.add_css_class("crumb-sep");
            crumbs.append(&separator);
            if at == parts.len() - 1 {
                let current = ui::line(&bdi(part), &["crumb", "current"]);
                current.set_valign(gtk::Align::Center);
                crumbs.append(&current);
            } else {
                let content = ui::hbox(6, &[]);
                content.append(&ui::line(&bdi(part), &[]));
                crumbs.append(&crumb(&content, parts[..=at].join("/")));
            }
        }
        crumbs.upcast()
    }

    fn folder_head(&self) -> gtk::Widget {
        let point = self.current_point();
        let (folder, hidden, has_entries, past, points) = {
            let state = self.state.borrow();
            (state.folder.clone(), state.hidden, matches!(&state.listing, Folder::Read(_)), state.past, state.timeline.as_ref().map_or(0, |timeline| timeline.points.len()))
        };
        let folder_name = if folder.is_empty() { t!("Your home folder") } else { folder.rsplit('/').next().unwrap_or("").to_string() };
        let head = ui::vbox(10, &["folder-head"]);
        let title = ui::vbox(4, &["folder-title"]);
        title.append(&heading(&t!("{folder} as it was", folder = bdi(&folder_name)), "h1"));
        if let Some(point) = point {
            let lead = Flex::new(10, 10, &["lead"]);
            lead.append(&ui::line(&moment(point.time), &[]));
            let chip = ui::hbox(6, &["chip", if point.source == "local" { "local" } else { "backup" }]);
            chip.set_valign(gtk::Align::Center);
            chip.append(&source_icon(&point.source, 16));
            chip.append(&ui::line(&source_label(&point.source), &[]));
            lead.append(&chip);
            title.append(&lead);
        }
        head.append(&title);
        head.append(&self.crumbs());
        let tools = Flex::new(10, 10, &["folder-tools"]);
        let hidden_files = Tick::new(Some(&t!("Show hidden files")), &["check-label"]);
        hidden_files.set(hidden);
        let weak = self.me.clone();
        hidden_files.button.connect_clicked(move |_| {
            if let Some(this) = upgrade(&weak) {
                let hidden = !this.state.borrow().hidden;
                this.state.borrow_mut().hidden = hidden;
                this.render();
            }
        });
        tools.append(&hidden_files.button);
        tools.append_with(&ui::hbox(0, &[]), GROW);
        // Back in time, visibly (R7-28), which is only ever offered here: the window opens as it always did
        if past {
            tools.append(&self.button(&t!("Back to the list"), Look { small: true, ..Default::default() }, |this| this.set_past(false)));
        } else if points > 1 {
            tools.append(&self.button(&t!("Back in time"), Look { small: true, icon: Some("history"), ..Default::default() }, |this| this.set_past(true)));
        }
        // Never one click for the whole home folder, which has the question below
        if has_entries && past && !folder.is_empty() {
            // Keeps both, as a file brought back always does: nothing that is there now goes. It asks first, as the list's own
            // button does, since every file that differs gains a copy beside it
            tools.append(&self.button(&t!("Put this back…"), Look { small: true, primary: true, icon: Some("restore"), disabled: self.past_pending.get() }, |this| this.ask(Dialog::Past)));
        } else if has_entries {
            let label = if folder.is_empty() { t!("Put your whole home folder back…") } else { t!("Put the whole folder back…") };
            tools.append(&self.button(&label, Look { small: true, icon: Some("restore"), disabled: self.past_pending.get(), ..Default::default() }, |this| this.ask(Dialog::Folder)));
        }
        head.append(&tools);
        head.upcast()
    }

    fn padded(children: Vec<Option<gtk::Widget>>) -> gtk::Widget {
        let inside = ui::vbox(14, &["padded"]);
        for child in children.into_iter().flatten() {
            inside.append(&child);
        }
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).vexpand(true).child(&inside).build();
        let card = ui::vbox(0, &["file-list"]);
        card.set_overflow(gtk::Overflow::Hidden);
        card.set_vexpand(true);
        card.append(&scroller);
        card.upcast()
    }

    /// The folder's entries to show, folders first, by name as people sort them
    pub(crate) fn entries(&self) -> Vec<Rc<backend::Entry>> {
        let state = self.state.borrow();
        let Folder::Read(listing) = &state.listing else { return Vec::new() };
        let mut list: Vec<Rc<backend::Entry>> = listing.entries.iter().filter(|entry| state.hidden || !entry.name.starts_with('.')).map(|entry| Rc::new(entry.clone())).collect();
        let collator = i18n::with(|i18n| i18n.collator());
        list.sort_by(|a, b| match (a.kind == "dir", b.kind == "dir") {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => match &collator {
                Some(collator) => collator.compare(&a.name, &b.name),
                None => a.name.cmp(&b.name),
            },
        });
        list
    }

    fn files_body(&self) -> gtk::Widget {
        if self.state.borrow().versions.is_some() {
            return self.versions_view();
        }
        let listing = self.state.borrow().listing.clone();
        match listing {
            Folder::None | Folder::Reading => {
                let card = ui::vbox(0, &["file-list"]);
                card.set_vexpand(true);
                card.append(&Self::spinner(&t!("Reading the folder…")));
                card.upcast()
            }
            Folder::Failed(failure) => {
                let missing = failure.error == "not-found";
                Self::padded(vec![Some(
                    Self::callout(
                        if missing { "info" } else { "failed" },
                        &if missing { t!("This folder was not there then") } else { error_text(&failure) },
                        &if missing { t!("Go back to a later point in time, or to a folder above it.") } else { String::new() },
                        vec![if missing { None } else { Self::log_box(&failure.detail) }],
                    )
                    .upcast(),
                )])
            }
            Folder::Read(read) => {
                let hidden = self.state.borrow().hidden;
                if !read.entries.iter().any(|entry| hidden || !entry.name.starts_with('.')) {
                    let text = if read.entries.is_empty() { t!("This folder was empty then.") } else { t!("Only hidden files were in this folder then.") };
                    return Self::padded(vec![Some(ui::label(&text, &["soft"]).upcast())]);
                }
                self.show_entries(&read);
                self.parts.list_card.clone().upcast()
            }
        }
    }

    /// The entries in the list view, sorted and given again only when they are other ones: a tick changes
    /// the rows shown in place
    fn show_entries(&self, listing: &Rc<backend::Listing>) {
        let hidden = self.state.borrow().hidden;
        let same = matches!(&*self.shown.borrow(), Some((shown, was_hidden)) if Rc::ptr_eq(shown, listing) && *was_hidden == hidden);
        if same {
            let state = self.state.borrow();
            self.parts.files.refresh(&|name| state.selected.iter().any(|s| s == name));
            return;
        }
        let list = self.entries();
        self.shown.replace(Some((listing.clone(), hidden)));
        // The list keeps where it was scrolled to, as the page's render() did, unless another folder opens
        let adjustment = self.parts.list_scroller.vadjustment();
        let top = if self.scroll_to_top.replace(false) { 0.0 } else { adjustment.value() };
        self.parts.files.set(&list);
        adjustment.set_value(top);
        glib::idle_add_local_once(move || {
            if (adjustment.value() - top).abs() > 0.5 {
                adjustment.set_value(top);
            }
        });
    }

    fn selection_bar(&self) -> Option<gtk::Widget> {
        let (names, versions, one) = {
            let state = self.state.borrow();
            let names = state.selected.clone();
            let one = match (&state.listing, names.len()) {
                (Folder::Read(listing), 1) => listing.entries.iter().find(|entry| entry.name == names[0]).cloned(),
                _ => None,
            };
            (names, state.versions.is_some(), one)
        };
        if names.is_empty() || versions {
            return None;
        }
        let folder = self.state.borrow().folder.clone();
        let bar = Flex::new(10, 10, &["selection-bar"]);
        let count = ui::line(&t!("{count, plural, one {# selected} other {# selected}}", count = names.len()), &[]);
        count.set_ellipsize(pango::EllipsizeMode::End);
        bar.append_with(&count, GROW);
        bar.append(&self.link_button(&t!("Clear"), None, |this| {
            this.state.borrow_mut().selected.clear();
            this.render();
        }));
        if let Some(one) = one {
            let file = backend::join(&folder, &one.name);
            let (versions_of, look_at) = (file.clone(), file);
            bar.append(&self.button(&t!("Versions"), Look { icon: Some("layers"), small: true, ..Default::default() }, move |this| this.show_versions(&versions_of)));
            bar.append(&self.button(&t!("Look"), Look { icon: Some("eye"), small: true, disabled: self.past_pending.get(), ..Default::default() }, move |this| this.look_at(&look_at, None)));
        }
        bar.append(&self.button(&t!("Put back"), Look { icon: Some("restore"), small: true, primary: true, disabled: self.past_pending.get() }, move |this| {
            let names = this.state.borrow().selected.clone();
            this.start_put_back(names);
        }));
        Some(bar.upcast())
    }

    /// While neither is set up, both are offered; with one or both on, there is just nothing there yet
    fn nothing_yet_text(&self) -> String {
        let state = self.state.borrow();
        let status = state.status.as_ref().expect("read");
        if status.backup.set_up || status.snapshots.on {
            t!("The first snapshot or backup appears here once it has been taken.")
        } else {
            t!("Neither the hourly snapshots nor a backup are set up.")
        }
    }

    fn nothing_to_browse(&self) -> gtk::Widget {
        let view = ui::vbox(28, &["page"]);
        let mark = icon("history", 40);
        view.append(&Self::empty(vec![
            Some(mark.upcast()),
            Some(centered(heading(&t!("Nothing to go back to yet"), "h2")).upcast()),
            Some(centered(ui::label(&self.nothing_yet_text(), &[])).upcast()),
            self.unreachable_callout(),
            Some(Self::actions(vec![Some(self.button(&t!("See what is set up"), Look { primary: true, ..Default::default() }, |this| this.show("status")).upcast())], &["actions"]).upcast()),
        ]));
        Self::page_column(&view)
    }

    /// The folder and the timeline, updated in place
    fn update_browse(&self) {
        set_slot(&self.parts.unreachable, self.unreachable_callout());
        set_slot(&self.parts.head, Some(self.folder_head()));
        let body = self.files_body();
        // Back in time, visibly: the same body, in front of the windows behind it
        let body = if self.state.borrow().past {
            if let Some(holder) = body.parent().filter(|holder| holder != self.parts.past.upcast_ref::<gtk::Widget>()).and_downcast::<gtk::Box>() {
                holder.remove(&body);
            }
            self.parts.past.set_front(Some(&body));
            let point = self.state.borrow().point.clone();
            self.show_behind(&point);
            self.parts.points.show(&point);
            self.parts.past.clone().upcast::<gtk::Widget>()
        } else {
            body
        };
        if self.parts.body.first_child().as_ref() != Some(&body) {
            if self.parts.body.first_child().as_ref() == Some(self.parts.list_card.upcast_ref()) {
                self.parts.body.remove(&self.parts.list_card);
            }
            ui::clear(&self.parts.body);
            self.parts.body.append(&body);
        }
        set_slot(&self.parts.selection, self.selection_bar());
        self.update_timeline();
    }

    // --- The timeline ---------------------------------------------------------------------------------

    fn update_timeline(&self) {
        let (timeline, point) = {
            let state = self.state.borrow();
            (state.timeline.clone(), state.point.clone())
        };
        let Some(timeline) = timeline else { return };
        let same = self.shown_timeline.borrow().as_ref().is_some_and(|shown| Rc::ptr_eq(shown, &timeline));
        if !same {
            self.shown_timeline.replace(Some(timeline.clone()));
            self.parts.count.set_label(&t!("{count, plural, one {# point in time} other {# points in time}}", count = timeline.points.len()));
            let adjustment = self.parts.points_scroller.vadjustment();
            let top = adjustment.value();
            self.parts.points.set(&timeline.points, &timeline.hostname);
            adjustment.set_value(top);
        } else {
            self.parts.points.refresh(&point);
        }
        let weak = self.me.clone();
        glib::idle_add_local_once(move || {
            if let Some(this) = upgrade(&weak) {
                this.update_sticky();
            }
        });
    }

    /// The day of the point at the top of the timeline, over it, pushed up by the next day's heading as it
    /// comes up under it
    pub(crate) fn update_sticky(&self) {
        let parts = &self.parts;
        if parts.points_scroller.vadjustment().value() <= 0.0 {
            parts.sticky.set_visible(false);
            return;
        }
        let Some(top) = parts.points.row_at(1.0) else {
            parts.sticky.set_visible(false);
            return;
        };
        parts.sticky_label.set_label(&top.day());
        let height = parts.sticky_label.measure(gtk::Orientation::Vertical, parts.points.view.width()).1 as f64;
        let mut offset = 0.0;
        if let Some(next) = parts.points.row_at(height - 1.0).filter(|row| row != &top && row.is_heading()) {
            if let Some(at) = next.compute_point(&parts.points.view, &amethystora_ui::graphene::Point::new(0.0, 0.0)) {
                offset = (height - at.y() as f64).max(0.0);
            }
        }
        parts.sticky.set_offset(offset as f32);
        parts.sticky.set_visible(true);
    }

    // --- Versions of one file -------------------------------------------------------------------------

    fn versions_view(&self) -> gtk::Widget {
        let Some((file, result)) = self.state.borrow().versions.clone() else { return ui::vbox(0, &[]).upcast() };
        let name = file.rsplit('/').next().unwrap_or("").to_string();
        let back = self.link_button(&t!("Back to the folder"), Some("back"), |this| {
            this.state.borrow_mut().versions = None;
            this.render();
        });
        match result {
            VersionsState::Reading => Self::padded(vec![Some(back.upcast()), Some(Self::spinner(&t!("Looking for every version of {name}…", name = &name)).upcast())]),
            VersionsState::Read(Err(failure)) => {
                Self::padded(vec![Some(back.upcast()), Some(Self::callout("failed", &error_text(&failure), "", vec![Self::log_box(&failure.detail)]).upcast())])
            }
            VersionsState::Read(Ok(result)) => {
                let rows = self.version_rows(&file, &name, &result);
                let now = match &result.now {
                    Some(now) => t!("Now, in your folder: modified {when}", when = when(now.mtime)),
                    None => t!("It is not in your folder now."),
                };
                Self::padded(vec![
                    Some(back.upcast()),
                    Some(heading(&t!("Versions of {name}", name = bdi(&name)), "versions-title").upcast()),
                    Some(ui::label(&now, &["soft"]).upcast()),
                    (result.backup == "unreachable").then(|| {
                        Self::callout(
                            "check",
                            &t!("The backup could not be read"),
                            &t!("Only the versions on this machine are shown."),
                            vec![Self::log_box(result.detail.as_deref().unwrap_or(""))],
                        )
                        .upcast()
                    }),
                    Some(match rows {
                        Some(rows) => rows.upcast(),
                        None if result.backup == "later" => ui::label(&t!("No version of it is on this machine."), &["soft"]).upcast(),
                        None => ui::label(&t!("No version of it was kept."), &["soft"]).upcast(),
                    }),
                    (result.backup == "later").then(|| Self::spinner(&t!("Looking in your backup…")).upcast()),
                    self.versions_kept_note().map(|note| ui::label(&note, &["soft"]).upcast()),
                ])
            }
        }
    }

    /// Which of the two that keep versions is off, in a sentence: only what they hold can be shown
    fn versions_kept_note(&self) -> Option<String> {
        let state = self.state.borrow();
        let status = state.status.as_ref()?;
        match (status.snapshots.on, status.backup.set_up) {
            (true, true) => None,
            (true, false) => Some(t!("Only the hourly snapshots keep versions: no backup is set up.")),
            (false, true) => Some(t!("Only your backup keeps versions: the hourly snapshots are off.")),
            (false, false) => Some(t!("The hourly snapshots are off and no backup is set up, so nothing new is being kept.")),
        }
    }

    fn version_rows(&self, file: &str, name: &str, result: &Versions) -> Option<gtk::Box> {
        if result.versions.is_empty() {
            return None;
        }
        let rows = ui::vbox(0, &["versions"]);
        rows.set_overflow(gtk::Overflow::Hidden);
        for version in &result.versions {
            let row = ui::hbox(12, &["version-row"]);
            row.append(&source_icon(&version.source, 16));
            let words = ui::vbox(0, &["grow"]);
            words.set_hexpand(true);
            words.append(&ui::label(&moment(version.time), &["strong"]));
            let detail: Vec<String> = [source_label(&version.source), t!("modified {when}", when = when(version.mtime)), size(version.size)].into_iter().filter(|s| !s.is_empty()).collect();
            words.append(&ui::label(&detail.join(" · "), &["small", "faint"]));
            row.append(&words);
            let (id, go, look) = (version.id.clone(), name.to_string(), file.to_string());
            let at = id.clone();
            let (keep, kept_id) = (file.to_string(), id.clone());
            let actions = Self::actions(
                vec![
                    Some(self.button(&t!("Go there"), Look { small: true, ..Default::default() }, move |this| this.go_to(&id, &go)).upcast()),
                    Some(self.button(&t!("Look"), Look { small: true, icon: Some("eye"), ..Default::default() }, move |this| this.look_at(&look, Some(&at))).upcast()),
                    Some(self.button(&t!("Keep this one too"), Look { small: true, ..Default::default() }, move |this| this.keep_version(&keep, &kept_id)).upcast()),
                ],
                &["actions"],
            );
            actions.set_valign(gtk::Align::Center);
            row.append(&actions);
            rows.append(&row);
        }
        Some(rows)
    }

    // --- The question before putting back ------------------------------------------------------------

    pub(crate) fn render_dialog(&self) {
        if let Some(layer) = self.dialog_layer.take() {
            self.parts.root.remove_overlay(&layer);
        }
        let dialog = self.state.borrow().dialog.clone();
        let behind = self.parts.root.child().expect("the window");
        let Some(dialog) = dialog else {
            // What is behind it is the page again, to a screen reader too
            behind.update_state(&[gtk::accessible::State::Hidden(false)]);
            return;
        };
        let close = |this: &Rc<Backups>| {
            this.state.borrow_mut().dialog = None;
            this.render_dialog();
        };
        let folder = self.state.borrow().folder.clone();
        let body = role_box(gtk::Orientation::Vertical, 14, &["dialog"], gtk::AccessibleRole::Dialog);
        body.update_property(&[gtk::accessible::Property::Modal(true)]);
        let primary;
        match &dialog {
            Dialog::Folder => {
                let name = if folder.is_empty() { t!("your home folder") } else { folder.rsplit('/').next().unwrap_or("").to_string() };
                body.append(&heading(&t!("Put {folder} back as it was?", folder = bdi(&name)), "h2"));
                body.append(&ui::label(
                    &t!("Files with the same name as one then are replaced by the earlier version. Files it did not have, such as what ransomware renamed, stay where they are: nothing is deleted."),
                    &["soft"],
                ));
                body.append(&ui::label(
                    &t!("This is for getting back from ransomware or from a change that went wrong everywhere at once. For a few files, select them instead."),
                    &["soft"],
                ));
                let whole = folder.clone();
                primary = self.button(&t!("Put the folder back"), Look { primary: true, icon: Some("restore"), ..Default::default() }, move |this| {
                    this.put_back(vec![whole.clone()], "replace");
                });
                body.append(&Self::dialog_actions(vec![self.button(&t!("Cancel"), Look::default(), close), primary.clone()]));
            }
            Dialog::Past => {
                let name = folder.rsplit('/').next().unwrap_or("").to_string();
                body.append(&heading(&t!("Put {folder} back as it was then?", folder = bdi(&name)), "h2"));
                body.append(&ui::label(
                    &t!("What it had then comes back. A file that is different now stays, and the earlier version comes back beside it with its date in its name. Nothing is deleted or replaced."),
                    &["soft"],
                ));
                let whole = folder.clone();
                primary = self.button(&t!("Put the folder back"), Look { primary: true, icon: Some("restore"), ..Default::default() }, move |this| {
                    this.put_back(vec![whole.clone()], "keep-both");
                });
                body.append(&Self::dialog_actions(vec![self.button(&t!("Cancel"), Look::default(), close), primary.clone()]));
            }
            Dialog::Conflict(names) => {
                let mode = self.state.borrow().mode;
                let options = role_box(gtk::Orientation::Vertical, 8, &["options"], gtk::AccessibleRole::RadioGroup);
                options.update_property(&[gtk::accessible::Property::Label(&t!("What to do with what you have now"))]);
                for (value, label, text) in [
                    ("keep-both", t!("Keep both"), t!("The earlier version comes back beside the one you have, with its date in its name.")),
                    ("replace", t!("Replace"), t!("The earlier version takes the place of the one you have.")),
                    ("skip", t!("Skip"), t!("What you have now stays as it is; only what is missing comes back.")),
                ] {
                    let chosen = mode == value;
                    let content = ui::hbox(10, &[]);
                    let dot = ui::hbox(0, &["dot"]);
                    dot.set_valign(gtk::Align::Start);
                    dot.set_hexpand(false);
                    if chosen {
                        let fill = ui::hbox(0, &["dot-fill"]);
                        fill.set_halign(gtk::Align::Center);
                        fill.set_valign(gtk::Align::Center);
                        fill.set_hexpand(true);
                        dot.append(&fill);
                    }
                    content.append(&dot);
                    let words = ui::vbox(1, &["option-words"]);
                    words.set_hexpand(true);
                    words.append(&ui::label(&label, &["strong"]));
                    words.append(&pretty(&text, &["small"]));
                    content.append(&words);
                    let option = gtk::Button::builder().accessible_role(gtk::AccessibleRole::Radio).child(&content).css_classes(["plain", "option"]).build();
                    if chosen {
                        option.add_css_class("checked");
                    }
                    option.update_property(&[gtk::accessible::Property::Label(&label), gtk::accessible::Property::Description(&text)]);
                    option.update_state(&[gtk::accessible::State::Checked(if chosen { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
                    let weak = self.me.clone();
                    option.connect_clicked(move |_| {
                        if let Some(this) = upgrade(&weak) {
                            this.state.borrow_mut().mode = value;
                            this.render_dialog();
                        }
                    });
                    options.append(&option);
                }
                body.append(&heading(
                    &t!("{count, plural, one {Something with this name is in your folder now} other {Some of these are in your folder now}}", count = names.len()),
                    "h2",
                ));
                body.append(&ui::label(&t!("Files that are the same are left alone. For those that differ:"), &["soft"]));
                body.append(&options);
                let files: Vec<String> = names.iter().map(|name| backend::join(&folder, name)).collect();
                primary = self.button(&t!("Put back"), Look { primary: true, icon: Some("restore"), ..Default::default() }, move |this| {
                    let mode = this.state.borrow().mode;
                    this.put_back(files.clone(), mode);
                });
                body.append(&Self::dialog_actions(vec![self.button(&t!("Cancel"), Look::default(), close), primary.clone()]));
            }
        }
        let width = ui::limit_center(&body, 600);
        width.add_css_class("dialog-width");
        // In the middle of the window, as place-items: center put it
        let back = ui::vbox(0, &["dialog-back"]);
        back.set_hexpand(true);
        back.set_vexpand(true);
        let above = ui::vbox(0, &[]);
        above.set_vexpand(true);
        let below = ui::vbox(0, &[]);
        below.set_vexpand(true);
        back.append(&above);
        back.append(&width);
        back.append(&below);
        // A click on the dimmed page around it closes it
        let click = gtk::GestureClick::new();
        let (weak, target, question) = (self.me.clone(), back.downgrade(), body.downgrade());
        click.connect_released(move |gesture, _, x, y| {
            let (Some(this), Some(back), Some(question)) = (upgrade(&weak), target.upgrade(), question.upgrade()) else { return };
            let inside = back.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|widget| widget == question || widget.is_ancestor(&question));
            if !inside {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                close(&this);
            }
        });
        back.add_controller(click);
        // An overlay gives its child at least the height the child asks for at no width in particular, which
        // a question of wrapped lines makes taller than the window: in a scroller it is the window's height,
        // and a question taller than the window scrolls
        let layer = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).child(&back).build();
        self.parts.root.add_overlay(&layer);
        let back = layer;
        // Only the question, to a screen reader, while it is asked
        behind.update_state(&[gtk::accessible::State::Hidden(true)]);
        self.dialog_layer.replace(Some(back.upcast()));
        primary.grab_focus();
    }

    fn dialog_actions(buttons: Vec<gtk::Button>) -> adw::WrapBox {
        let actions = ui::wrap(8, 8, &["actions", "end"]);
        actions.set_align(1.0);
        for button in buttons {
            actions.append(&button);
        }
        actions
    }

    // --- Status ---------------------------------------------------------------------------------------

    fn facts(rows: Vec<Option<(String, gtk::Widget)>>) -> gtk::Grid {
        let grid = gtk::Grid::builder().column_spacing(18).row_spacing(6).css_classes(["facts"]).build();
        for (at, (label, value)) in rows.into_iter().flatten().enumerate() {
            let name = ui::label(&label, &["faint"]);
            name.set_hexpand(false);
            name.set_valign(gtk::Align::Start);
            grid.attach(&name, 0, at as i32, 1, 1);
            value.set_hexpand(true);
            grid.attach(&value, 1, at as i32, 1, 1);
        }
        grid
    }

    /// The agent looks into the failed unit in a terminal and changes nothing until asked
    fn agent_link(&self) -> Option<gtk::Widget> {
        let agent = self.state.borrow().status.as_ref().is_some_and(|status| status.agent);
        agent.then(|| self.link_button(&t!("Ask the agent"), Some("sparkle"), |_| backend::diagnose()).upcast())
    }

    fn card(glyph: &str, title: &str, text: &str, tag: gtk::Widget, body: Vec<Option<gtk::Widget>>) -> gtk::Box {
        let card = ui::vbox(14, &["setting", "card"]);
        let head = ui::hbox(14, &["setting-head"]);
        let square = ui::hbox(0, &["setting-icon"]);
        square.set_valign(gtk::Align::Start);
        square.set_hexpand(false);
        let mark = icon(glyph, 22);
        mark.set_hexpand(true);
        square.append(&mark);
        head.append(&square);
        let words = ui::vbox(3, &["setting-words"]);
        words.set_hexpand(true);
        let line = Flex::new(8, 8, &["setting-title"]);
        line.append(&heading(title, "h3"));
        line.append(&tag);
        words.append(&line);
        words.append(&ui::limit(&pretty(text, &["setting-text"]), 640));
        head.append(&words);
        card.append(&head);
        for child in body.into_iter().flatten() {
            card.append(&child);
        }
        card
    }

    fn backup_progress(&self) -> Option<gtk::Widget> {
        let backup = self.state.borrow().backup.clone()?;
        Some(match backup {
            BackupRun::Running(lines) => {
                let progress = role_box(gtk::Orientation::Vertical, 4, &["progress"], gtk::AccessibleRole::Status);
                let head = ui::hbox(8, &["progress-head"]);
                let spinner = icon("spinner", 16);
                spinner.set_spinning(true);
                head.append(&spinner);
                head.append(&ui::line(&t!("Backing up…"), &[]));
                progress.append(&head);
                let shown = lines[lines.len().saturating_sub(6)..].join("\n");
                if !shown.is_empty() {
                    let (scroller, label) = Self::log_scroller(&shown);
                    progress.append(&scroller);
                    self.live_log.replace(Some(label));
                }
                progress.upcast()
            }
            BackupRun::Done { ok: true, .. } => Self::callout("info", &t!("Backed up"), &t!("Your home folder is in the backup as it is now."), Vec::new()).upcast(),
            BackupRun::Done { ok: false, lines } => {
                Self::callout("failed", &t!("The backup did not succeed"), &t!("What it logged:"), vec![Self::log_box(&lines), self.agent_link()]).upcast()
            }
        })
    }

    fn backup_card(&self) -> gtk::Box {
        let (backup, failed_lines, running_here, daily_busy) = {
            let state = self.state.borrow();
            let status = state.status.as_ref().expect("read");
            (status.backup.clone(), state.failed_lines.clone().unwrap_or_default(), matches!(state.backup, Some(BackupRun::Running(_))), state.daily_busy)
        };
        let tag = if !backup.set_up {
            Self::tag(&t!("Not set up"), "")
        } else if backup.daily {
            Self::tag(&t!("Every day"), "on")
        } else {
            Self::tag(&t!("Daily run off"), "")
        };
        let title = t!("Daily backup");
        let text = t!("A restic backup of your home folder, once a day, to a repository that ransomware on this machine cannot delete from.");
        if !backup.set_up {
            let body = vec![
                Some(ui::limit(&pretty(&t!("Set it up once, in a terminal, where it explains how to make a repository that this machine can add to and read from but never delete from."), &["setting-text"]), 640).upcast()),
                Some(
                    Self::actions(
                        vec![
                            Some(self.button(&t!("Set up a backup…"), Look { primary: true, icon: Some("terminal"), ..Default::default() }, |_| backend::set_up()).upcast()),
                            Some(Self::terminal_tag().upcast()),
                        ],
                        &["actions"],
                    )
                    .upcast(),
                ),
            ];
            return Self::card("archive", &title, &text, tag, body);
        }
        let repository = ui::label(backup.repository.as_deref().unwrap_or(""), &["code"]);
        repository.set_direction(gtk::TextDirection::Ltr);
        repository.set_wrap_mode(pango::WrapMode::Char);
        repository.set_selectable(true);
        let last = match backup.last_success {
            Some(last) => format!("{} ({})", when(last), ago(last)),
            None => t!("None yet"),
        };
        let facts = Self::facts(vec![
            Some((t!("Repository"), repository.upcast())),
            Some((t!("Last good backup"), ui::label(&last, &[]).upcast())),
            backup.next.filter(|_| backup.daily).map(|next| (t!("Next"), ui::label(&when(next), &[]).upcast())),
        ]);
        let mut body: Vec<Option<gtk::Widget>> = vec![Some(facts.upcast())];
        if backup.stale {
            let title = match backup.last_success {
                Some(last) => t!(
                    "{days, plural, one {No backup has succeeded for # day} other {No backup has succeeded for # days}}",
                    days = ((i18n::now() - last) / 86400.0).floor()
                ),
                None => t!("The daily backup is on, but none has succeeded yet"),
            };
            body.push(Some(
                Self::callout("check", &title, &t!("It runs once a day. Back up now to see what goes wrong, or ask the agent."), vec![Self::log_box(&failed_lines), self.agent_link()]).upcast(),
            ));
        } else if backup.failed {
            body.push(Some(Self::callout("check", &t!("The last backup failed"), &t!("The next one runs as planned. What it logged:"), vec![Self::log_box(&failed_lines), self.agent_link()]).upcast()));
        }
        body.push(self.backup_progress());
        let daily = backup.daily;
        body.push(Some(
            Self::actions(
                vec![
                    Some(self.button(&t!("Back up now"), Look { primary: true, icon: Some("upload"), disabled: running_here || backup.running, ..Default::default() }, |this| this.backup_now()).upcast()),
                    Some(self.button(&t!("Change…"), Look { icon: Some("terminal"), ..Default::default() }, |_| backend::set_up()).upcast()),
                    Some(
                        self.button(&if daily { t!("Turn off the daily run") } else { t!("Turn on the daily run") }, Look { disabled: daily_busy, ..Default::default() }, move |this| this.set_daily(!daily))
                            .upcast(),
                    ),
                ],
                &["actions"],
            )
            .upcast(),
        ));
        body.push(Some(ui::label(&t!("Nothing on this machine deletes from the repository, so old backups build up. Thin them out from somewhere else, as the manual explains."), &["faint", "small"]).upcast()));
        Self::card("archive", &title, &text, tag, body)
    }

    fn snapshots_card(&self) -> gtk::Box {
        let snapshots = self.state.borrow().status.as_ref().expect("read").snapshots.clone();
        let tag = if snapshots.on { Self::tag(&t!("Every hour"), "on") } else { Self::tag(&t!("Off"), "") };
        let mut body: Vec<Option<gtk::Widget>> = Vec::new();
        if !snapshots.possible {
            body.push(Some(
                Self::callout("info", "", &t!("This machine cannot take them: the home folders are not on a btrfs subvolume of their own, which is how the installer sets a disk up unless it was partitioned by hand."), Vec::new()).upcast(),
            ));
        } else if snapshots.on {
            body.push(Some(
                Self::facts(vec![
                    Some((t!("Kept"), ui::label(&t!("{count, plural, one {# snapshot of your home folder} other {# snapshots of your home folder}}", count = snapshots.count), &[]).upcast())),
                    snapshots.latest.map(|latest| (t!("Latest"), ui::label(&format!("{} ({})", when(latest), ago(latest)), &[]).upcast())),
                ])
                .upcast(),
            ));
        }
        if snapshots.on && !snapshots.covers {
            body.push(Some(Self::callout("check", "", &t!("Your home folder is not under /var/home, so the snapshots do not cover it."), Vec::new()).upcast()));
        }
        body.push(Some(ui::limit(&pretty(&t!("They are part of ransomware protection, which Security turns on and off."), &["setting-text"]), 640).upcast()));
        body.push(Some(Self::actions(vec![Some(self.button(&t!("Open Security"), Look { icon: Some("external"), ..Default::default() }, |_| backend::security()).upcast())], &["actions"]).upcast()));
        Self::card(
            "history",
            &t!("Hourly snapshots"),
            &t!("A read-only copy of the home folders every hour, on this machine: every one from the last day, and one a day for two weeks."),
            tag,
            body,
        )
    }

    fn page_head(title: &str, lead: &str) -> gtk::Box {
        let head = ui::vbox(6, &["page-head"]);
        head.append(&heading(title, "h1"));
        head.append(&ui::limit(&pretty(lead, &["lead"]), 640));
        head
    }

    fn status_view(&self) -> gtk::Widget {
        let view = ui::vbox(28, &["page"]);
        view.append(&Self::page_head(
            &t!("Status"),
            &t!("Two things keep earlier versions of your files: snapshots on this machine every hour, and a backup somewhere else every day. Each covers what the other cannot."),
        ));
        view.append(&self.backup_card());
        view.append(&self.snapshots_card());
        Self::page_column(&view)
    }

    /// Neither is set up: the Status page offers both
    pub(crate) fn nothing_set_up(&self) -> bool {
        let state = self.state.borrow();
        let Some(status) = &state.status else { return false };
        !status.backup.set_up && !status.snapshots.on && state.timeline.as_ref().is_none_or(|timeline| timeline.points.is_empty())
    }

    fn empty_view(&self) -> gtk::Widget {
        let possible = self.state.borrow().status.as_ref().is_some_and(|status| status.snapshots.possible);
        let view = ui::vbox(28, &["page"]);
        let gem = Gem::plain(46.0);
        gem.add_css_class("gem");
        view.append(&Self::empty(vec![
            Some(gem.upcast()),
            Some(centered(heading(&t!("Nothing keeps earlier versions of your files yet"), "h2")).upcast()),
            Some(centered(ui::label(&t!("Two things can, and they work best together: a backup somewhere else every day, which survives losing this machine, and snapshots here every hour, which bring back a file from this morning in a moment."), &[])).upcast()),
        ]));
        let offers = AutoGrid::new(240, 14, &["offers"]);
        let offer = |glyph: &str, title: &str, text: &str, button: gtk::Button| {
            let card = ui::vbox(8, &["card", "offer"]);
            let mark = icon(glyph, 26);
            mark.set_halign(gtk::Align::Start);
            card.append(&mark);
            card.append(&heading(title, "h3"));
            card.append(&ui::label(text, &["soft"]));
            card.append(&Self::actions(vec![Some(button.upcast())], &["actions"]));
            card
        };
        offers.append(&offer(
            "archive",
            &t!("A daily backup"),
            &t!("To a server, an object store or a USB drive, set up in a terminal."),
            self.button(&t!("Set up a backup…"), Look { primary: true, icon: Some("terminal"), ..Default::default() }, |_| backend::set_up()),
        ));
        offers.append(&offer(
            "history",
            &t!("Hourly snapshots"),
            &if possible { t!("Turned on in Security, as ransomware protection.") } else { t!("Not possible on this machine: the home folders are not on a btrfs subvolume of their own.") },
            self.button(&t!("Open Security"), Look { icon: Some("external"), disabled: !possible, ..Default::default() }, |_| backend::security()),
        ));
        view.append(&offers);
        Self::page_column(&view)
    }

    // --- The window -----------------------------------------------------------------------------------

    fn loading_view(&self) -> gtk::Widget {
        let loading = role_box(gtk::Orientation::Vertical, 0, &["loading"], gtk::AccessibleRole::Status);
        // min-height: 50vh, half the window as it is now, or as it opens
        let height = self.shell.window.height();
        loading.set_size_request(-1, if height > 0 { height / 2 } else { 420 });
        let middle = ui::vbox(12, &[]);
        middle.set_vexpand(true);
        middle.set_valign(gtk::Align::Center);
        middle.set_halign(gtk::Align::Center);
        let gem = Gem::plain(34.0);
        gem.add_css_class("gem");
        middle.append(&gem);
        middle.append(&ui::line(&t!("Looking for your snapshots and backups…"), &[]));
        loading.append(&middle);
        // At the top of the view: the middle's expanding stops here, or the view would be as tall as the window
        loading.set_vexpand(false);
        let top = ui::vbox(0, &[]);
        top.append(&loading);
        top.upcast()
    }

    fn error_view(&self) -> gtk::Widget {
        let error = self.state.borrow().error.clone();
        let view = ui::vbox(28, &["page"]);
        view.append(&Self::empty(vec![
            Some(centered(heading(&t!("What is set up could not be read"), "h2")).upcast()),
            Self::log_box(&error),
            Some(Self::actions(vec![Some(self.button(&t!("Try again"), Look { icon: Some("refresh"), primary: true, ..Default::default() }, |this| this.load(false, false)).upcast())], &["actions"]).upcast()),
        ]));
        Self::page_column(&view)
    }

    fn banner_view(&self) -> Option<gtk::Widget> {
        let (working, banner) = {
            let state = self.state.borrow();
            (state.working.clone(), state.banner.clone())
        };
        let wrap = ui::vbox(0, &["banner-wrap"]);
        if !working.is_empty() {
            let line = role_box(gtk::Orientation::Horizontal, 12, &["banner"], gtk::AccessibleRole::Status);
            let spinner = icon("spinner", 18);
            spinner.set_spinning(true);
            line.append(&spinner);
            let text = ui::label(&working, &["grow"]);
            text.set_hexpand(true);
            line.append(&text);
            wrap.append(&ui::limit_center(&line, 920));
            return Some(wrap.upcast());
        }
        let banner: Banner = banner?;
        let line = role_box(gtk::Orientation::Horizontal, 12, if banner.failed { &["banner", "failed"] } else { &["banner"] }, if banner.failed { gtk::AccessibleRole::Alert } else { gtk::AccessibleRole::Status });
        line.append(&icon(if banner.failed { "alert" } else { "check" }, 18));
        let words = ui::vbox(0, &["grow"]);
        words.set_hexpand(true);
        words.append(&ui::label(&banner.text, &[]));
        if let Some(log) = Self::log_box(&banner.detail) {
            words.append(&log);
        }
        line.append(&words);
        line.append(&self.icon_button("close", 16, &t!("Dismiss"), |this| {
            this.state.borrow_mut().banner = None;
            this.render();
        }));
        wrap.append(&ui::limit_center(&line, 920));
        Some(wrap.upcast())
    }

    /// The top bar: when the backup last succeeded, and backing up now, from any page
    fn render_topbar(&self) {
        ui::clear(&self.parts.topbar);
        let (backup, running_here, page) = {
            let state = self.state.borrow();
            (state.status.as_ref().map(|status| status.backup.clone()), matches!(state.backup, Some(BackupRun::Running(_))), state.page)
        };
        let Some(backup) = backup.filter(|backup| backup.set_up) else { return };
        let state_line = |glyph: &str, text: &str, spinning: bool| {
            let line = ui::hbox(7, &["topbar-state"]);
            line.set_valign(gtk::Align::Center);
            let mark = icon(glyph, 15);
            mark.set_spinning(spinning);
            line.append(&mark);
            line.append(&ui::line(text, &[]));
            line
        };
        if running_here || backup.running {
            self.parts.topbar.append(&state_line("spinner", &t!("Backing up…"), true));
        } else if backup.stale || backup.failed {
            let text = match backup.last_success {
                Some(last) => t!("Last good backup {when}", when = ago(last)),
                None => t!("No good backup yet"),
            };
            let content = state_line("alert", &text, false);
            let button = gtk::Button::builder().child(&content).css_classes(["plain", "topbar-state", "check"]).valign(gtk::Align::Center).build();
            let weak = self.me.clone();
            button.connect_clicked(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.show("status");
                }
            });
            self.parts.topbar.append(&button);
        } else if let Some(last) = backup.last_success {
            self.parts.topbar.append(&state_line("check", &t!("Last good backup {when}", when = ago(last)), false));
        }
        // The status page has the button on its own card
        if page != "status" {
            let button = self.button(&t!("Back up now"), Look { small: true, icon: Some("upload"), disabled: running_here || backup.running, ..Default::default() }, |this| this.backup_now());
            self.parts.topbar.append(&button);
        }
    }

    fn render_nav(&self) {
        let state = self.state.borrow();
        self.parts.browse_nav.set_current(state.page == "browse");
        self.parts.status_nav.set_current(state.page == "status");
        let backup = state.status.as_ref().map(|status| &status.backup);
        self.parts.status_dot.set_visible(backup.is_some_and(|backup| backup.stale || backup.failed));
    }

    fn render_banner(&self) {
        let banner = self.banner_view();
        set_slot(&self.parts.banner, banner);
    }

    /// Drawn again in place: the page, the file list and the timeline keep where they were scrolled to
    pub(crate) fn render(&self) {
        self.live_log.replace(None);
        self.render_nav();
        self.render_topbar();
        // What had the focus goes with the page, and the focus with it to no widget, as it went to the
        // page's body: GTK would otherwise hand it to the next widget it finds, and scroll to that
        let window = &self.shell.window;
        let inside = |area: &gtk::Widget| gtk::prelude::GtkWindowExt::focus(window).is_some_and(|focus| focus.is_ancestor(area) && !focus.is_ancestor(&self.parts.points.view) && !focus.is_ancestor(&self.parts.files.view));
        if inside(self.parts.content.upcast_ref()) || inside(self.parts.banner.upcast_ref()) || inside(self.parts.topbar.upcast_ref()) {
            gtk::prelude::GtkWindowExt::set_focus(window, None::<&gtk::Widget>);
        }
        self.render_banner();
        let (loaded, error, page) = {
            let state = self.state.borrow();
            (state.status.is_some() && state.timeline.is_some(), !state.error.is_empty(), state.page)
        };
        let browsing = loaded && !self.nothing_set_up() && page == "browse" && self.state.borrow().timeline.as_ref().is_some_and(|timeline| !timeline.points.is_empty());
        // Both stay in the window, the one not shown hidden: the lists keep the rows they bound, which a
        // view taken out of the window would leave unbound under the names they had
        self.parts.browse.set_visible(browsing);
        self.parts.page.set_visible(!browsing);
        if browsing {
            self.update_browse();
            return;
        }
        let content = if !loaded {
            if error { self.error_view() } else { self.loading_view() }
        } else if page == "status" {
            if self.nothing_set_up() { self.empty_view() } else { self.status_view() }
        } else {
            self.nothing_to_browse()
        };
        let adjustment = self.parts.page.vadjustment();
        let top = adjustment.value();
        self.parts.page.set_child(Some(&content));
        adjustment.set_value(top);
        let adjustment = adjustment.clone();
        glib::idle_add_local_once(move || {
            if (adjustment.value() - top).abs() > 0.5 {
                adjustment.set_value(top);
            }
        });
    }

    /// The new lines of a backup that runs, in its log in place, or the page drawn again for its first
    pub(crate) fn render_lines(&self) {
        let (lines, status) = match &*self.state.borrow() {
            PageState { backup: Some(BackupRun::Running(lines)), page, .. } => (lines[lines.len().saturating_sub(6)..].join("\n"), *page == "status"),
            _ => return,
        };
        // Only the status page's card shows them: nothing else on any page changes with them
        if !status {
            return;
        }
        let live = self.live_log.borrow().clone();
        match live {
            Some(label) if label.root().is_some() => label.set_label(&lines),
            _ => self.render(),
        }
    }

    /// A page of the sidebar shown, from the top
    pub(crate) fn scroll_page_top(&self) {
        self.parts.page.vadjustment().set_value(0.0);
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::PutBack;

    #[test]
    fn rounds_as_math_round() {
        assert_eq!(js_round(-2.5), -2);
        assert_eq!(js_round(2.5), 3);
        assert_eq!(js_round(-0.4), 0);
        assert_eq!(js_round(-180.0), -180);
    }

    #[test]
    fn says_what_putting_back_came_to() {
        let result = |added: f64, same: f64, kept: f64, replaced: f64, skipped: f64, missing: usize| PutBack {
            added,
            same,
            kept,
            replaced,
            skipped,
            missing: (0..missing).map(|n| format!("f{n}")).collect(),
        };
        assert_eq!(put_back_text(&result(0.0, 2.0, 0.0, 0.0, 0.0, 0), "keep-both"), "Nothing needed putting back: it is the same in your folder now.");
        assert_eq!(put_back_text(&result(1.0, 0.0, 0.0, 0.0, 0.0, 0), "keep-both"), "Put back.");
        assert_eq!(
            put_back_text(&result(0.0, 1.0, 1.0, 0.0, 0.0, 0), "keep-both"),
            "Put back. The earlier version of 1 file is beside the newer one, with its date in its name."
        );
        assert_eq!(put_back_text(&result(0.0, 0.0, 0.0, 1.0, 0.0, 0), "replace"), "Put back. What had the same name was replaced.");
        // Replaced is only said of a replace
        assert_eq!(put_back_text(&result(0.0, 0.0, 0.0, 1.0, 0.0, 0), "keep-both"), "Put back.");
        assert_eq!(
            put_back_text(&result(1.0, 0.0, 0.0, 0.0, 1.0, 2), "skip"),
            "Put back. What differs now was left as it is. 2 were not there then."
        );
        assert_eq!(put_back_text(&result(0.0, 0.0, 0.0, 0.0, 0.0, 1), "keep-both"), "1 was not there then.");
    }

    #[test]
    fn names_what_went_wrong() {
        let failure = |error: &str| Failure { error: error.into(), detail: String::new() };
        assert_eq!(error_text(&failure("refused")), "That cannot be done from here now. Choose the folder again and try once more.");
        assert_eq!(error_text(&failure("outside")), "Only what is inside your home folder comes back here.");
        assert_eq!(error_text(&failure("repository")), "The backup could not be read.");
        assert_eq!(error_text(&failure("failed")), "That did not work.");
        assert_eq!(error_text(&failure("usage")), "That did not work.");
    }

    #[test]
    fn says_how_long_ago() {
        let now = i18n::now();
        assert_eq!(ago(now - 3.0 * 3600.0), "3 hours ago");
        assert_eq!(ago(now - 20.0), "this minute");
        assert_eq!(ago(now - 125.0), "2 minutes ago");
        assert_eq!(ago(now - 8.0 * 86400.0 - 30.0), "8 days ago");
        assert_eq!(ago(now - 86400.0 - 30.0), "yesterday");
    }
}
