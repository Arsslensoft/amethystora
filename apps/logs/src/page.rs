//! The page of Amethystora Logs, as logs.js drew it: the sidebar, a view of the journal with its search,
//! time range, level and the entry open beside it, and the apps and services. What it shows comes from
//! backend.rs. Everything is built as widgets with text in them and never as markup: a log message is
//! whatever any program on the machine chose to write.

use std::rc::{Rc, Weak};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use amethystora_ui::frame::{self, NavButton};
use amethystora_ui::gem::{Gem, Motion};
use amethystora_ui::i18n::{self, DateStyle, Piece};
use amethystora_ui::{adw, glib, gtk, pango, process, t, t_parts, ui};
use gtk::prelude::*;

use crate::backend::{self, Boot, Entry, Kind, Scope, Source, Target, UnitItem, COREDUMP};
use crate::icons::icon;
use crate::layout::{Clip, Exact, Flex, Panes, Tiles, Toolbar};
use crate::list::{self, Item, Rows, Slot, SlotRow};
use crate::{Logs, PageKind};

static START: OnceLock<Instant> = OnceLock::new();

/// When the window started, for timing it
pub fn mark_start() {
    START.get_or_init(Instant::now);
}

pub fn since_start() -> u128 {
    START.get().map(|start| start.elapsed().as_millis()).unwrap_or(0)
}

/// The views of the sidebar: what each is called, says about itself, and says when it is empty
fn category(name: &str) -> (String, String, String) {
    match name {
        "important" => (
            t!("Important"),
            t!("Errors and worse, from everything on the machine: what failed, and what is worth a look."),
            t!("Nothing went wrong"),
        ),
        "all" => (t!("All logs"), t!("Everything the system, the desktop and the apps wrote down."), t!("Nothing was logged")),
        "session" => (t!("Your session"), t!("Your desktop, and the apps and services you started."), t!("Nothing was logged")),
        "system" => (t!("System"), t!("The services that run the machine, whoever is signed in."), t!("Nothing was logged")),
        "kernel" => (
            t!("Kernel and hardware"),
            t!("The kernel and its drivers: devices plugged in and out, disks, graphics, power."),
            t!("Nothing was logged"),
        ),
        "security" => (
            t!("Security"),
            t!("Sign-ins, sudo and polkit, and what the audit rules or the file-watch sensor caught."),
            t!("Nothing was logged"),
        ),
        _ => (t!("Crashes"), t!("Programs that crashed, and where in their code they were when they did."), t!("Nothing crashed")),
    }
}

/// The views made of the system's journal, which only administrators can read
const SYSTEM_VIEWS: [&str; 3] = ["system", "kernel", "security"];

fn ranges() -> [(&'static str, String); 6] {
    [
        ("boot", t!("This boot")),
        ("previous", t!("Previous boot")),
        ("hour", t!("Last hour")),
        ("day", t!("Last 24 hours")),
        ("week", t!("Last 7 days")),
        ("all", t!("Everything kept")),
    ]
}

fn levels() -> [(i32, String); 5] {
    [(7, t!("Every level")), (6, t!("Info and above")), (5, t!("Notices and above")), (4, t!("Warnings and errors")), (3, t!("Errors only"))]
}

// --- Words ---------------------------------------------------------------------------------------------

fn seconds(millis: i64) -> f64 {
    millis.div_euclid(1000) as f64
}

fn short_day(millis: i64) -> String {
    i18n::date(seconds(millis), DateStyle::WeekdayDayMonthShort)
}

/// To the millisecond, as the entry has it
fn full_time(millis: i64) -> String {
    let day = i18n::date(seconds(millis), DateStyle::WeekdayDayMonthYear);
    // fractionalSecondDigits: 3, with the language's own decimal sign and digits
    let half = i18n::number(1.5);
    let sign: String = half.chars().skip(1).take(half.chars().count().saturating_sub(2)).collect();
    let fraction = i18n::with(|i18n| i18n.number_with(millis.rem_euclid(1000) as f64, 0, 0, 3));
    let time = format!("{}{sign}{fraction}", i18n::date(seconds(millis), DateStyle::Time24Seconds));
    t!("{day}, {time}", day = day, time = time)
}

fn capped(count: usize) -> String {
    let cap = backend::COUNT_CAP;
    if count >= cap { t!("{count}+", count = cap - 1) } else { i18n::number(count as f64) }
}

/// A sentence given as parts, with each value written the way `value` says
pub fn sentence(pieces: Vec<Piece>, value: impl Fn(&str) -> String) -> String {
    pieces
        .into_iter()
        .map(|piece| match piece {
            Piece::Text(text) => text,
            Piece::Value(name) => value(&name),
        })
        .collect()
}

/// Text that keeps to its own direction inside a sentence, as <bdi> did
fn isolated(text: &str) -> String {
    format!("\u{2068}{text}\u{2069}")
}

/// The time range, for the sentences that name it, which choose their words by { range }: one of the
/// ranges, an earlier boot 'dated' by its first day or not, or the two minutes 'around' an entry
fn range_values(range: &str, boots: &[Boot]) -> (String, String) {
    if ranges().iter().any(|(value, _)| *value == range) {
        return (range.to_string(), String::new());
    }
    if let Some(id) = range.strip_prefix("b:") {
        return match boots.iter().find(|boot| boot.id == id).and_then(|boot| boot.first) {
            Some(first) => ("dated".into(), short_day(first)),
            None => ("undated".into(), String::new()),
        };
    }
    ("around".into(), String::new())
}

fn boot_label(boot: &Boot) -> String {
    let Some(first) = boot.first else { return boot.id.chars().take(12).collect() };
    let minutes = |millis: i64| i18n::date(seconds(millis), DateStyle::Time24);
    let day = short_day(first);
    match boot.last {
        // A boot that ran past midnight says the day it ended, or its end reads as the same day's
        Some(last) if short_day(last) != day => {
            t!("{day}, {start} – {endDay}, {end}", day = &day, start = minutes(first), endDay = short_day(last), end = minutes(last))
        }
        Some(last) => t!("{day}, {start}–{end}", day = &day, start = minutes(first), end = minutes(last)),
        None => t!("{day}, {time}", day = &day, time = minutes(first)),
    }
}

// --- The parts that stay ---------------------------------------------------------------------------------

/// The sidebar, the list and the open entry's panel, which are made once and stay; the rest of the page is
/// render()'s
pub struct Parts {
    pub root: gtk::Widget,
    view: gtk::Box,
    floating: gtk::WindowControls,
    nav: Vec<(&'static str, NavButton, Option<gtk::Label>)>,
    sources: NavButton,
    usage: gtk::Label,
    hint: gtk::Label,
    /// The list, its sticky day and the pill for new entries, over one another
    list_wrap: gtk::Overlay,
    stack: gtk::Stack,
    state_scroller: gtk::ScrolledWindow,
    pub scroller: gtk::ScrolledWindow,
    pub list_view: gtk::ListView,
    pub model: Rows,
    foot: gtk::Box,
    sticky: Clip,
    sticky_label: gtk::Label,
    fresh: gtk::Button,
    fresh_label: gtk::Label,
    detail: gtk::Box,
    detail_scroller: gtk::ScrolledWindow,
    detail_inner: gtk::Box,
}

/// The parts of the page that change without the whole of it being drawn again
#[derive(Default)]
pub struct Live {
    pub list_shown: bool,
    panes: Option<Panes>,
    head: Option<gtk::Box>,
    actions: Option<gtk::Box>,
    count: Option<gtk::Label>,
    notice: Option<gtk::Box>,
    access: Option<gtk::Widget>,
    toolbar: Option<Toolbar>,
    range: Option<gtk::Widget>,
    search: Option<gtk::Text>,
    filter: Option<gtk::Text>,
    title_row: Option<gtk::Box>,
    loading: Option<gtk::Box>,
    message: Option<(gtk::ScrolledWindow, gtk::Label, String)>,
    /// The list going back to the newest, kept while it plays
    scrolling: Option<adw::TimedAnimation>,
}

fn upgrade(me: &Weak<Logs>) -> Option<Rc<Logs>> {
    me.upgrade()
}

impl Parts {
    pub fn new(me: &Weak<Logs>) -> Parts {
        // The wordmark of the boot splash and the login screen, the glowing stone as its "a"
        let brand = amethystora_ui::brand::brand(&t!("Logs"), 21.0);
        brand.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Logs"))]);
        let sidebar = frame::Sidebar::new(&brand, 18);
        let nav_box = frame::nav(3, None, &t!("Logs"));
        let mut nav = Vec::new();
        for (name, glyph, title) in [
            ("important", "alert", t!("Important")),
            ("all", "list", t!("All")),
            ("session", "user", t!("Your session")),
            ("system", "layers", t!("System")),
            ("kernel", "chip", t!("Kernel and hardware")),
            ("security", "shield", t!("Security")),
            ("crashes", "crash", t!("Crashes")),
        ] {
            let button = NavButton::new(icon(glyph, 20), &title, 12);
            wrap_name(&button);
            let badge = (name == "important" || name == "crashes").then(|| {
                let badge = frame::badge();
                button.append(&badge);
                badge
            });
            let weak = me.clone();
            button.button.connect_clicked(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.open_category(name);
                }
            });
            nav_box.append(&button.button);
            nav.push((name, button, badge));
        }
        sidebar.body.append(&nav_box);
        let browse = frame::nav(3, Some(&t!("Browse")), &t!("Browse"));
        let sources = NavButton::new(icon("apps", 20), &t!("Apps and services"), 12);
        wrap_name(&sources);
        let weak = me.clone();
        sources.button.connect_clicked(move |_| {
            if let Some(this) = upgrade(&weak) {
                this.open_view("sources");
            }
        });
        browse.append(&sources.button);
        sidebar.body.append(&browse);

        sidebar.foot.set_spacing(12);
        let manual = ui::button(&t!("Logs in the manual"), Some(icon("book", 18)), &["link", "quiet"], || process::manual("troubleshooting#the-logs"));
        manual.set_halign(gtk::Align::Start);
        sidebar.foot.append(&manual);
        let usage = wrapped("", &["terminal"]);
        usage.set_visible(false);
        sidebar.foot.append(&usage);
        let terminal = frame::terminal(&t!("Also in a terminal:"), "journalctl");
        let hint = terminal.last_child().and_downcast::<gtk::Label>().expect("the command");
        hint.set_wrap_mode(pango::WrapMode::WordChar);
        list::unhyphenated(&hint);
        sidebar.foot.append(&terminal);
        let column = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&Exact::new(&sidebar.root)).build();
        column.add_css_class("sidebar-scroller");

        // The list, made once: the list view keeps the rows it made, and binds them to what scrolls in
        let model = Rows::default();
        let foot = ui::vbox(8, &["list-foot"]);
        let factory = gtk::SignalListItemFactory::new();
        {
            let weak = me.clone();
            factory.connect_setup(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let row = SlotRow::new();
                let click = gtk::GestureClick::new();
                click.set_button(1);
                let (opener, target) = (weak.clone(), row.downgrade());
                click.connect_released(move |_, _, _, _| {
                    if let (Some(this), Some(row)) = (upgrade(&opener), target.upgrade())
                        && let Some(cursor) = row.cursor()
                    {
                        this.open_entry(&cursor, false);
                    }
                });
                row.parts().entry.add_controller(click);
                item.set_child(Some(&row));
                if let Some(this) = upgrade(&weak) {
                    this.rows.borrow_mut().push(row.downgrade());
                }
            });
        }
        {
            let weak = me.clone();
            factory.connect_bind(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let (Some(this), Some(row)) = (upgrade(&weak), item.child().and_downcast::<SlotRow>()) else { return };
                let Some(slot) = item.item().and_downcast::<Item>().and_then(|item| item.slot()) else { return };
                let position = item.position();
                let after_day = position > 0 && matches!(this.parts.model.slot(position - 1), Some(Slot::Day(_)));
                let entry = matches!(slot, Slot::Entry(_));
                item.set_activatable(entry);
                item.set_focusable(entry);
                let selected = this.state.borrow().selected.clone();
                let matcher = this.matcher.borrow().clone();
                let mark = *this.mark.borrow();
                row.bind(&slot, after_day, &selected, matcher.as_deref(), &mark, this.parts.foot.upcast_ref());
            });
        }
        {
            let weak = me.clone();
            factory.connect_unbind(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let (Some(this), Some(row)) = (upgrade(&weak), item.child().and_downcast::<SlotRow>()) else { return };
                row.unbind(this.parts.foot.upcast_ref());
            });
        }
        let list_view = gtk::ListView::new(Some(gtk::NoSelection::new(Some(model.clone()))), Some(factory));
        list_view.add_css_class("rows");
        list_view.update_property(&[gtk::accessible::Property::Label(&t!("Logs"))]);
        {
            let weak = me.clone();
            list_view.connect_activate(move |_, position| {
                let Some(this) = upgrade(&weak) else { return };
                if let Some(Slot::Entry(entry)) = this.parts.model.slot(position) {
                    this.open_entry(&entry.cursor, false);
                }
            });
        }
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();
        scroller.set_child(Some(&list_view));
        {
            let weak = me.clone();
            scroller.vadjustment().connect_value_changed(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.scrolled();
                }
            });
            let weak = me.clone();
            scroller.vadjustment().connect_changed(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.list_resized();
                }
            });
        }
        // The day's heading stays at the top while its entries scroll under it, as position: sticky did
        let sticky_label = gtk::Label::builder().xalign(0.0).css_classes(["day"]).build();
        let sticky = Clip::new(&sticky_label);
        sticky.set_valign(gtk::Align::Start);
        sticky.set_can_target(false);
        sticky.set_visible(false);
        let fresh_label = ui::line("", &[]);
        let fresh_content = ui::hbox(0, &[]);
        let up = icon("chevron-down", 15);
        up.add_css_class("up");
        fresh_content.append(&up);
        fresh_content.append(&fresh_label);
        let fresh = gtk::Button::builder().child(&fresh_content).css_classes(["plain", "fresh"]).halign(gtk::Align::Center).valign(gtk::Align::Start).build();
        fresh.set_visible(false);
        {
            let weak = me.clone();
            fresh.connect_clicked(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.scroll_to_top();
                }
            });
        }
        // The list stays in its place; what is shown while it is not the list is beside it, in a stack
        let state_scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();
        let stack = gtk::Stack::builder().vexpand(true).build();
        stack.add_child(&scroller);
        stack.add_child(&state_scroller);
        let list_wrap = gtk::Overlay::builder().child(&stack).vexpand(true).build();
        list_wrap.add_overlay(&sticky);
        list_wrap.add_overlay(&fresh);

        let detail_inner = ui::vbox(16, &["detail-inner"]);
        let detail_scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&natural(&detail_inner)).build();
        // The window's own buttons, which the page had in the window's frame, over the open entry as
        // GNOME's split views have them over their last pane
        let detail_bar = ui::hbox(0, &["detail-bar"]);
        let detail_controls = frame::controls();
        detail_controls.set_hexpand(true);
        detail_controls.set_halign(gtk::Align::End);
        detail_bar.append(&detail_controls);
        let detail = ui::vbox(0, &["detail"]);
        detail.append(&detail_bar);
        detail.append(&detail_scroller);
        detail.update_property(&[gtk::accessible::Property::Label(&t!("The entry"))]);

        let view = ui::vbox(0, &["view"]);
        view.set_hexpand(true);
        let floating = frame::controls();
        floating.add_css_class("floating");
        floating.set_halign(gtk::Align::End);
        floating.set_valign(gtk::Align::Start);
        let main = gtk::Overlay::builder().child(&view).hexpand(true).build();
        main.add_overlay(&floating);
        let root = frame::with_sidebar(&column, &main, -1, false);
        Parts {
            root,
            view,
            floating,
            nav,
            sources,
            usage,
            hint,
            list_wrap,
            stack,
            state_scroller,
            scroller,
            list_view,
            model,
            foot,
            sticky,
            sticky_label,
            fresh,
            fresh_label,
            detail,
            detail_scroller,
            detail_inner,
        }
    }
}

// --- Building blocks ------------------------------------------------------------------------------------

/// The page's button(): an icon and words, either of them left out
fn button_widget(label: &str, glyph: Option<&str>, small: bool, primary: bool) -> gtk::Button {
    let mut classes = vec!["btn"];
    if small {
        classes.push("small");
    }
    if primary {
        classes.push("primary");
    }
    let content = ui::hbox(0, &["content"]);
    if let Some(glyph) = glyph {
        content.append(&icon(glyph, 16));
    }
    if !label.is_empty() {
        content.append(&ui::line(label, &[]));
    }
    let button = gtk::Button::builder().child(&content).build();
    for class in classes {
        button.add_css_class(class);
    }
    button
}

fn copy_button(label: &str, text: &str) -> gtk::Button {
    ui::copy_button(text, icon("copy", 15), label, &t!("Copied"), label, &["btn", "small"])
}

fn callout(tone: &str, title: &str, text: &str, extra: Vec<gtk::Widget>) -> gtk::Box {
    let callout = ui::hbox(14, &["callout", tone]);
    let mark = icon(if tone == "check" { "alert" } else { "info" }, 20);
    mark.set_valign(gtk::Align::Start);
    callout.append(&mark);
    let body = ui::vbox(6, &["grow"]);
    body.set_hexpand(true);
    body.append(&wrapped(title, &["strong"]));
    if !text.is_empty() {
        body.append(&ui::limit(&wrapped(text, &["p"]), 640));
    }
    for child in extra {
        body.append(&child);
    }
    callout.append(&body);
    callout
}

fn command_block(lead: &str, command: &str) -> gtk::Box {
    let wrap = ui::vbox(8, &["command-wrap"]);
    if !lead.is_empty() {
        wrap.append(&wrapped(lead, &["soft", "small"]));
    }
    let line = ui::hbox(12, &["command"]);
    let code = wrapped(command, &["code"]);
    code.set_hexpand(true);
    code.set_selectable(true);
    code.set_wrap_mode(pango::WrapMode::Char);
    code.set_direction(gtk::TextDirection::Ltr);
    line.append(&code);
    let copy = copy_button(&t!("Copy"), command);
    copy.set_valign(gtk::Align::Center);
    line.append(&copy);
    wrap.append(&ui::limit(&line, 620));
    wrap
}

/// One choice of a select: its value, its words, and the group it is listed under
struct Choice {
    value: String,
    label: String,
    group: Option<String>,
}

/// A <select>: as wide as its widest choice, its choices in a popover
fn select(accessible: &str, choices: Vec<Choice>, value: &str, on_change: impl Fn(String) + 'static) -> gtk::MenuButton {
    let words = gtk::Stack::builder().hhomogeneous(true).vhomogeneous(true).build();
    let list = ui::vbox(0, &[]);
    let button = gtk::MenuButton::builder().css_classes(["select"]).build();
    let popover = gtk::Popover::builder().child(&list).has_arrow(false).css_classes(["choices"]).build();
    let on_change = Rc::new(on_change);
    let mut group: Option<String> = None;
    for choice in &choices {
        // A choice in a group is indented by four spaces, as Chromium measured and showed it
        let shown = if choice.group.is_some() { format!("    {}", choice.label) } else { choice.label.clone() };
        let label = gtk::Label::builder().label(&shown).xalign(0.0).ellipsize(pango::EllipsizeMode::End).build();
        words.add_named(&label, Some(&choice.value));
        if choice.group != group {
            if let Some(name) = &choice.group {
                list.append(&ui::line(name, &["group"]));
            }
            group = choice.group.clone();
        }
        let option = gtk::Button::builder().child(&ui::line(&choice.label, &[])).css_classes(["plain", "option"]).build();
        if choice.group.is_some() {
            option.add_css_class("grouped");
        }
        if choice.value == value {
            option.add_css_class("current");
        }
        let (popover, on_change, chosen) = (popover.clone(), on_change.clone(), choice.value.clone());
        option.connect_clicked(move |_| {
            popover.popdown();
            on_change(chosen.clone());
        });
        list.append(&option);
    }
    words.set_visible_child_name(value);
    let content = ui::hbox(7, &[]);
    let limited = ui::limit(&words, 192);
    limited.set_hexpand(true);
    content.append(&limited);
    content.append(&icon("chevron-down", 16));
    button.set_child(Some(&content));
    button.set_popover(Some(&popover));
    button.set_valign(gtk::Align::Center);
    button.update_property(&[gtk::accessible::Property::Label(accessible)]);
    button
}

/// Content to scroll, laid out at its natural height: a viewport's default gives each child its minimum
/// and shares out the rest, which squeezed the boxes the page capped with max-height
fn natural(child: &impl IsA<gtk::Widget>) -> gtk::Viewport {
    gtk::Viewport::builder().vscroll_policy(gtk::ScrollablePolicy::Natural).child(child).build()
}

/// The mark while the journal is read, its stem blinking as a cursor does
fn reading() -> Gem {
    let gem = Gem::plain(26.0);
    gem.set_motion(Motion::Blink);
    gem
}

fn section(heading: &str, count: usize, body: &impl IsA<gtk::Widget>) -> gtk::Box {
    let section = ui::vbox(12, &["section"]);
    let head = ui::hbox(12, &["section-head"]);
    // In capitals the language's own way; the stylesheet's text-transform is not measured by GTK
    let title = ui::line(&heading.to_uppercase(), &["h2"]);
    title.set_valign(gtk::Align::Baseline);
    let note = ui::line(&i18n::number(count as f64), &["section-note"]);
    note.set_valign(gtk::Align::Baseline);
    head.append(&title);
    head.append(&note);
    section.append(&head);
    section.append(body);
    section
}

impl Logs {
    fn on(&self, f: impl Fn(&Rc<Logs>) + 'static) -> impl Fn() + 'static {
        let weak = self.me.clone();
        move || {
            if let Some(this) = weak.upgrade() {
                f(&this);
            }
        }
    }

    fn button(&self, label: &str, glyph: Option<&str>, small: bool, primary: bool, f: impl Fn(&Rc<Logs>) + 'static) -> gtk::Button {
        let button = button_widget(label, glyph, small, primary);
        let act = self.on(f);
        button.connect_clicked(move |_| act());
        button
    }

    fn icon_button(&self, glyph: &str, size: i32, accessible: &str, f: impl Fn(&Rc<Logs>) + 'static) -> gtk::Button {
        let button = gtk::Button::builder().child(&icon(glyph, size)).css_classes(["plain", "icon-btn"]).valign(gtk::Align::Center).build();
        button.update_property(&[gtk::accessible::Property::Label(accessible)]);
        let act = self.on(f);
        button.connect_clicked(move |_| act());
        button
    }

    // --- The list ----------------------------------------------------------------------------------

    /// What the list area shows while it is not the list: the reading mark, what went wrong, or nothing
    fn set_list_child(&self, child: Option<&gtk::Widget>) {
        let parts = &self.parts;
        match child {
            Some(child) => {
                self.live.borrow_mut().list_shown = false;
                parts.state_scroller.set_child(Some(&natural(child)));
                parts.stack.set_visible_child(&parts.state_scroller);
                parts.model.clear();
            }
            None => {
                self.live.borrow_mut().list_shown = true;
                parts.stack.set_visible_child(&parts.scroller);
                parts.state_scroller.set_child(gtk::Widget::NONE);
            }
        }
        self.update_sticky();
    }

    fn empty_state(&self) -> gtk::Widget {
        let state = self.state.borrow();
        let boots = state.overview.as_ref().map(|o| o.boots.clone()).unwrap_or_default();
        let (range, date) = range_values(&state.range, &boots);
        let (title, text) = if !state.search.is_empty() {
            // The search as it was typed, in whichever direction its own words run
            let pieces = t_parts!("No message {range, select, boot {from this boot} previous {from the previous boot} hour {from the last hour} day {from the last 24 hours} week {from the last 7 days} all {from everything kept} dated {from the boot of {date}} around {in those two minutes} other {from that boot}} has “{search}” in it. The search looks at what was said, not at who said it.", range = &range, date = &date, search = &state.search);
            (t!("Nothing matches"), sentence(pieces, |name| if name == "search" { isolated(&state.search) } else { date.clone() }))
        } else if let Some(source) = &state.source {
            (
                t!("Nothing from {source}", source = &source.label),
                t!("It wrote nothing to the journal in {range, select, boot {this boot} previous {the previous boot} hour {the last hour} day {the last 24 hours} week {the last 7 days} all {everything kept} dated {the boot of {date}} around {those two minutes} other {that boot}} that this view shows.", range = &range, date = &date),
            )
        } else if state.category == "important" || state.category == "crashes" {
            (category(&state.category).2, t!("Not in {range, select, boot {this boot} previous {the previous boot} hour {the last hour} day {the last 24 hours} week {the last 7 days} all {everything kept} dated {the boot of {date}} around {those two minutes} other {that boot}}, at least.", range = &range, date = &date))
        } else {
            (category(&state.category).2, t!("Not in {range, select, boot {this boot} previous {the previous boot} hour {the last hour} day {the last 24 hours} week {the last 7 days} all {everything kept} dated {the boot of {date}} around {those two minutes} other {that boot}}.", range = &range, date = &date))
        };
        let mut actions: Vec<gtk::Widget> = Vec::new();
        if state.range != "all" {
            actions.push(self.button(&t!("Look further back"), None, true, false, |this| this.set_range("all")).upcast());
        }
        if state.level < 7 && state.category != "important" {
            actions.push(self.button(&t!("Every level"), None, true, false, |this| this.set_level(7)).upcast());
        }
        let good = state.search.is_empty() && state.source.is_none() && (state.category == "important" || state.category == "crashes");
        drop(state);
        let empty = ui::vbox(8, &["empty"]);
        let mark = ui::hbox(0, &["empty-icon", if good { "ok" } else { "info" }]);
        mark.set_halign(gtk::Align::Center);
        mark.set_hexpand(false);
        let glyph = icon(if good { "shield" } else { "search" }, 28);
        glyph.set_hexpand(true);
        mark.append(&glyph);
        empty.append(&mark);
        let centered = |label: gtk::Label| {
            label.set_justify(gtk::Justification::Center);
            label.set_xalign(0.5);
            label
        };
        empty.append(&centered(wrapped(&list::in_page_direction(&title), &["h2"])));
        empty.append(&centered(wrapped(&list::in_page_direction(&text), &["p"])));
        if !actions.is_empty() {
            let row = ui::wrap(10, 10, &["actions"]);
            row.set_align(0.5);
            row.set_halign(gtk::Align::Center);
            for action in actions {
                row.append(&action);
            }
            empty.append(&row);
        }
        let limited = ui::limit_center(&empty, 520);
        limited.set_valign(gtk::Align::Start);
        limited.upcast()
    }

    pub(crate) fn render_count(&self) {
        let Some(count_label) = self.live.borrow().count.clone() else { return };
        let state = self.state.borrow();
        let count = state.entries.len();
        let text = if count == 0 {
            String::new()
        } else if state.more {
            t!("{count, plural, one {The newest # entry} other {The newest # entries}}", count = count)
        } else {
            t!("{count, plural, one {# entry} other {# entries}}", count = count)
        };
        count_label.set_label(&text);
    }

    pub(crate) fn render_foot(&self) {
        let foot = &self.parts.foot;
        ui::clear(foot);
        let (loading_more, more, any, range) = {
            let state = self.state.borrow();
            (state.loading_more, state.more, !state.entries.is_empty(), state.range.clone())
        };
        if loading_more {
            let line = ui::hbox(8, &["soft"]);
            line.set_halign(gtk::Align::Center);
            let spinner = icon("spinner", 16);
            spinner.set_spinning(true);
            line.append(&spinner);
            line.append(&ui::line(&t!("Reading older entries…"), &[]));
            foot.append(&line);
        } else if more {
            let older = self.button(&t!("Show older"), Some("chevron-down"), true, false, |this| this.load_older());
            older.set_halign(gtk::Align::Center);
            foot.append(&older);
        } else if any {
            let text = if range == "all" {
                t!("That is as far back as the journal goes.")
            } else {
                let boots = self.state.borrow().overview.as_ref().map(|o| o.boots.clone()).unwrap_or_default();
                let (range, date) = range_values(&range, &boots);
                t!("That is everything from {range, select, boot {this boot} previous {the previous boot} hour {the last hour} day {the last 24 hours} week {the last 7 days} all {everything kept} dated {the boot of {date}} around {those two minutes} other {that boot}}.", range = &range, date = &date)
            };
            let label = wrapped(&text, &["faint", "small"]);
            label.set_justify(gtk::Justification::Center);
            label.set_xalign(0.5);
            foot.append(&label);
        }
        self.render_count();
    }

    pub(crate) fn render_fresh(&self) {
        let fresh = self.state.borrow().fresh;
        self.parts.fresh.set_visible(fresh > 0 && self.live.borrow().list_shown);
        self.parts.fresh_label.set_label(&t!("{count, plural, one {# new entry} other {# new entries}}", count = fresh));
    }

    pub(crate) fn render_list(&self) {
        if self.live.borrow().panes.is_none() {
            return;
        }
        let (loading, searching, error, regex, empty) = {
            let state = self.state.borrow();
            (state.loading, !state.search.is_empty(), state.error.clone(), state.regex, state.entries.is_empty())
        };
        self.live.borrow_mut().loading = None;
        if loading {
            let box_ = ui::vbox(12, &["loading"]);
            box_.set_valign(gtk::Align::Start);
            let inner = ui::vbox(12, &[]);
            inner.set_valign(gtk::Align::Center);
            inner.set_vexpand(true);
            inner.append(&reading());
            let words = ui::line(&if searching { t!("Searching the journal…") } else { t!("Reading the journal…") }, &[]);
            words.set_halign(gtk::Align::Center);
            inner.append(&words);
            box_.append(&inner);
            // min-height: 40vh
            box_.set_size_request(-1, (0.4 * self.vh.get() as f64).round() as i32);
            self.live.borrow_mut().loading = Some(box_.clone());
            self.set_list_child(Some(box_.upcast_ref()));
            self.render_count();
            self.render_fresh();
            return;
        }
        if self.system_view() && error.contains("insufficient permissions") {
            // The system's logs asked for by somebody who is not an administrator: the callout over the list says so
            // already, and journalctl's own words would only say it again
            let nothing = ui::vbox(0, &[]);
            self.set_list_child(Some(nothing.upcast_ref()));
            self.render_count();
            self.render_fresh();
            return;
        }
        if !error.is_empty() {
            // What journalctl said is in its own words, which may run the other way from the page's
            let mut extra: Vec<gtk::Widget> = Vec::new();
            if regex {
                extra.push(ui::limit(&wrapped(&t!("If it is the regular expression, switch it off to search for the text as typed."), &["p"]), 640).upcast());
            }
            let pad = ui::vbox(0, &["list-pad"]);
            pad.append(&callout("check", &t!("journalctl could not read that"), &isolated(&error), extra));
            pad.set_valign(gtk::Align::Start);
            self.set_list_child(Some(pad.upcast_ref()));
            self.render_count();
            self.render_fresh();
            return;
        }
        if empty {
            let child = self.empty_state();
            self.set_list_child(Some(&child));
            self.render_count();
            self.render_fresh();
            return;
        }
        let entries = self.state.borrow().entries.clone();
        self.render_foot();
        let began = since_start();
        self.list_of.set(self.state.borrow().request);
        self.parts.model.replace_gradually(&entries, self.parts.list_view.upcast_ref());
        glib::g_debug!("amethystora-logs", "model of {} rows from {began} to {} ms", self.parts.model.len(), since_start());
        self.set_list_child(None);
        self.parts.scroller.vadjustment().set_value(0.0);
        self.parts.list_view.scroll_to(0, gtk::ListScrollFlags::NONE, None);
        self.render_fresh();
    }

    /// The entry's row scrolled into sight by as little as it takes (scrollIntoView, block: nearest), and
    /// given the focus without scrolling for it
    pub(crate) fn reveal(&self, position: u32, cursor: &str, focus: bool) {
        let view = &self.parts.list_view;
        let row = self.rows.borrow().iter().filter_map(|row| row.upgrade()).find(|row| row.cursor().as_deref() == Some(cursor));
        let bounds = row.as_ref().filter(|row| row.is_mapped()).and_then(|row| row.compute_bounds(view));
        let Some(bounds) = bounds else {
            let flags = if focus { gtk::ListScrollFlags::FOCUS } else { gtk::ListScrollFlags::NONE };
            view.scroll_to(position, flags, None);
            return;
        };
        let adjustment = self.parts.scroller.vadjustment();
        let (top, bottom, height) = (bounds.y() as f64, (bounds.y() + bounds.height()) as f64, view.height() as f64);
        if top < 0.0 {
            adjustment.set_value(adjustment.value() + top);
        } else if bottom > height {
            adjustment.set_value(adjustment.value() + (bottom - height).min(top));
        }
        if focus && let Some(item) = row.and_then(|row| row.parent()) {
            item.grab_focus();
        }
    }

    /// Back to the newest, smoothly
    pub(crate) fn scroll_to_top(&self) {
        let adjustment = self.parts.scroller.vadjustment();
        let target = adw::PropertyAnimationTarget::new(&adjustment, "value");
        let animation = adw::TimedAnimation::new(&self.parts.scroller, adjustment.value(), 0.0, 300, target);
        animation.set_easing(adw::Easing::EaseInOutCubic);
        adw::prelude::AnimationExt::play(&animation);
        self.live.borrow_mut().scrolling = Some(animation);
    }

    /// The list moved: what came in while away is seen once back at the top, and older entries are read
    /// before the end is reached
    /// The list made taller or shorter, as the head above it changes: it keeps its place as the page's
    /// scrollTop did, where the list view would keep an entry where it was
    pub(crate) fn list_resized(&self) {
        let adjustment = self.parts.scroller.vadjustment();
        let page = adjustment.page_size();
        let before = self.list_page.replace(page);
        if before <= 0.0 || (page - before).abs() < 0.5 || self.list_restoring.get() {
            return;
        }
        self.list_restoring.set(true);
        let weak = self.me.clone();
        glib::idle_add_local_once(move || {
            let Some(this) = weak.upgrade() else { return };
            this.list_restoring.set(false);
            let adjustment = this.parts.scroller.vadjustment();
            let keep = this.list_top.get().min(adjustment.upper() - adjustment.page_size()).max(0.0);
            adjustment.set_value(keep);
            this.list_top.set(adjustment.value());
        });
    }

    pub(crate) fn scrolled(&self) {
        let adjustment = self.parts.scroller.vadjustment();
        if self.list_restoring.get() {
            return;
        }
        self.list_top.set(adjustment.value());
        let shown = self.live.borrow().list_shown;
        if shown {
            let fresh = self.state.borrow().fresh;
            if adjustment.value() <= 8.0 && fresh > 0 {
                self.state.borrow_mut().fresh = 0;
                self.render_fresh();
            }
            let wanted = {
                let state = self.state.borrow();
                state.more && !state.loading_more && !state.loading
            };
            if wanted && adjustment.upper() - adjustment.value() - adjustment.page_size() < 400.0 {
                self.load_older();
            }
        }
        self.update_sticky();
    }

    /// The day of the row at the top, over it, cut off where the next day's heading comes up under it
    pub(crate) fn update_sticky(&self) {
        let parts = &self.parts;
        let shown = self.live.borrow().list_shown && parts.scroller.vadjustment().value() > 0.0;
        if !shown {
            parts.sticky.set_visible(false);
            return;
        }
        let view = &parts.list_view;
        let x = view.width() as f64 / 2.0;
        let row_at = |y: f64| view.pick(x, y, gtk::PickFlags::DEFAULT).and_then(|widget| widget.ancestor(SlotRow::static_type())).and_downcast::<SlotRow>();
        let Some(top) = row_at(1.0) else {
            parts.sticky.set_visible(false);
            return;
        };
        let time = match top.slot() {
            Some(Slot::Day(time)) => time,
            Some(Slot::Entry(entry)) => entry.time,
            _ => match self.state.borrow().entries.last() {
                Some(entry) => entry.time,
                None => return,
            },
        };
        parts.sticky_label.set_label(&list::day_name(time).to_uppercase());
        let height = parts.sticky_label.measure(gtk::Orientation::Vertical, view.width()).1 as f64;
        let mut shown_part = None;
        if let Some(next) = row_at(height - 1.0).filter(|row| row != &top && matches!(row.slot(), Some(Slot::Day(_))))
            && let Some(point) = next.compute_point(view, &amethystora_ui::graphene::Point::new(0.0, 0.0))
            && point.y() > 0.0
        {
            shown_part = Some(point.y());
        }
        parts.sticky.set_shown(shown_part);
        parts.sticky.set_visible(true);
    }

    // --- One entry ---------------------------------------------------------------------------------

    fn facts(fields: &std::collections::BTreeMap<String, String>) -> Vec<(String, String, bool)> {
        let get = |key: &str| fields.get(key).cloned().unwrap_or_default();
        let either = |a: &str, b: &str| {
            let value = get(a);
            if value.is_empty() { get(b) } else { value }
        };
        // The fields worth a name of their own, in the order they are shown, before the rest. Those that
        // are names, paths and command lines read left to right in any language.
        let program = {
            let name = either("_COMM", "SYSLOG_IDENTIFIER");
            let pid = get("_PID");
            if pid.is_empty() {
                name
            } else if name.is_empty() {
                t!("process {pid}", pid = &pid)
            } else {
                t!("{name}, process {pid}", name = &name, pid = &pid)
            }
        };
        let unit = get("_SYSTEMD_UNIT");
        let user_unit = get("_SYSTEMD_USER_UNIT");
        let transport = get("_TRANSPORT");
        let uid = get("_UID");
        let list = [
            (t!("Program"), program, false),
            (t!("Service"), if !unit.is_empty() && unit != user_unit { unit.clone() } else { String::new() }, true),
            (t!("Your service"), user_unit, true),
            (t!("Executable"), either("_EXE", "COREDUMP_EXE"), true),
            (t!("Command line"), either("_CMDLINE", "COREDUMP_CMDLINE"), true),
            (t!("Signal"), get("COREDUMP_SIGNAL_NAME"), true),
            (t!("Account"), if uid.is_empty() { String::new() } else { t!("uid {uid}", uid = &uid) }, false),
            (
                t!("Where from"),
                match transport.as_str() {
                    "kernel" => t!("The kernel"),
                    "audit" => t!("The audit log"),
                    "syslog" => "syslog".into(),
                    "journal" => t!("The journal"),
                    "stdout" => t!("What it printed"),
                    "driver" => t!("journald itself"),
                    _ => transport.clone(),
                },
                false,
            ),
            (t!("Boot"), get("_BOOT_ID"), true),
        ];
        list.into_iter().filter(|(_, value, _)| !value.is_empty()).collect()
    }

    fn detail_panel(&self, entry: &Rc<Entry>) {
        let inner = &self.parts.detail_inner;
        ui::clear(inner);
        let fields = entry.fields();
        let get = |key: &str| fields.get(key).cloned().unwrap_or_default();
        let (word, tone) = list::priority(entry.priority);
        let crash = get("MESSAGE_ID") == COREDUMP;
        let program = {
            let comm = get("COREDUMP_COMM");
            if !comm.is_empty() {
                Some(comm)
            } else {
                fields.get("COREDUMP_EXE").map(|exe| exe.rsplit('/').next().unwrap_or("").to_string())
            }
        };
        let title = if crash {
            match program.filter(|p| !p.is_empty()) {
                Some(program) => t!("{program} crashed", program = &program),
                None => t!("A program crashed"),
            }
        } else {
            entry
                .from
                .as_ref()
                .map(|from| from.label.clone())
                .filter(|label| !label.is_empty())
                .or_else(|| (!entry.source.is_empty()).then(|| entry.source.clone()))
                .unwrap_or_else(|| t!("Entry"))
        };

        let head = ui::hbox(10, &["detail-head"]);
        let pill = ui::line(&word, &["pill", tone]);
        pill.set_valign(gtk::Align::Center);
        head.append(&pill);
        let when = wrapped(&full_time(entry.time), &["faint", "small"]);
        when.set_hexpand(true);
        head.append(&when);
        head.append(&self.icon_button("close", 18, &t!("Close"), |this| this.close_entry()));
        inner.append(&head);
        let heading = wrapped(&list::in_page_direction(&title), &["detail-title"]);
        heading.set_wrap_mode(pango::WrapMode::WordChar);
        inner.append(&heading);

        // The message as it was written, every line of it
        let text = if entry.message.is_empty() { t!("(no message)") } else { entry.message.clone() };
        let message = wrapped(&text, &["message"]);
        message.set_selectable(true);
        message.set_wrap_mode(pango::WrapMode::WordChar);
        message.set_valign(gtk::Align::Start);
        if !entry.message.is_empty() {
            message.set_direction(gtk::TextDirection::Ltr);
        }
        let matcher = self.matcher.borrow().clone();
        message.set_attributes(list::marks(&text, matcher.as_deref(), &self.mark.borrow()).as_ref());
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(self.message_height())
            .child(&message)
            .css_classes(["message-box"])
            .build();
        self.live.borrow_mut().message = Some((scroller.clone(), message.clone(), text.clone()));
        inner.append(&scroller);

        let actions = ui::wrap(8, 8, &["detail-actions"]);
        actions.append(&copy_button(&t!("Copy message"), &entry.message));
        let (source, agent) = {
            let state = self.state.borrow();
            (state.source.clone(), state.agent)
        };
        if let Some(from) = entry.from.clone()
            && !source.as_ref().is_some_and(|source| source.kind == from.kind && source.id == from.id)
        {
            let button = button_widget("", Some("filter"), true, false);
            let words = gtk::Label::builder().label(list::in_page_direction(&t!("Only {source}", source = &from.label))).ellipsize(pango::EllipsizeMode::End).build();
            if let Some(content) = button.child().and_downcast::<gtk::Box>() {
                content.append(&ui::limit(&words, 220));
            }
            let act = self.on(move |this| this.set_source(Some(from.clone())));
            button.connect_clicked(move |_| act());
            actions.append(&button);
        }
        let around = entry.clone();
        let button = self.button(&t!("Around this"), Some("target"), true, false, move |this| this.around(&around));
        button.set_tooltip_text(Some(&t!("Everything from a minute before to a minute after")));
        actions.append(&button);
        // A crash, or something a service wrote, can be handed to the AI agent, which looks into it in a
        // terminal: by the crashed process's number or the service's name, which is all backend.rs passes on
        let pid = get("COREDUMP_PID");
        let target = if crash && !pid.is_empty() {
            Some(Target::Crash(pid))
        } else {
            entry.from.as_ref().filter(|from| from.kind == Kind::Unit).map(|from| Target::Unit(from.id.clone()))
        };
        if let (true, Some(target)) = (agent, target) {
            let button = self.button(&t!("Ask the agent"), Some("sparkle"), true, false, move |_| backend::diagnose(&target));
            button.set_tooltip_text(Some(&t!("Find out why, without changing anything")));
            actions.append(&button);
        }
        inner.append(&actions);

        let facts = Self::facts(&fields);
        if !facts.is_empty() {
            let grid = gtk::Grid::builder().column_spacing(16).row_spacing(8).css_classes(["facts"]).build();
            for (row, (label, value, ltr)) in facts.into_iter().enumerate() {
                let dt = ui::line(&label, &["dt"]);
                dt.set_valign(gtk::Align::Start);
                grid.attach(&dt, 0, row as i32, 1, 1);
                let dd = wrapped(&if ltr { value.clone() } else { list::in_page_direction(&value) }, &["dd"]);
                dd.set_wrap_mode(pango::WrapMode::WordChar);
                dd.set_selectable(true);
                dd.set_hexpand(true);
                if ltr {
                    dd.set_direction(gtk::TextDirection::Ltr);
                }
                grid.attach(&dd, 1, row as i32, 1, 1);
            }
            inner.append(&grid);
        }
        inner.append(&Self::every_field(&fields));
    }

    /// The fields are the journal's own, names and values, and read left to right in any language; they
    /// are laid out when the list is opened
    fn every_field(fields: &std::collections::BTreeMap<String, String>) -> gtk::Box {
        let card = ui::vbox(0, &["fields"]);
        let chevron = icon("chevron", 15).mirror_in_rtl();
        chevron.add_css_class("chevron");
        let summary = ui::hbox(0, &["content"]);
        summary.append(&chevron);
        let words = ui::line(&t!("Every field ({count})", count = fields.len()), &["grow"]);
        words.set_hexpand(true);
        summary.append(&words);
        let toggle = gtk::Button::builder().child(&summary).css_classes(["plain", "summary"]).build();
        card.append(&toggle);
        let body = ui::vbox(0, &[]);
        body.set_visible(false);
        card.append(&body);
        let everything = fields.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("\n");
        let fields = fields.clone();
        let (card_ref, body_ref) = (card.clone(), body.clone());
        toggle.connect_clicked(move |_| {
            let open = !body_ref.is_visible();
            if open && body_ref.first_child().is_none() {
                let grid = gtk::Grid::builder().column_spacing(14).row_spacing(6).css_classes(["dl"]).build();
                grid.set_direction(gtk::TextDirection::Ltr);
                for (row, (key, value)) in fields.iter().enumerate() {
                    let dt = ui::line(key, &["dt"]);
                    dt.set_direction(gtk::TextDirection::Ltr);
                    dt.set_valign(gtk::Align::Start);
                    grid.attach(&dt, 0, row as i32, 1, 1);
                    let dd = wrapped(value, &["dd"]);
                    dd.set_direction(gtk::TextDirection::Ltr);
                    dd.set_wrap_mode(pango::WrapMode::WordChar);
                    dd.set_selectable(true);
                    dd.set_hexpand(true);
                    // max-height: 12em, scrolled within, for the few that run long
                    let long = value.len() > 1200 || value.matches('\n').count() > 7;
                    if long {
                        let scroller = gtk::ScrolledWindow::builder()
                            .hscrollbar_policy(gtk::PolicyType::Never)
                            .propagate_natural_height(true)
                            .max_content_height((144.0 * amethystora_ui::icon::zoom()) as i32)
                            .child(&dd)
                            .hexpand(true)
                            .build();
                        grid.attach(&scroller, 1, row as i32, 1, 1);
                    } else {
                        grid.attach(&dd, 1, row as i32, 1, 1);
                    }
                }
                body_ref.append(&grid);
                let foot = ui::vbox(0, &["fields-foot"]);
                let copy = copy_button(&t!("Copy every field"), &everything);
                copy.set_halign(gtk::Align::Start);
                foot.append(&copy);
                body_ref.append(&foot);
            }
            body_ref.set_visible(open);
            if open {
                card_ref.add_css_class("open");
            } else {
                card_ref.remove_css_class("open");
            }
        });
        card
    }

    /// max-height: 46vh
    fn message_height(&self) -> i32 {
        ((0.46 * self.vh.get() as f64).round() as i32 - 2).max(40)
    }

    pub(crate) fn redraw_detail_marks(&self) {
        if let Some((_, label, text)) = self.live.borrow().message.clone() {
            let matcher = self.matcher.borrow().clone();
            label.set_attributes(list::marks(&text, matcher.as_deref(), &self.mark.borrow()).as_ref());
        }
    }

    pub(crate) fn render_detail(&self) {
        let Some(panes) = self.live.borrow().panes.clone() else { return };
        let entry = {
            let state = self.state.borrow();
            state.by_cursor.get(&state.selected).cloned()
        };
        self.live.borrow_mut().message = None;
        panes.set_open(entry.is_some());
        match &entry {
            Some(entry) => self.detail_panel(entry),
            None => ui::clear(&self.parts.detail_inner),
        }
        self.parts.detail_scroller.vadjustment().set_value(0.0);
        self.place_controls();
    }

    /// The window's own buttons: in the open entry's head, else at the end of the view's title, else over
    /// the top of the page
    fn place_controls(&self) {
        let live = self.live.borrow();
        let detail_open = live.panes.as_ref().is_some_and(|panes| panes.has_css_class("open"));
        let logs = self.state.borrow().page == PageKind::Logs;
        // None in the title's row while the desktop puts them all at the start of the title bar, where the row's
        // spacing would still keep room for them
        if let Some(controls) = live.title_row.as_ref().and_then(|row| row.last_child()).and_downcast::<gtk::WindowControls>() {
            controls.set_visible(!detail_open && !controls.is_empty());
        }
        self.parts.floating.set_visible(!logs);
    }

    // --- The log view ------------------------------------------------------------------------------

    fn title(&self) -> String {
        let state = self.state.borrow();
        if let Some(source) = &state.source {
            return source.label.clone();
        }
        if state.range.starts_with("around:") {
            return t!("Around one entry");
        }
        category(&state.category).0
    }

    fn lead(&self) -> String {
        let state = self.state.borrow();
        if state.range.starts_with("around:") {
            return t!("Everything that was logged from a minute before it to a minute after.");
        }
        if let Some(source) = &state.source {
            return match source.kind {
                Kind::App => t!("What this app wrote, each time it ran."),
                Kind::Unit if source.scope == Scope::User => t!("One of the services of your session."),
                Kind::Unit => t!("One of the system’s services."),
                Kind::Identifier => t!("Everything logged under this name."),
            };
        }
        category(&state.category).1
    }

    fn range_select(&self) -> gtk::Widget {
        let (range, boots) = {
            let state = self.state.borrow();
            (state.range.clone(), state.overview.as_ref().map(|o| o.boots.clone()).unwrap_or_default())
        };
        let mut choices: Vec<Choice> = ranges().into_iter().map(|(value, label)| Choice { value: value.into(), label, group: None }).collect();
        let earlier: Vec<&Boot> = boots.iter().filter(|boot| boot.index < -1).collect();
        for boot in &earlier {
            choices.push(Choice { value: format!("b:{}", boot.id), label: boot_label(boot), group: Some(t!("Earlier boots")) });
        }
        if !choices.iter().any(|choice| choice.value == range) {
            let label = if range.starts_with("around:") { t!("Two minutes") } else { t!("That boot") };
            choices.push(Choice { value: range.clone(), label, group: None });
        }
        let weak = self.me.clone();
        select(&t!("Time"), choices, &range, move |value| {
            if let Some(this) = weak.upgrade() {
                this.set_range(&value);
            }
        })
        .upcast()
    }

    fn level_select(&self) -> gtk::Widget {
        let (important, level) = {
            let state = self.state.borrow();
            (state.category == "important", state.level)
        };
        let value = if important { level.min(3) } else { level };
        let choices = levels()
            .into_iter()
            .filter(|(level, _)| !important || *level <= 3)
            .map(|(level, label)| Choice { value: level.to_string(), label, group: None })
            .collect();
        let weak = self.me.clone();
        select(&t!("Level"), choices, &value.to_string(), move |chosen| {
            if let Some(this) = weak.upgrade() {
                this.set_level(chosen.parse().unwrap_or(7));
            }
        })
        .upcast()
    }

    fn search_box(&self) -> gtk::Box {
        let (search, regex) = {
            let state = self.state.borrow();
            (state.search.clone(), state.regex)
        };
        let field = ui::hbox(9, &["search"]);
        let glass = icon("search", 17);
        field.append(&glass);
        let input = gtk::Text::builder().accessible_role(gtk::AccessibleRole::TextBox).placeholder_text(t!("Search messages")).hexpand(true).valign(gtk::Align::Center).build();
        input.set_text(&search);
        input.update_property(&[gtk::accessible::Property::Label(&t!("Search messages"))]);
        field.append(&input);
        let toggle_words = ui::line(".*", &["mono"]);
        toggle_words.set_direction(gtk::TextDirection::Ltr);
        let toggle = gtk::Button::builder().child(&toggle_words).css_classes(["plain", "toggle"]).valign(gtk::Align::Center).build();
        if regex {
            toggle.add_css_class("pressed");
        }
        toggle.set_tooltip_text(Some(&t!("Search with a regular expression")));
        toggle.update_state(&[gtk::accessible::State::Pressed(if regex { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
        field.append(&toggle);

        let apply = {
            let weak = self.me.clone();
            move |input: &gtk::Text| {
                let Some(this) = weak.upgrade() else { return };
                if let Some(timer) = this.search_timer.take() {
                    timer.remove();
                }
                let text = input.text().to_string();
                if text != this.state.borrow().search {
                    this.state.borrow_mut().search = text;
                    this.load();
                }
            }
        };
        {
            let (weak, apply) = (self.me.clone(), apply.clone());
            input.connect_changed(move |input| {
                let Some(this) = weak.upgrade() else { return };
                if let Some(timer) = this.search_timer.take() {
                    timer.remove();
                }
                let (input, apply) = (input.clone(), apply.clone());
                let timer = glib::timeout_add_local_once(Duration::from_millis(400), move || apply(&input));
                this.search_timer.replace(Some(timer));
            });
        }
        {
            let apply = apply.clone();
            input.connect_activate(move |input| apply(input));
        }
        let keys = gtk::EventControllerKey::new();
        {
            let (field_input, apply) = (input.clone(), apply.clone());
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape && !field_input.text().is_empty() {
                    field_input.set_text("");
                    apply(&field_input);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }
        input.add_controller(keys);
        {
            let (weak, input) = (self.me.clone(), input.clone());
            toggle.connect_clicked(move |toggle| {
                let Some(this) = weak.upgrade() else { return };
                let regex = {
                    let mut state = this.state.borrow_mut();
                    state.regex = !state.regex;
                    state.regex
                };
                if regex {
                    toggle.add_css_class("pressed");
                } else {
                    toggle.remove_css_class("pressed");
                }
                toggle.update_state(&[gtk::accessible::State::Pressed(if regex { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
                if !this.state.borrow().search.is_empty() {
                    this.load();
                }
                input.grab_focus_without_selecting();
            });
        }
        self.live.borrow_mut().search = Some(input);
        field
    }

    fn chips(&self) -> Option<gtk::Widget> {
        let (source, range) = {
            let state = self.state.borrow();
            (state.source.clone(), state.range.clone())
        };
        let mut list: Vec<gtk::Widget> = Vec::new();
        let chip = |glyph: &str, text: &str, close: &str, f: Box<dyn Fn()>| {
            let chip = ui::hbox(7, &["chip"]);
            chip.append(&icon(glyph, 14));
            let words = ui::line(&list::in_page_direction(text), &[]);
            words.set_ellipsize(pango::EllipsizeMode::End);
            chip.append(&words);
            let button = gtk::Button::builder().child(&icon("close", 14)).css_classes(["plain"]).valign(gtk::Align::Center).build();
            button.update_property(&[gtk::accessible::Property::Label(close)]);
            button.connect_clicked(move |_| f());
            chip.append(&button);
            chip.upcast::<gtk::Widget>()
        };
        if let Some(source) = &source {
            list.push(chip("filter", &t!("Only {source}", source = &source.label), &t!("Show everything again"), Box::new(self.on(|this| this.set_source(None)))));
        }
        if let Some(second) = range.strip_prefix("around:").and_then(|s| s.parse::<i64>().ok()) {
            let text = t!(
                "{start} to {end}, {day}",
                start = list::clock((second - 60) * 1000),
                end = list::clock((second + 60) * 1000),
                day = short_day(second * 1000)
            );
            list.push(chip("target", &text, &t!("Back to this boot"), Box::new(self.on(|this| this.set_range("boot")))));
        }
        if list.is_empty() {
            return None;
        }
        let chips = ui::wrap(8, 8, &["chips"]);
        for chip in list {
            chips.append(&chip);
        }
        Some(chips.upcast())
    }

    /// Whether the system's side of the journal is shown, which access() says over the list is for administrators
    /// when this account cannot read it (the overview that says so may come after the list)
    fn system_view(&self) -> bool {
        SYSTEM_VIEWS.contains(&self.state.borrow().category.as_str())
    }

    /// Who is not shown what: the system's side of the journal is for administrators
    fn access(&self) -> Option<gtk::Widget> {
        let state = self.state.borrow();
        let info = state.overview.as_ref()?;
        if info.readable || state.category == "session" {
            return None;
        }
        if SYSTEM_VIEWS.contains(&state.category.as_str()) {
            let command = if state.command.is_empty() { "sudo journalctl".to_string() } else { format!("sudo {}", state.command) };
            return Some(
                callout(
                    "check",
                    &t!("Only administrators can read these"),
                    &t!("The system’s logs are open to the administrators of this machine, and {user} is not one.", user = &info.user),
                    vec![command_block(&t!("An administrator can read them in a terminal:"), &command).upcast()],
                )
                .upcast(),
            );
        }
        Some(
            callout(
                "info",
                &t!("Only your own logs"),
                &t!("The system’s logs are for the administrators of this machine: you see what your session and your apps wrote."),
                Vec::new(),
            )
            .upcast(),
        )
    }

    pub(crate) fn render_actions(&self) {
        let Some(actions) = self.live.borrow().actions.clone() else { return };
        ui::clear(&actions);
        let live = self.state.borrow().live;
        let followable = self.can_follow();
        let words = if live { t!("Live") } else { t!("Follow") };
        let follow = self.button(&words, Some(if live { "pause" } else { "live" }), true, live, |this| {
            let live = this.state.borrow().live;
            this.set_live(!live);
        });
        follow.set_sensitive(followable);
        follow.update_state(&[gtk::accessible::State::Pressed(if live { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
        follow.set_tooltip_text(Some(&if followable { t!("Show new entries as they are written") } else { t!("Only a time range that ends now can be followed") }));
        actions.append(&follow);
        let export = self.button(&t!("Export"), Some("download"), true, false, |this| this.export());
        export.set_tooltip_text(Some(&t!("Save what is shown to a file")));
        actions.append(&export);
        let terminal = self.button("", Some("terminal"), true, false, |this| {
            let this = this.clone();
            glib::spawn_future_local(async move {
                let request = this.request();
                let live = this.state.borrow().live;
                let _ = this.journal.terminal(&request, live).await;
            });
        });
        terminal.set_tooltip_text(Some(&t!("Open the same in journalctl, in a terminal")));
        actions.append(&terminal);
        let refresh = self.button("", Some("refresh"), true, false, |this| {
            this.load();
            let this = this.clone();
            glib::spawn_future_local(async move { this.refresh_overview().await });
        });
        refresh.set_tooltip_text(Some(&t!("Read again ({key})", key = "Ctrl+R")));
        actions.append(&refresh);
    }

    pub(crate) fn render_notice(&self) {
        let Some(slot) = self.live.borrow().notice.clone() else { return };
        ui::clear(&slot);
        let Some(notice) = self.state.borrow().notice.clone() else { return };
        let banner = ui::hbox(12, &["banner", notice.tone]);
        banner.append(&icon(if notice.tone == "check" { "alert" } else { "info" }, 18));
        let words = wrapped(&notice.text, &["grow"]);
        words.set_hexpand(true);
        words.set_wrap_mode(pango::WrapMode::WordChar);
        banner.append(&words);
        banner.append(&self.icon_button("close", 16, &t!("Dismiss"), |this| this.set_notice(None)));
        slot.append(&banner);
    }

    /// The range select again, for the boots the overview found
    pub(crate) fn render_range(&self) {
        let (toolbar, old) = {
            let live = self.live.borrow();
            match (&live.toolbar, &live.range) {
                (Some(toolbar), Some(old)) => (toolbar.clone(), old.clone()),
                _ => return,
            }
        };
        let new = self.range_select();
        toolbar.replace(&old, &new);
        self.live.borrow_mut().range = Some(new);
    }

    pub(crate) fn render_access(&self) {
        let (head, notice, old) = {
            let live = self.live.borrow();
            match (&live.head, &live.notice) {
                (Some(head), Some(notice)) => (head.clone(), notice.clone(), live.access.clone()),
                _ => return,
            }
        };
        if let Some(old) = old {
            head.remove(&old);
        }
        let new = self.access();
        if let Some(new) = &new {
            head.insert_child_after(new, notice.prev_sibling().as_ref());
        }
        self.live.borrow_mut().access = new;
    }

    fn logs_page(&self) -> Panes {
        let main = ui::vbox(0, &["logs-main"]);
        main.set_hexpand(true);
        let head = self.logs_head();
        main.append(&head);
        main.append(&self.parts.list_wrap);

        let panes = Panes::new(&main, &self.parts.detail);
        let weak = self.me.clone();
        panes.on_height(move |height| {
            if let Some(this) = weak.upgrade() {
                this.vh.set(height);
                if let Some(loading) = this.live.borrow().loading.clone() {
                    loading.set_size_request(-1, (0.4 * height as f64).round() as i32);
                }
                if let Some((scroller, _, _)) = this.live.borrow().message.clone() {
                    scroller.set_max_content_height(this.message_height());
                }
            }
        });
        self.live.borrow_mut().panes = Some(panes.clone());
        panes
    }

    /// What is above the list: the title, the search and the filters
    fn logs_head(&self) -> gtk::Box {
        let head = ui::vbox(14, &["logs-head"]);
        let title_row = ui::hbox(20, &["title-row"]);
        let grow = ui::vbox(0, &["grow"]);
        grow.set_hexpand(true);
        let title = ui::line(&self.title(), &["h1"]);
        list::ellipsize_as_the_page(&title, &title.label());
        grow.append(&title);
        grow.append(&ui::limit(&wrapped(&self.lead(), &["lead"]), 640));
        title_row.append(&grow);
        let actions = ui::hbox(8, &["head-actions"]);
        actions.set_valign(gtk::Align::Start);
        title_row.append(&actions);
        let controls = frame::controls();
        controls.set_valign(gtk::Align::Start);
        controls.add_css_class("title-controls");
        // The desktop's layout changed: placed again, once the change has been made
        let weak = self.me.clone();
        controls.connect_empty_notify(move |_| {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.place_controls();
                }
            });
        });
        title_row.append(&controls);
        head.append(&title_row);

        let toolbar = Toolbar::new(10);
        toolbar.append(&self.search_box(), Flex { basis: Some(260.0), grow: true, min: 200.0, ..Default::default() });
        let range = self.range_select();
        toolbar.append(&range, Flex::default());
        toolbar.append(&self.level_select(), Flex::default());
        let count = ui::line("", &["count"]);
        toolbar.append(&count, Flex { auto_start: true, full: true, ..Default::default() });
        head.append(&toolbar);
        if let Some(chips) = self.chips() {
            head.append(&chips);
        }
        let access = self.access();
        if let Some(access) = &access {
            head.append(access);
        }
        let notice = ui::vbox(0, &["notice"]);
        head.append(&notice);
        {
            let mut live = self.live.borrow_mut();
            live.head = Some(head.clone());
            live.actions = Some(actions);
            live.count = Some(count);
            live.notice = Some(notice);
            live.access = access;
            live.toolbar = Some(toolbar);
            live.range = Some(range);
            live.title_row = Some(title_row);
        }
        head
    }

    // --- Apps and services -------------------------------------------------------------------------

    fn source_row(&self, kind: Kind, scope: Scope, unit: &UnitItem, first: bool) -> gtk::Button {
        let row = ui::hbox(14, &[]);
        let grow = ui::vbox(1, &["grow"]);
        grow.set_hexpand(true);
        let name = ui::line(&unit.label, &["strong"]);
        list::ellipsize_as_the_page(&name, &unit.label);
        grow.append(&name);
        if let Some(instances) = unit.instances {
            grow.append(&ui::line(&t!("{count, plural, one {# instance, together} other {# instances, together}}", count = instances), &["small-text"]));
        }
        row.append(&grow);
        let chevron = icon("chevron", 16).mirror_in_rtl();
        chevron.add_css_class("chevron");
        row.append(&chevron);
        let button = gtk::Button::builder().child(&row).css_classes(["plain", "list-row"]).build();
        if first {
            button.add_css_class("first");
        }
        let source = Source { kind, id: unit.name.clone(), scope, label: unit.label.clone() };
        let act = self.on(move |this| this.set_source(Some(source.clone())));
        button.connect_clicked(move |_| act());
        button
    }

    fn card_list(&self, rows: Vec<gtk::Widget>) -> gtk::Widget {
        let list = ui::vbox(0, &[]);
        for row in rows {
            list.append(&row);
        }
        // max-height: 520px, scrolled within
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height((518.0 * amethystora_ui::icon::zoom()) as i32)
            .child(&list)
            .css_classes(["card", "list"])
            .build();
        scroller.set_overflow(gtk::Overflow::Hidden);
        scroller.upcast()
    }

    fn none_match() -> gtk::Widget {
        wrapped(&t!("None match."), &["list-empty"]).upcast()
    }

    fn fill_sources(&self, results: &gtk::Box) {
        ui::clear(results);
        let state = self.state.borrow();
        let Some(found) = &state.sources else { return };
        let filter = state.source_filter.trim().to_lowercase();
        let matches = |texts: &[&str]| filter.is_empty() || texts.iter().any(|text| text.to_lowercase().contains(&filter));
        let apps: Vec<_> = found.apps.iter().filter(|item| matches(&[&item.name, &item.id])).cloned().collect();
        let user: Vec<_> = found.user.iter().filter(|unit| matches(&[&unit.label])).cloned().collect();
        let system: Vec<_> = found.system.iter().filter(|unit| matches(&[&unit.label])).cloned().collect();
        let readable = state.overview.as_ref().map(|o| o.readable);
        drop(state);

        let apps_body: gtk::Widget = if apps.is_empty() {
            let card = ui::vbox(0, &["card"]);
            card.append(&Self::none_match());
            card.upcast()
        } else {
            let tiles = Tiles::new(220, 10);
            for item in &apps {
                let content = ui::hbox(12, &[]);
                let letter = item.name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect::<String>()).unwrap_or_else(|| "?".into());
                let avatar = ui::line(&letter, &["avatar", "tone", "accent"]);
                avatar.set_xalign(0.5);
                avatar.set_valign(gtk::Align::Center);
                content.append(&avatar);
                let text = ui::vbox(0, &["grow", "app-text"]);
                text.set_hexpand(true);
                text.set_valign(gtk::Align::Center);
                let name = ui::line(&item.name, &["strong"]);
                list::ellipsize_as_the_page(&name, &item.name);
                let id = ui::line(&item.id, &["small-text"]);
                list::ellipsize_as_the_page(&id, &item.id);
                text.append(&name);
                text.append(&id);
                content.append(&text);
                let button = gtk::Button::builder().child(&content).css_classes(["plain", "app", "card"]).build();
                let source = Source { kind: Kind::App, id: item.id.clone(), scope: Scope::User, label: item.name.clone() };
                let act = self.on(move |this| this.set_source(Some(source.clone())));
                button.connect_clicked(move |_| act());
                tiles.append(&button);
            }
            tiles.upcast()
        };
        results.append(&section(&t!("Apps"), apps.len(), &apps_body));
        let user_rows: Vec<gtk::Widget> = if user.is_empty() {
            vec![Self::none_match()]
        } else {
            user.iter().enumerate().map(|(at, unit)| self.source_row(Kind::Unit, Scope::User, unit, at == 0).upcast()).collect()
        };
        results.append(&section(&t!("Your services"), user.len(), &self.card_list(user_rows)));
        let system_body = if !system.is_empty() || readable != Some(false) {
            let rows: Vec<gtk::Widget> = if system.is_empty() {
                vec![Self::none_match()]
            } else {
                system.iter().enumerate().map(|(at, unit)| self.source_row(Kind::Unit, Scope::System, unit, at == 0).upcast()).collect()
            };
            self.card_list(rows)
        } else {
            callout(
                "check",
                &t!("Only administrators can read these"),
                &t!("The system’s services write to the system’s logs, which are for the administrators of this machine."),
                Vec::new(),
            )
            .upcast()
        };
        results.append(&section(&t!("System services"), system.len(), &system_body));
    }

    fn sources_page(&self) -> gtk::Widget {
        let page = ui::vbox(28, &["page"]);
        let head = ui::vbox(6, &["page-head"]);
        head.append(&wrapped(&t!("Apps and services"), &["h1"]));
        head.append(&ui::limit(&wrapped(&t!("Everything that has written to the journal. Pick one to read only what it said."), &["lead"]), 640));
        page.append(&head);
        let loaded = self.state.borrow().sources.is_some();
        if !loaded {
            let loading = ui::vbox(12, &["loading"]);
            let gem = reading();
            gem.set_valign(gtk::Align::Center);
            gem.set_vexpand(true);
            loading.append(&gem);
            loading.set_size_request(-1, (0.4 * self.vh.get() as f64).round() as i32);
            page.append(&loading);
        } else {
            let field = ui::hbox(9, &["search", "wide"]);
            field.append(&icon("search", 17));
            let filter = gtk::Text::builder().accessible_role(gtk::AccessibleRole::TextBox).placeholder_text(t!("Find an app or service")).hexpand(true).valign(gtk::Align::Center).build();
            filter.set_text(&self.state.borrow().source_filter);
            filter.update_property(&[gtk::accessible::Property::Label(&t!("Find an app or service"))]);
            field.append(&filter);
            page.append(&ui::limit(&field, 520));
            let results = ui::vbox(28, &["stack"]);
            self.fill_sources(&results);
            let (weak, list) = (self.me.clone(), results.clone());
            filter.connect_changed(move |filter| {
                if let Some(this) = weak.upgrade() {
                    this.state.borrow_mut().source_filter = filter.text().to_string();
                    let (again, list) = (this.clone(), list.clone());
                    this.filter_pause.after_typing(move || again.fill_sources(&list));
                }
            });
            page.append(&results);
            self.live.borrow_mut().filter = Some(filter);
        }
        let column = ui::limit_center(&page, 1000);
        gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&natural(&column)).build().upcast()
    }

    // --- The page ----------------------------------------------------------------------------------

    pub(crate) fn render_hint(&self) {
        let state = self.state.borrow();
        let text = if state.page == PageKind::Sources {
            "journalctl --field=_SYSTEMD_UNIT".to_string()
        } else if state.command.is_empty() {
            "journalctl".to_string()
        } else {
            state.command.clone()
        };
        self.parts.hint.set_label(&text);
    }

    pub(crate) fn render_nav(&self) {
        let state = self.state.borrow();
        for (name, button, badge) in &self.parts.nav {
            button.set_current(state.page == PageKind::Logs && state.source.is_none() && *name == state.category);
            if let Some(badge) = badge {
                let count = state.overview.as_ref().map(|o| if *name == "important" { o.important } else { o.crashes }).unwrap_or(0);
                badge.set_visible(count > 0);
                badge.set_label(&if count > 0 { capped(count) } else { String::new() });
                badge.set_tooltip_text((count > 0).then(|| t!("This boot")).as_deref());
            }
        }
        self.parts.sources.set_current(state.page == PageKind::Sources || (state.page == PageKind::Logs && state.source.is_some()));
        let size = state.overview.as_ref().map(|o| o.usage.clone()).unwrap_or_default();
        self.parts.usage.set_visible(!size.is_empty());
        if !size.is_empty() {
            let readable = state.overview.as_ref().is_some_and(|o| o.readable);
            self.parts.usage.set_label(&if readable {
                t!("The journal takes {size} on disk.", size = &size)
            } else {
                t!("Your logs take {size} on disk.", size = &size)
            });
        }
    }

    pub(crate) fn render(&self) {
        // On the logs already, only what is above the list changes: a list view taken out of the window and put back makes
        // all its rows again, which stops the window at every change of view
        if self.state.borrow().page == PageKind::Logs && self.render_head_only() {
            return;
        }
        // The parts that stay are taken out of the page before it is drawn again
        if let Some(parent) = self.parts.list_wrap.parent().and_downcast::<gtk::Box>() {
            parent.remove(&self.parts.list_wrap);
        }
        if self.parts.detail.parent().is_some() {
            self.parts.detail.unparent();
        }
        *self.live.borrow_mut() = Live::default();
        ui::clear(&self.parts.view);
        self.render_nav();
        self.render_hint();
        let page = self.state.borrow().page;
        if page == PageKind::Sources {
            self.parts.model.clear();
            self.parts.view.append(&self.sources_page());
            self.place_controls();
            return;
        }
        let panes = self.logs_page();
        panes.set_vexpand(true);
        self.parts.view.append(&panes);
        self.render_actions();
        self.render_notice();
        self.render_list();
        self.render_fresh();
        self.render_detail();
    }

    fn render_head_only(&self) -> bool {
        let old = {
            let live = self.live.borrow();
            live.panes.as_ref().and(live.head.clone())
        };
        let Some(old) = old else { return false };
        let Some(main) = old.parent().and_then(|parent| parent.downcast::<gtk::Box>().ok()) else { return false };
        main.remove(&old);
        main.prepend(&self.logs_head());
        self.render_nav();
        self.render_hint();
        self.render_actions();
        self.render_notice();
        // Only a list that is not what the state says: load() draws its own, and the same rows drawn again are work for nothing
        let (loading, request) = {
            let state = self.state.borrow();
            (state.loading, state.request)
        };
        if loading || self.list_of.get() != request {
            self.render_list();
        } else {
            self.render_count();
        }
        self.render_fresh();
        self.render_detail();
        true
    }

    pub(crate) fn focus_search(&self) {
        let live = self.live.borrow();
        if let Some(search) = live.search.as_ref().or(live.filter.as_ref()) {
            search.grab_focus();
        }
    }

    /// The window crossed one of the widths the page changes at
    pub(crate) fn relayout(&self) {
        if let Some(panes) = self.live.borrow().panes.clone() {
            panes.queue_resize();
        }
        if let Some(toolbar) = self.live.borrow().toolbar.clone() {
            toolbar.queue_resize();
        }
    }
}

/// Text that wraps, as ui::label, breaking a word that does not fit without a hyphen
fn wrapped(text: &str, classes: &[&str]) -> gtk::Label {
    let label = ui::label(text, classes);
    list::unhyphenated(&label);
    label
}

/// A section's name wraps in a narrow sidebar, as the page's did, where the shared sidebar cuts it short
fn wrap_name(button: &NavButton) {
    button.label.set_ellipsize(pango::EllipsizeMode::None);
    button.label.set_wrap(true);
    button.label.set_wrap_mode(pango::WrapMode::WordChar);
    button.label.set_natural_wrap_mode(gtk::NaturalWrapMode::None);
    list::unhyphenated(&button.label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boot_over_several_days_says_the_day_it_ended() {
        let boot = |first: i64, last: i64| Boot { index: -2, id: String::new(), first: Some(first), last: Some(last) };
        let start = 1_793_000_000_000;
        let (day, later) = (short_day(start), short_day(start + 3 * 86_400_000));
        assert_ne!(day, later);
        let long = boot_label(&boot(start, start + 3 * 86_400_000));
        assert!(long.contains(&day) && long.contains(&later), "{long}");
        assert!(!boot_label(&boot(start, start + 1000)).contains(&later));
    }
}
