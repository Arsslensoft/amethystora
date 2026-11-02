//! The page of Amethystora Security, as security.js drew it. The report is
//! /usr/libexec/amethystora-security-status --json, the one `ame security status` prints, the scans are the
//! ones this window runs, and network protection is what the security watcher kept of Suricata's alerts.
//! Everything shown is built as widgets with text in them and never as markup: a file name is whatever
//! somebody called a file, and a rule's name and a host name are whatever the network sent.
//!
//! The report's own words, each check's title, text and details, and what amethystora-app-permissions says
//! each app can reach, are in the session's language already, as those scripts write them: they are shown as
//! they come, and never looked up in the catalog.

use std::rc::{Rc, Weak};

use amethystora_ui::frame::{self, NavButton};
use amethystora_ui::i18n::{self, DateStyle, ListKind};
use amethystora_ui::icon::Icon;
use amethystora_ui::ui::{self, Bar};
use amethystora_ui::{adw, gdk, glib, gtk, process, t};
use gtk::prelude::*;

use crate::art::{self, ScanGem, Shield};
use crate::backend::{self, Check, Container, Entry, Event, Found, Hardening, Kind, Label, Machine, Network, NetworkEvent, Setting, Signatures, Switches};
use crate::icons;
use crate::{Begun, PageState, Security, Shown};

// --- The window around the page -------------------------------------------------------------------------------

/// The count or the spinner after a section's name, as `.badge`, hidden while it has nothing to say
pub(crate) struct Badge {
    root: gtk::Box,
    label: gtk::Label,
    spinner: Icon,
}

impl Badge {
    fn new() -> Badge {
        let root = ui::hbox(0, &["badge"]);
        root.set_valign(gtk::Align::Center);
        root.set_hexpand(false);
        let label = ui::line("", &[]);
        label.set_xalign(0.5);
        label.set_hexpand(true);
        let spinner = icons::icon("spinner", 12);
        spinner.set_hexpand(true);
        root.append(&label);
        root.append(&spinner);
        root.set_visible(false);
        Badge { root, label, spinner }
    }

    /// A number, the spinner, or nothing
    fn set(&self, content: Option<&str>, busy: bool, tone: &str) {
        let mut classes = vec!["badge"];
        if !tone.is_empty() {
            classes.push(tone);
        }
        self.root.set_css_classes(&classes);
        self.label.set_label(content.unwrap_or(""));
        self.label.set_visible(!busy);
        self.spinner.set_visible(busy);
        self.spinner.set_spinning(busy);
        self.root.set_visible(busy || content.is_some_and(|c| !c.is_empty()));
    }
}

/// The sidebar, the page beside it, and the zone a file is dropped on
pub(crate) struct Frame {
    pub root: gtk::Overlay,
    pub view: gtk::Box,
    pub scroller: gtk::ScrolledWindow,
    pub nav: Vec<(&'static str, NavButton, Badge)>,
    pub terminal: gtk::Label,
    pub drop: gtk::Label,
    /// The page the view shows, to keep its scroll when it is drawn again
    pub shown: std::cell::RefCell<String>,
}

impl Frame {
    pub fn new(me: &Weak<Security>) -> Frame {
        // The wordmark of the boot splash and the login screen, the glowing stone as its "a"
        let brand = amethystora_ui::brand::brand(&t!("Security"), 21.0);
        brand.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Security"))]);
        let sidebar = frame::Sidebar::new(&brand, 22);
        let nav = frame::nav(4, None, &t!("Sections"));
        let mut buttons = Vec::new();
        for (page, icon_name, text) in [
            ("overview", "shield", t!("Overview")),
            ("scan", "scan", t!("Virus scan")),
            ("network", "globe", t!("Network")),
            ("apps", "apps", t!("Apps")),
            ("containers", "box", t!("Containers")),
            ("events", "history", t!("Events")),
            ("settings", "sliders", t!("Settings")),
        ] {
            let button = NavButton::new(icons::icon(icon_name, 20), &text, 12);
            let badge = Badge::new();
            button.append(&badge.root);
            let weak = me.clone();
            button.button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.state.borrow_mut().notice.clear();
                    this.show(page, None);
                }
            });
            nav.append(&button.button);
            buttons.push((page, button, badge));
        }
        sidebar.body.append(&nav);
        let manual = ui::button(&t!("Security in the manual"), Some(icons::icon("book", 18)), &["link", "quiet"], || process::manual("security"));
        manual.set_halign(gtk::Align::Start);
        sidebar.foot.append(&manual);
        let terminal = frame::terminal(&t!("Also in a terminal:"), "ame security status");
        let code = terminal.last_child().and_downcast::<gtk::Label>().expect("the command under the words");
        sidebar.foot.append(&terminal);

        let view = ui::vbox(0, &["view"]);
        view.set_vexpand(true);
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&view).build();
        scroller.add_css_class("main");
        let layout = frame::with_sidebar(&sidebar.root, &scroller, 264, true);

        let drop = ui::line(&t!("Drop to scan"), &["drop"]);
        drop.set_xalign(0.5);
        drop.set_can_target(false);
        drop.set_visible(false);
        drop.update_state(&[gtk::accessible::State::Hidden(true)]);
        let root = gtk::Overlay::new();
        root.set_child(Some(&layout));
        root.add_overlay(&drop);
        Frame { root, view, scroller, nav: buttons, terminal: code, drop, shown: Default::default() }
    }
}

/// The parts of the scanning page that change while it is on screen
#[derive(Default)]
pub(crate) struct Live {
    /// Since when the scan has run, in seconds
    pub since: Option<f64>,
    pub elapsed: Option<gtk::Label>,
    pub files: Option<gtk::Label>,
    pub found: Option<gtk::Label>,
    pub path: Option<gtk::Label>,
}

// --- Building blocks ------------------------------------------------------------------------------------------

/// An icon whose strokes are as wide as the page drew that one
fn icon_with(name: &str, size: i32, stroke: f32) -> Icon {
    Icon::new(icons::data(name), size, stroke)
}

/// An arrow or chevron, which points the other way right to left
fn icon_for(name: &str, size: i32) -> Icon {
    let icon = icons::icon(name, size);
    if matches!(name, "arrow-right" | "arrow-left" | "chevron") { icon.mirror_in_rtl() } else { icon }
}

/// What a button that starts a scan asks the window for
type Request = fn(Rc<Security>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Begun>>>;

#[derive(Default)]
struct Opts {
    icon: Option<&'static str>,
    small: bool,
    primary: bool,
    disabled: bool,
    /// The icon after the words
    after: bool,
    title: Option<String>,
}

/// A button's words, with an icon before them or after
fn content(label: &str, icon: Option<Icon>, after: bool) -> gtk::Box {
    let content = ui::hbox(0, &["content"]);
    let words = ui::line(label, &[]);
    match (icon, after) {
        (Some(icon), false) => {
            content.append(&icon);
            content.append(&words);
        }
        (Some(icon), true) => {
            content.append(&words);
            content.append(&icon);
        }
        (None, _) => content.append(&words),
    }
    content
}

/// A name kept apart from the words around it, whichever way its letters run, as <bdi> did
fn isolate(text: &str) -> String {
    format!("\u{2068}{text}\u{2069}")
}

/// ...and one that reads left to right in any language, as <bdi dir="ltr">
fn ltr(text: &str) -> String {
    format!("\u{2066}{text}\u{2069}")
}

/// A mono pill holds a name such as a signature's, which reads left to right in any language
fn pill(text: &str, tone: &str, small: bool, mono: bool) -> gtk::Label {
    let mut classes = vec!["pill"];
    if !tone.is_empty() {
        classes.push(tone);
    }
    if small {
        classes.push("small-pill");
    }
    if mono {
        classes.push("mono");
    }
    let label = ui::line(text, &classes);
    label.set_valign(gtk::Align::Center);
    label.set_halign(gtk::Align::Start);
    if mono {
        label.set_direction(gtk::TextDirection::Ltr);
    }
    label
}

fn mark(tone: &str) -> &'static str {
    match tone {
        "ok" => "mark-ok",
        "check" | "off" => "mark-alert",
        _ => "mark-info",
    }
}

/// A square or round tint with an icon in its middle
fn tinted(classes: &[&str], icon: Icon) -> gtk::Box {
    let square = ui::hbox(0, classes);
    // Its own size, whatever the icon in it asks for
    square.set_hexpand(false);
    icon.set_hexpand(true);
    square.append(&icon);
    square
}

/// The small round mark in front of a row
fn dot(tone: &str) -> gtk::Box {
    let dot = tinted(&["dot", tone], icon_with(mark(tone), 13, 3.0));
    dot.set_valign(gtk::Align::Start);
    dot
}

/// ...and the larger one in front of a scan
fn status(tone: &str) -> gtk::Box {
    let status = tinted(&["status", "tone", tone], icon_with(mark(tone), 15, 2.6));
    status.set_valign(gtk::Align::Center);
    status
}

fn section(title: &str, body: &impl IsA<gtk::Widget>, note: Option<&str>) -> gtk::Box {
    let section = ui::vbox(12, &["section"]);
    let head = ui::hbox(16, &["section-head"]);
    // In capitals the language's own way; the stylesheet's text-transform is not measured by GTK
    let h2 = wrapping(&title.to_uppercase(), &["h2"]);
    h2.set_hexpand(true);
    h2.set_valign(gtk::Align::BaselineFill);
    head.append(&h2);
    if let Some(note) = note.filter(|note| !note.is_empty()) {
        let note = wrapping(note, &["section-note"]);
        note.set_valign(gtk::Align::BaselineFill);
        head.append(&note);
    }
    section.append(&head);
    section.append(body);
    section
}

fn callout(tone: &str, title: &str, text: &str, extra: Option<gtk::Widget>) -> gtk::Box {
    let callout = ui::hbox(14, &["callout", tone]);
    let icon = icons::icon(if tone == "check" { "alert" } else { "info" }, 20);
    icon.set_valign(gtk::Align::Start);
    callout.append(&icon);
    let body = ui::vbox(6, &["grow"]);
    body.set_hexpand(true);
    body.append(&wrapping(title, &["strong"]));
    if !text.is_empty() {
        body.append(&ui::limit(&wrapping(text, &["p"]), 640));
    }
    if let Some(extra) = extra {
        body.append(&extra);
    }
    callout.append(&body);
    callout
}

fn copy_button(text: &str, label: &str, description: &str) -> gtk::Button {
    let button = ui::copy_button(text, icons::icon("copy", 14), label, &t!("Copied"), description, &["btn", "small"]);
    button.set_valign(gtk::Align::Center);
    button
}

fn command_block(lead: &str, command: &str) -> gtk::Box {
    let wrap = ui::vbox(8, &["command-wrap"]);
    if !lead.is_empty() {
        wrap.append(&ui::limit(&wrapping(lead, &["p"]), 640));
    }
    let line = ui::hbox(12, &["command"]);
    let code = wrapping(command, &["code"]);
    code.set_hexpand(true);
    code.set_selectable(true);
    code.set_direction(gtk::TextDirection::Ltr);
    line.append(&code);
    line.append(&copy_button(command, &t!("Copy"), &t!("Copy the command")));
    wrap.append(&ui::limit(&line, 560));
    wrap
}

fn actions(children: Vec<gtk::Widget>) -> adw::WrapBox {
    let actions = ui::wrap(16, 10, &["actions"]);
    for child in children {
        child.set_valign(gtk::Align::Center);
        actions.append(&child);
    }
    actions
}

/// Facts about one thing on one line, with dots between them
fn dotted(pieces: &[String]) -> String {
    pieces.iter().filter(|piece| !piece.is_empty()).cloned().collect::<Vec<_>>().join(" · ")
}

fn page_head(title: &str, lead: &str) -> gtk::Box {
    let head = ui::vbox(6, &["page-head"]);
    head.append(&wrapping(title, &["h1"]));
    head.append(&ui::limit(&wrapping(lead, &["lead"]), 640));
    head
}

fn notice(text: &str) -> gtk::Label {
    let notice = wrapping(text, &["notice"]);
    notice.update_property(&[gtk::accessible::Property::Description(text)]);
    notice
}

/// A list's card with one line of text in it, for while it is read or when it has nothing
fn empty(text: &str) -> gtk::Box {
    let card = ui::vbox(0, &["list", "card"]);
    card.set_overflow(gtk::Overflow::Hidden);
    card.append(&wrapping(text, &["list-empty"]));
    card
}

/// A long list's rows are made at the first frames they can be shown, a few at a time: made and laid out at once, they stop
/// the window
const ROWS_AT_ONCE: usize = 4;
const ROWS_PER_FRAME: usize = 2;

fn list_card_filling<T: 'static>(values: Vec<T>, row: impl Fn(&T) -> Option<gtk::Widget> + 'static) -> gtk::Box {
    let card = list_card(Vec::new());
    let row = Rc::new(row);
    let mut steps: std::collections::VecDeque<Box<dyn FnOnce()>> = Default::default();
    for (index, value) in values.into_iter().enumerate() {
        if index < ROWS_AT_ONCE {
            card.append(&row(&value).unwrap_or_else(|| ui::hbox(0, &[]).upcast()));
        } else {
            let (card, row) = (card.clone(), row.clone());
            steps.push_back(Box::new(move || {
                if let Some(row) = row(&value) {
                    card.append(&row);
                }
            }));
        }
    }
    ui::in_frames(&card, ROWS_PER_FRAME, steps);
    card
}

fn list_card(rows: Vec<gtk::Widget>) -> gtk::Box {
    let card = ui::vbox(0, &["list", "card"]);
    card.set_overflow(gtk::Overflow::Hidden);
    for row in rows {
        card.append(&row);
    }
    card
}

/// Text that wraps, as ui::label, but never with a hyphen where a word is broken: the page broke a path or a
/// long name where it had to (overflow-wrap) and added nothing to it, and Pango would add hyphens
fn wrapping(text: &str, classes: &[&str]) -> gtk::Label {
    let label = ui::label(text, classes);
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrInt::new_insert_hyphens(false));
    label.set_attributes(Some(&attributes));
    label
}

/// One line that is cut short with an ellipsis, as a row's name
fn ellipsized(text: &str, classes: &[&str]) -> gtk::Label {
    let label = ui::line(text, classes);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_hexpand(true);
    label
}

// --- Words ------------------------------------------------------------------------------------------------------

fn time(seconds: f64) -> String {
    i18n::date(seconds, DateStyle::Time)
}

fn short_day(seconds: f64) -> String {
    i18n::date(seconds, DateStyle::WeekdayDayMonthShort)
}

/// When something happened, in a sentence: "today at 18:00", "yesterday at 09:30", "Sat 26 Sep at 12:00"
fn when(seconds: f64) -> String {
    let days = i18n::days_from_today(seconds);
    let day = if days.abs() <= 1 { i18n::days(days) } else { short_day(seconds) };
    t!("{day} at {time}", day = day, time = time(seconds))
}

/// ...and at the start of a line, where it begins with a capital
fn when_first(seconds: f64) -> String {
    let day = match i18n::days_from_today(seconds) {
        0 => "today",
        -1 => "yesterday",
        _ => "other",
    };
    t!(
        "{day, select, today {Today at {time}} yesterday {Yesterday at {time}} other {{date} at {time}}}",
        day = day,
        date = short_day(seconds),
        time = time(seconds)
    )
}

/// "today", "yesterday", "12 days ago", for a count of whole days back, which a clock set back can make less
/// than none
fn days_ago(days: f64) -> String {
    let back = if days.is_finite() { days.max(0.0) as i64 } else { 0 };
    i18n::days(-back)
}

/// How long a scan took: "42 s", "5 min", "1 h 20 min"
fn took(seconds: f64) -> String {
    if seconds < 60.0 {
        return t!("{seconds} s", seconds = seconds);
    }
    let minutes = (seconds / 60.0).round();
    if minutes < 60.0 {
        t!("{minutes} min", minutes = minutes)
    } else {
        t!("{hours} h {minutes} min", hours = (minutes / 60.0).floor(), minutes = minutes % 60.0)
    }
}

/// How long a scan has run, as a clock shows it, in the language's digits: "2:05", "1:02:05"
fn clock(seconds: f64) -> String {
    let pad = |n: f64| i18n::with(|i18n| i18n.number_with(n, 0, 3, 2));
    let hours = (seconds / 3600.0).floor();
    let minutes = (seconds / 60.0).floor() % 60.0;
    if hours > 0.0 {
        format!("{}:{}:{}", i18n::number(hours), pad(minutes), pad(seconds % 60.0))
    } else {
        format!("{}:{}", i18n::number(minutes), pad(seconds % 60.0))
    }
}

pub(crate) fn elapsed(since: f64) -> String {
    clock((backend::now() - since).round().max(0.0))
}

fn basename(file: &str) -> String {
    let name = file.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if name.is_empty() { file.to_string() } else { name.to_string() }
}

fn signatures_text(signatures: Option<&Signatures>) -> String {
    match signatures {
        None => t!("None yet"),
        Some(s) if s.stale => t!("{days, plural, one {# day old} other {# days old}}", days = s.days),
        Some(s) => t!("Updated {when}", when = days_ago(s.days)),
    }
}

/// ...as the note under a running scan says it
fn signatures_note(signatures: Option<&Signatures>) -> String {
    match signatures {
        None => t!("No signatures yet"),
        Some(s) if s.stale => t!("{days, plural, one {Signatures # day old} other {Signatures # days old}}", days = s.days),
        Some(s) => t!("Signatures updated {when}", when = days_ago(s.days)),
    }
}

// What a scan is of, as the values its sentences choose their words by (label_for): home, a number of items,
// or one by its name, the name kept apart from the words around it, whichever way its letters run

fn place(label: &Label) -> &str {
    &label.place
}

fn items(label: &Label) -> f64 {
    label.items.unwrap_or(f64::NAN)
}

fn named(label: &Label) -> String {
    isolate(label.name.as_deref().unwrap_or(""))
}

/// What a scan is of, as a list of them names it
fn scan_name(label: &Label) -> String {
    t!(
        "{place, select, home {Your home folder} items {{items, plural, one {# item} other {# items}}} other {{name}}}",
        place = place(label),
        items = items(label),
        name = named(label)
    )
}

/// ...and while it runs
fn scanning_title(label: &Label) -> String {
    t!(
        "{place, select, home {Scanning your home folder} items {Scanning {items, plural, one {# item} other {# items}}} other {Scanning {name}}}",
        place = place(label),
        items = items(label),
        name = named(label)
    )
}

// --- The report ---------------------------------------------------------------------------------------------------

fn groups() -> [(&'static str, String); 5] {
    [
        ("updates", t!("Updates and boot")),
        ("watching", t!("Watching")),
        ("apps", t!("Apps")),
        ("scans", t!("Scans")),
        ("sends", t!("What this machine sends by itself")),
    ]
}

fn tile_icon(id: &str) -> &'static str {
    match id {
        "security-key" => "key",
        "fingerprint" => "fingerprint",
        "ssh-codes" => "terminal",
        "backups" => "archive",
        "ransomware-protection" => "history",
        "disk-unlock" => "chip",
        "usb-protection" => "plug",
        "private-folder" => "folder",
        "browser-protection" | "browser-privacy" => "monitor",
        "site-checking" => "scan",
        "key-remapping" => "keyboard",
        "on-detection" => "file-alert",
        "realtime" => "activity",
        "network-protection" => "globe",
        "encrypted-dns" => "lock",
        "network-address" => "wifi",
        _ => "shield",
    }
}

/// What was done with what a scan found, as a heading: the weekly scan's paragraph says the same in sentences
/// of its own (attention_card)
fn detection_done(mode: &str) -> String {
    match mode {
        "quarantine" => t!("What it found was moved to quarantine"),
        "delete" => t!("What it found was deleted"),
        _ => t!("Nothing was deleted, quarantined or moved"),
    }
}

fn scan_lead(mode: &str) -> String {
    match mode {
        "quarantine" => t!("What it finds is moved to quarantine, which asks for your password."),
        "delete" => t!("What it finds is deleted, which asks for your password."),
        _ => t!("What it finds is reported, and never deleted, quarantined or moved."),
    }
}

/// What was done to one file a scan started here found
fn outcome_of(outcome: &str) -> Option<(String, &'static str)> {
    Some(match outcome {
        "quarantined" => (t!("Quarantined"), ""),
        "deleted" => (t!("Deleted"), ""),
        "declined" => (t!("Left in place"), "check"),
        "failed" => (t!("Could not be moved"), "check"),
        "clean" => (t!("Clean on a second look"), "ok"),
        _ => return None,
    })
}

/// ...and to all of them, in a few words
fn entry_done(entry: &Entry) -> String {
    let mut outcomes: Vec<&str> = entry.found.iter().filter_map(|item| item.outcome.as_deref()).filter(|o| !o.is_empty() && *o != "reported").collect();
    outcomes.sort();
    outcomes.dedup();
    match outcomes.as_slice() {
        [] => detection_done("report"),
        ["quarantined"] => t!("Moved to quarantine"),
        ["deleted"] => t!("Deleted"),
        ["declined"] => t!("Left where it is, without the password"),
        _ => t!("Each file below says what was done with it"),
    }
}

/// ...and what to know about that
fn done_note(entry: &Entry) -> String {
    let has = |outcome: &str| entry.found.iter().any(|item| item.outcome.as_deref() == Some(outcome));
    if has("quarantined") {
        t!(
            "Quarantined files are kept where nothing can open them. {list} shows them, and {restore} puts one back.",
            list = "ame security scan",
            restore = "ame security scan restore"
        )
    } else if has("deleted") {
        t!("A deleted file can only come back from a backup.")
    } else {
        t!("A match can be a false positive, so check where the file came from first. If you did not mean to have it, delete it in Files, then scan again.")
    }
}

/// Where the manual explains a check, when it is not the Security page
fn manual_page(id: &str) -> &'static str {
    if id.starts_with("key-") {
        return "hardware#secure-boot";
    }
    match id {
        "secure-boot" | "signing-keys" => "hardware#secure-boot",
        "disk-unlock-pin" => "security#unlock-the-disk-with-the-tpm",
        "firmware" | "firmware-dbx" | "firmware-pk" => "updates#firmware",
        "signed-updates" => "updates#signed-updates",
        "single-build" => "updates",
        "network" | "network-protection" => "security#network-protection",
        "home-snapshots" | "ransomware-protection" => "security#ransomware-protection",
        "backup-runs" => "security#backups",
        "containers" => "software#packages-from-other-distributions",
        "app-permissions" => "security#app-permissions",
        "home-networks" => "security#on-from-the-start",
        _ => "security",
    }
}

fn needs_attention(check: &Check) -> bool {
    check.group != "optional" && check.group != "sends" && (check.state == "check" || check.state == "off")
}

/// A check's details, and apart from them the one ending in a colon that introduces its command
fn explain(check: &Check) -> (Vec<String>, String) {
    let mut lines = check.details.clone();
    let lead = if check.command.is_some() && lines.last().is_some_and(|line| line.ends_with(':')) { lines.pop().unwrap_or_default() } else { String::new() };
    (lines, lead)
}

/// The latest scan started here that found something, until it is dismissed or a later scan of the same place
/// comes back clean
fn open_finding(history: &[Entry]) -> Option<&Entry> {
    let mut cleared = std::collections::HashSet::new();
    for entry in history {
        let place = entry.targets.join("\n");
        if entry.status == "clean" {
            cleared.insert(place);
        } else if entry.status == "found" && !entry.dismissed && !cleared.contains(&place) {
            return Some(entry);
        }
    }
    None
}

/// Something that needs attention: a check, or what a scan here found
enum Item {
    Check(Check),
    Finding(Entry),
}

impl Item {
    fn tone(&self) -> &str {
        match self {
            Item::Check(check) => &check.state,
            Item::Finding(_) => "off",
        }
    }
}

fn attention_items(state: &PageState, history: &[Entry]) -> Vec<Item> {
    let mut items: Vec<Item> = state.report.as_ref().map(|r| r.checks.iter().filter(|c| needs_attention(c)).cloned().map(Item::Check).collect()).unwrap_or_default();
    if let Some(finding) = open_finding(history) {
        items.insert(0, Item::Finding(finding.clone()));
    }
    // What is off before what only wants a look
    items.sort_by_key(|item| if item.tone() == "off" { 0 } else { 1 });
    items
}

// --- Network protection ----------------------------------------------------------------------------------------------

/// Connections blocked since a moment, in seconds, counting each time a rule went off
fn blocked_since(network: Option<&Network>, seconds: f64) -> f64 {
    network
        .map(|n| n.events.iter().filter(|e| e.action == "blocked" && e.time >= seconds).map(|e| if e.count != 0.0 { e.count } else { 1.0 }).sum())
        .unwrap_or(0.0)
}

/// ...and in the last day, which the sidebar counts
fn recent_blocks(network: Option<&Network>) -> f64 {
    blocked_since(network, backend::now() - 86400.0)
}

fn rules_text(rules: Option<f64>) -> String {
    match rules.filter(|r| *r != 0.0) {
        None => t!("None yet"),
        Some(rules) => t!("Updated {when}", when = days_ago(((backend::now() - rules) / 86400.0).floor())),
    }
}

// --- App permissions --------------------------------------------------------------------------------------------------

// What lets an app leave its sandbox altogether, and what lets it see what is typed into another app: the ids
// amethystora-app-permissions gives each reach, as the report sorts them too
const ESCAPES: [&str; 6] = ["session-bus", "system-bus", "flatpak", "systemd", "autostart", "installations"];
const TYPING: [&str; 2] = ["x11", "input"];

fn leaves(app: &backend::App) -> bool {
    app.reaches.iter().any(|reach| ESCAPES.contains(&reach.id.as_str()))
}

fn types(app: &backend::App) -> bool {
    app.reaches.iter().any(|reach| TYPING.contains(&reach.id.as_str()))
}

fn reach_tone(id: &str) -> &'static str {
    if ESCAPES.contains(&id) {
        "off"
    } else if TYPING.contains(&id) {
        "check"
    } else {
        "info"
    }
}

/// The apps that can leave their sandbox first, then those that see typing, then the rest by name
fn weight(app: &backend::App) -> i32 {
    if leaves(app) {
        0
    } else if types(app) {
        1
    } else {
        2
    }
}

// --- Installed outside Flatpak -----------------------------------------------------------------------------------------

const DAY: f64 = 86400.0;

/// A container left two weeks without an upgrade, as the report counts it (amethystora-security-status)
fn stale(container: &Container) -> bool {
    let since = container.last_upgrade.or(container.created).unwrap_or(0.0);
    backend::now() - since >= 14.0 * DAY
}

fn upgraded_text(container: &Container) -> String {
    let Some(since) = container.last_upgrade else { return t!("never upgraded") };
    let when = days_ago(((backend::now() - since) / DAY).floor());
    if container.last_upgrade_result == "failed" {
        t!("upgraded {when}, and the last try failed", when = when)
    } else {
        t!("upgraded {when}", when = when)
    }
}

/// The app grid shows an exported app with the container it runs in after its name
fn app_name(name: &str) -> String {
    match name.rfind(" (on ") {
        Some(at) if name.ends_with(')') && !name[at + 5..name.len() - 1].contains(')') => name[..at].to_string(),
        _ => name.to_string(),
    }
}

/// The names of what is in a container, in a list the language's way: "vim, git, htop"
fn names(list: &[String]) -> String {
    i18n::with(|i18n| i18n.list_kind(list, ListKind::Unit))
}

// --- Settings --------------------------------------------------------------------------------------------------------

/// The values a switch can take, in words
fn value_words(value: &str) -> String {
    match value {
        "off" => t!("Off"),
        "on" => t!("On"),
        "record" => t!("Record only"),
        "manual" => t!("Manual"),
        "weekly" => t!("Weekly"),
        "daily" => t!("Daily"),
        "hourly" => t!("Hourly"),
        "all" => t!("All"),
        "critical" => t!("Critical only"),
        "none" => t!("None"),
        "report" => t!("Report"),
        "quarantine" => t!("Quarantine"),
        "delete" => t!("Delete"),
        "watch" => t!("Watch"),
        "block" => t!("Block"),
        "standard" => t!("Standard"),
        "strict" => t!("Strict"),
        "skip" => t!("Skip"),
        "run" => t!("Run"),
        _ => value.to_string(),
    }
}

/// The Settings page's categories, which amethystora-security-config and amethystora-hardening give their switches:
/// the id, the icon, and what it holds
fn categories() -> [(&'static str, &'static str, String, String); 11] {
    [
        ("watching", "activity", t!("The security watcher"), t!("What it reads every 15 minutes, or as it happens: the journal, the settings against the image's, programs open to the network, and what each session finds first.")),
        ("audit", "history", t!("What the audit log records"), t!("The kernel's own record of changes to accounts, to who may become root, to what starts by itself and to the drivers, which the watcher reads.")),
        ("detection", "shield-alert", t!("Looking for new threats"), t!("Programs, secrets, memory and snapshots watched for what an attacker does. Off as the image ships: a profile above turns them on in one step.")),
        ("scans", "scan", t!("Virus scans"), t!("When ClamAV and the settings audit run, the YARA rules they scan with, and what is done with what they find.")),
        ("network", "globe", t!("Network"), t!("The firewall, how this machine shows itself on a network, its clock, and network protection's look at the traffic.")),
        ("logins", "key", t!("Logins"), t!("Wrong passwords, logins over SSH, and a screen left locked.")),
        ("devices", "plug", t!("Devices"), t!("What Bluetooth, the webcam, FireWire, virtual keyboards and the drives you plug in may do.")),
        ("apps", "apps", t!("Apps and updates"), t!("What Flatpak apps can reach, which apps, extensions and packages may be installed, and the signature every update must carry.")),
        ("system", "chip", t!("Kernel and boot"), t!("The kernel's settings and arguments, the drivers that may not load, the boot menu's password, and what a crash leaves behind.")),
        ("notices", "info", t!("What is said"), t!("Which findings become a notification.")),
        ("away", "arrow-right", t!("What leaves this machine"), t!("What it sends by itself, and the features that send something to somebody else, each explained before it is turned on.")),
    ]
}

/// How many of a category's switches are on, of how many
fn on_count(switches: &Switches, group: &str) -> (usize, usize) {
    let settings: Vec<&Setting> = switches.settings.iter().filter(|e| e.group == group).collect();
    let hardening: Vec<&Hardening> = switches.hardening.iter().filter(|e| e.group == group).collect();
    let on = settings.iter().filter(|e| e.value != "off" && e.value != "none").count() + hardening.iter().filter(|e| e.state == "on").count();
    (on, settings.len() + hardening.len())
}

/// What a switch costs while it is on, from systemd's accounting: memory now, or its last run
fn cost_text(cost: Option<&backend::Cost>) -> String {
    let Some(cost) = cost else { return String::new() };
    if let Some(memory) = cost.memory.filter(|m| *m != 0.0) {
        return t!("Uses {size} MB of memory now", size = i18n::number((memory / 1048576.0).round()));
    }
    if let Some(seconds) = cost.seconds.filter(|s| s.is_finite()) {
        return t!("Its last run took {time}", time = took(seconds));
    }
    // A part of the security watcher, timed in its last run
    if let Some(ms) = cost.milliseconds.filter(|m| m.is_finite()) {
        return if ms < 1000.0 {
            t!("{ms} ms of each run of the watcher", ms = i18n::number(ms))
        } else {
            t!("Its last run took {time}", time = took((ms / 1000.0).round()))
        };
    }
    String::new()
}

// --- The page ------------------------------------------------------------------------------------------------------

/// The sidebar entry each page is under
fn nav_of(page: &str) -> &'static str {
    match page {
        "scan" | "scanning" | "result" => "scan",
        "network" => "network",
        page if page == "apps" || page.starts_with("apps/") => "apps",
        "containers" => "containers",
        "events" => "events",
        page if page == "settings" || page.starts_with("settings/") => "settings",
        _ => "overview",
    }
}

/// ...and the recipe that does in a terminal what it does
fn recipe(page: &str) -> &'static str {
    if page == "settings/duress" {
        return "ame security duress";
    }
    match nav_of(page) {
        "scan" => "ame security scan",
        "network" => "ame security connections",
        "apps" => "ame security apps",
        "containers" => "amepkg containers list",
        "settings" => "ame security settings",
        "events" => "ame security events",
        _ => "ame security status",
    }
}

impl Security {
    // --- Buttons ----------------------------------------------------------------------------------------------

    fn button_with(&self, label: &str, opts: Opts, on_click: impl Fn(&Rc<Security>, &gtk::Button) + 'static) -> gtk::Button {
        let mut classes = vec!["btn"];
        if opts.small {
            classes.push("small");
        }
        if opts.primary {
            classes.push("primary");
        }
        let icon = opts.icon.map(|name| icon_for(name, 16));
        let button = gtk::Button::builder().child(&content(label, icon, opts.after)).css_classes(classes).build();
        button.set_sensitive(!opts.disabled);
        if let Some(title) = &opts.title {
            button.set_tooltip_text(Some(title));
        }
        let weak = self.me.clone();
        button.connect_clicked(move |button| {
            if let Some(this) = weak.upgrade() {
                on_click(&this, button);
            }
        });
        button
    }

    fn button(&self, label: &str, opts: Opts, on_click: impl Fn(&Rc<Security>) + 'static) -> gtk::Button {
        self.button_with(label, opts, move |this, _| on_click(this))
    }

    fn link_button(&self, label: &str, icon: Option<&'static str>, quiet: bool, small: bool, on_click: impl Fn(&Rc<Security>) + 'static) -> gtk::Button {
        let mut classes = vec!["link"];
        if quiet {
            classes.push("quiet");
        }
        if small {
            classes.push("small");
        }
        let button = gtk::Button::builder().child(&content(label, icon.map(|name| icon_for(name, 16)), false)).css_classes(classes).build();
        button.set_halign(gtk::Align::Start);
        let weak = self.me.clone();
        button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                on_click(&this);
            }
        });
        button
    }

    fn manual_link(&self, label: &str, page: &'static str) -> gtk::Button {
        self.link_button(label, Some("book"), false, false, move |_| process::manual(page))
    }

    /// A button that asks for a scan
    fn begin_button(&self, label: &str, opts: Opts, request: Request) -> gtk::Button {
        self.button(label, opts, move |this| {
            let this = this.clone();
            glib::spawn_future_local(async move {
                let asked = request(this.clone());
                this.begin(asked).await;
            });
        })
    }

    /// One button for each value, the current one pressed; the others do what `pick` does with it
    fn segmented(&self, title: &str, values: &[(String, String)], current: &str, pick: impl Fn(&Rc<Security>, &str) + Clone + 'static) -> gtk::Box {
        let group = ui::hbox(0, &["segmented"]);
        group.set_overflow(gtk::Overflow::Hidden);
        group.set_hexpand(false);
        group.set_halign(gtk::Align::Start);
        group.update_property(&[gtk::accessible::Property::Label(title)]);
        for (value, label) in values {
            let active = value == current;
            let button = gtk::Button::builder().child(&ui::line(label, &[])).css_classes(["plain", "segment"]).build();
            button.update_state(&[gtk::accessible::State::Pressed(if active { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
            if active {
                button.add_css_class("active");
            } else {
                let weak = self.me.clone();
                let (value, pick) = (value.clone(), pick.clone());
                button.connect_clicked(move |_| {
                    if let Some(this) = weak.upgrade() {
                        pick(&this, &value);
                    }
                });
            }
            group.append(&button);
        }
        group
    }

    fn grid(&self, children: Vec<gtk::Widget>, class: &str, stretch: bool) -> gtk::Grid {
        let grid = gtk::Grid::builder().column_homogeneous(true).column_spacing(16).row_spacing(16).build();
        grid.add_css_class(class);
        // A card's text grows to push its foot down, as `flex: 1` did, within its row only: the grid itself is
        // as tall as its rows, not as tall as the window
        grid.set_vexpand(false);
        let columns = self.columns.get().max(1) as usize;
        let count = children.len();
        for (at, child) in children.into_iter().enumerate() {
            if !stretch {
                child.set_valign(gtk::Align::Start);
            }
            grid.attach(&child, (at % columns) as i32, (at / columns) as i32, 1, 1);
        }
        // The columns are there even when there are fewer cards than them
        for at in count..columns {
            grid.attach(&gtk::Box::new(gtk::Orientation::Vertical, 0), at as i32, 0, 1, 1);
        }
        grid
    }

    fn tilde(&self, file: &str) -> String {
        let home = &self.home;
        if !home.is_empty() && (file == home || file.starts_with(&format!("{home}/"))) { format!("~{}", &file[home.len()..]) } else { file.to_string() }
    }

    fn detection(&self) -> String {
        let mode = self.state.borrow().report.as_ref().map(|r| r.settings.on_detection.clone()).unwrap_or_default();
        if mode.is_empty() { "report".into() } else { mode }
    }

    // --- Navigation --------------------------------------------------------------------------------------------

    pub(crate) fn render_nav(&self) {
        let state = self.state.borrow();
        let history = self.history.borrow();
        let current = nav_of(&state.page);
        self.frame.terminal.set_label(recipe(&state.page));
        let items = attention_items(&state, &history);
        let machine = state.scanner.as_ref().map(|s| &s.machine);
        let busy = state.running.is_some() || machine.is_some_and(|m| m.running);
        let findings = open_finding(&history).is_some() as usize + machine.is_some_and(|m| m.found) as usize;
        let blocked = recent_blocks(state.network.as_ref());
        let count = |n: f64| (n > 0.0).then(|| i18n::number(n));
        for (page, button, badge) in &self.frame.nav {
            button.set_current(*page == current);
            match *page {
                "overview" => {
                    let tone = if items.iter().any(|item| item.tone() == "off") { "off" } else { "check" };
                    badge.set(count(items.len() as f64).as_deref(), false, tone);
                }
                "network" => badge.set(count(blocked).as_deref(), false, "off"),
                "containers" => {
                    let behind = state.inventory.as_ref().map(|i| i.containers.iter().filter(|c| stale(c)).count()).unwrap_or(0);
                    badge.set(count(behind as f64).as_deref(), false, "check");
                }
                "apps" => {
                    let leaving = state.permissions.as_ref().map(|p| p.apps.iter().filter(|a| leaves(a)).count()).unwrap_or(0);
                    badge.set(count(leaving as f64).as_deref(), false, "off");
                }
                "settings" | "events" => badge.set(None, false, ""),
                _ if busy => badge.set(None, true, "busy"),
                _ => badge.set(count(findings as f64).as_deref(), false, "off"),
            }
        }
    }

    pub(crate) fn render(&self) {
        let page = self.state.borrow().page.clone();
        let adjustment = self.frame.scroller.vadjustment();
        let keep = if *self.frame.shown.borrow() == page { adjustment.value() } else { 0.0 };
        *self.live.borrow_mut() = Live::default();
        let built = self.build();
        self.render_nav();
        ui::clear(&self.frame.view);
        self.frame.view.append(&built);
        self.frame.shown.replace(self.state.borrow().page.clone());
        adjustment.set_value(keep);
        // ...and once the new page has been measured, and is long enough again, its lists filling in
        let frames = std::cell::Cell::new(0);
        self.frame.scroller.add_tick_callback(move |_, _| {
            adjustment.set_value(keep);
            frames.set(frames.get() + 1);
            if (adjustment.value() - keep).abs() < 0.5 || frames.get() > 120 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
        });
    }

    fn build(&self) -> gtk::Widget {
        let page = self.state.borrow().page.clone();
        match page.as_str() {
            "scan" => self.scan_page(),
            "scanning" => self.scanning_page(),
            "result" => self.result_page(),
            "network" => self.network_page(),
            "apps" => self.apps_page(None),
            page if page.starts_with("apps/") => self.apps_page(Some(&page["apps/".len()..])),
            "containers" => self.containers_page(),
            "settings" => self.settings_page(),
            "settings/duress" => self.duress_page(),
            page if page.starts_with("settings/") => self.category_page(&page["settings/".len()..]),
            "events" => self.events_page(),
            _ => self.overview_page(),
        }
    }

    pub(crate) fn show(&self, page: &str, result: Option<Shown>) {
        {
            let mut state = self.state.borrow_mut();
            state.page = page.to_string();
            if let Some(result) = result {
                state.result = Some(result);
            }
        }
        self.render();
    }

    /// A page of the usual kind: at most 1000 pixels wide, in the middle
    fn page(children: Vec<gtk::Widget>) -> gtk::Widget {
        let page = ui::vbox(32, &["page"]);
        for child in children {
            page.append(&child);
        }
        adw::Clamp::builder().maximum_size(1000).tightening_threshold(1000).child(&page).build().upcast()
    }

    fn notice_widget(&self) -> Option<gtk::Widget> {
        let notice = self.state.borrow().notice.clone();
        (!notice.is_empty()).then(|| self::notice(&notice).upcast())
    }

    // --- Overview -----------------------------------------------------------------------------------------------

    fn overview_page(&self) -> gtk::Widget {
        let Some(report) = self.state.borrow().report.clone() else {
            let loading = ui::vbox(0, &["loading"]);
            let height = self.shell.window.height().max(self.shell.window.default_height());
            loading.set_size_request(-1, (height as f64 * 0.7) as i32);
            let text = ui::line(&t!("Checking this machine…"), &[]);
            text.set_halign(gtk::Align::Center);
            text.set_valign(gtk::Align::Center);
            text.set_vexpand(true);
            loading.append(&text);
            return loading.upcast();
        };
        let items = attention_items(&self.state.borrow(), &self.history.borrow());
        let on = report.checks.iter().filter(|c| c.group != "optional" && c.state == "ok").count();
        let mut tone = if items.iter().any(|item| item.tone() == "off") {
            "off"
        } else if !items.is_empty() {
            "check"
        } else {
            "ok"
        };
        let mut headline = t!("Everything is on");
        let mut summary = t!("{count, plural, one {# check is on, and nothing needs you right now.} other {All # checks are on, and nothing needs you right now.}}", count = on);
        if !items.is_empty() {
            headline = t!("{count, plural, =1 {One thing to look at} other {# things to look at}}", count = items.len());
            summary = if on > 0 { t!("{count, plural, =1 {The other check is on.} other {The other # checks are all on.}}", count = on) } else { String::new() };
        }
        if let Some(error) = &report.error {
            tone = "check";
            headline = t!("The report could not be read");
            summary = error.clone();
        }
        let mut children: Vec<gtk::Widget> = vec![self.hero(&report, &headline, &summary, tone, items.len()).upcast()];
        if !items.is_empty() {
            let stack = ui::vbox(12, &["stack"]);
            for item in &items {
                stack.append(&match item {
                    Item::Check(check) => self.attention_card(check),
                    Item::Finding(entry) => self.finding_card(entry),
                });
            }
            children.push(section(&t!("Needs your attention"), &stack, None).upcast());
        }
        if let Some(groups) = self.groups_section(&report, !items.is_empty()) {
            children.push(groups.upcast());
        }
        if let Some(optional) = self.optional_section(&report) {
            children.push(optional.upcast());
        }
        Self::page(children)
    }

    fn emblem(tone: &str, count: usize) -> gtk::Overlay {
        let emblem = gtk::Overlay::new();
        emblem.add_css_class("emblem");
        emblem.set_child(Some(&Shield::new(84)));
        let badge = ui::hbox(0, &["emblem-badge", tone]);
        if count > 0 {
            let number = ui::line(&i18n::number(count as f64), &[]);
            number.set_xalign(0.5);
            number.set_hexpand(true);
            badge.append(&number);
        } else {
            let icon = icon_with(if tone == "ok" { "mark-ok" } else { "mark-alert" }, 14, 3.2);
            icon.set_hexpand(true);
            badge.append(&icon);
        }
        emblem.add_overlay(&badge);
        // inset-inline-end: -4px; bottom: 0
        emblem.connect_get_child_position(|emblem, child| {
            let (_, natural) = child.preferred_size();
            let past = (4.0 * amethystora_ui::icon::zoom()).round() as i32;
            let x = if emblem.direction() == gtk::TextDirection::Rtl { -past } else { emblem.width() - natural.width() + past };
            Some(gdk::Rectangle::new(x, emblem.height() - natural.height(), natural.width(), natural.height()))
        });
        emblem.set_valign(gtk::Align::Center);
        emblem.set_hexpand(false);
        emblem
    }

    fn hero(&self, report: &backend::Report, headline: &str, summary: &str, tone: &str, count: usize) -> gtk::Widget {
        let hero = ui::hbox(26, &["hero"]);
        hero.update_property(&[gtk::accessible::Property::Label(headline)]);
        hero.append(&Self::emblem(tone, count));
        let text = ui::vbox(6, &["hero-text"]);
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        // The image's name reads left to right in any language, on one line: in a window too narrow for it, it is
        // cut short where the page let it run on under the button beside it
        let image = ellipsized(&report.image, &["mono", "soft", "small-type"]);
        image.set_direction(gtk::TextDirection::Ltr);
        text.append(&image);
        text.append(&wrapping(headline, &["h1"]));
        if !summary.is_empty() {
            text.append(&ui::limit(&wrapping(summary, &["lead"]), 520));
        }
        hero.append(&text);
        // Beside the words, or under them in a window of one column, which would leave the words a few letters wide
        let under = self.columns.get() == 1;
        let side = ui::vbox(8, &["hero-side"]);
        side.set_valign(gtk::Align::Start);
        let end = if under { gtk::Align::Start } else { gtk::Align::End };
        let again = self.button_with(&t!("Check again"), Opts { icon: Some("refresh"), ..Default::default() }, |this, button| {
            button.set_sensitive(false);
            if let Some(words) = ui::button_words(button) {
                words.set_label(&t!("Checking…"));
            }
            let this = this.clone();
            glib::spawn_future_local(async move { this.reload().await });
        });
        again.set_halign(end);
        side.append(&again);
        let checked = ui::line(&t!("Checked at {time}", time = time(report.checked)), &["faint", "small-type"]);
        checked.set_halign(end);
        side.append(&checked);
        if under {
            side.set_margin_top(10);
            text.append(&side);
        } else {
            hero.append(&side);
        }
        art::hero_glow(&hero)
    }

    fn attention_card(&self, check: &Check) -> gtk::Box {
        let (lines, lead) = explain(check);
        // The weekly scan's findings are in a root-only log, which the recipe shows after asking for a password
        let weekly = check.id == "virus-scan";
        let card = ui::hbox(18, &["attention", check.state.as_str()]);
        let square = tinted(&["attention-icon", "tone", check.state.as_str()], icons::icon(if check.state == "off" { "alert" } else { "shield-alert" }, 22));
        square.set_valign(gtk::Align::Start);
        card.append(&square);
        let body = ui::vbox(10, &["attention-body"]);
        body.set_hexpand(true);
        let head = ui::vbox(4, &["attention-head"]);
        let title = ui::wrap(10, 10, &["attention-title"]);
        title.append(&pill(&if check.state == "off" { t!("Off") } else { t!("Check") }, &check.state, false, false));
        let h3 = wrapping(&check.text, &["h3"]);
        h3.set_valign(gtk::Align::Center);
        title.append(&h3);
        head.append(&title);
        for line in &lines {
            head.append(&ui::limit(&wrapping(line, &["p", "soft"]), 640));
        }
        if weekly {
            head.append(&ui::limit(
                &wrapping(
                    &t!(
                        "{detection, select, quarantine {What it found was moved to quarantine.} delete {What it found was deleted.} other {Nothing was deleted, quarantined or moved.}} What it found is in a log only an administrator can read, so seeing it asks for your password.",
                        detection = self.detection()
                    ),
                    &["p", "soft"],
                ),
                640,
            ));
        }
        body.append(&head);
        if let (Some(command), false) = (&check.command, weekly) {
            body.append(&command_block(&lead, command));
        }
        let mut buttons: Vec<gtk::Widget> = Vec::new();
        if check.command.is_some() {
            let id = check.id.clone();
            let opts = if weekly { Opts { icon: Some("arrow-right"), after: true, ..Default::default() } } else { Opts { icon: Some("terminal"), ..Default::default() } };
            buttons.push(
                self.button(&if weekly { t!("See what it found") } else { t!("Run in a terminal") }, opts, move |this| {
                    backend::run_check(this.state.borrow().report.as_ref(), &id)
                })
                .upcast(),
            );
        }
        let page = manual_page(&check.id);
        buttons.push(self.link_button(&t!("{check} in the manual", check = check.title.as_str()), Some("book"), false, false, move |_| process::manual(page)).upcast());
        // The AI agent looks into it in a terminal and changes nothing until asked; not offered at all while the
        // account has the agentic features off
        if self.state.borrow().report.as_ref().is_some_and(|r| r.agent) {
            let id = check.id.clone();
            buttons.push(self.link_button(&t!("Ask the agent"), Some("sparkle"), false, false, move |this| backend::diagnose(this.state.borrow().report.as_ref(), &id)).upcast());
        }
        // What cannot be fixed on this machine can be accepted: shown as information from then on, until it
        // turns worse. Not what something found, which has its own way to be read.
        if check.id != "virus-scan" && check.id != "security-watcher" {
            let id = check.id.clone();
            buttons.push(self.link_button(&t!("Accept"), Some("mark-ok"), true, false, move |this| backend::accept(this.state.borrow().report.as_ref(), &id, false)).upcast());
        }
        body.append(&actions(buttons));
        card.append(&body);
        card
    }

    fn finding_title(entry: &Entry) -> String {
        let label = &entry.label;
        if entry.single {
            t!("{name} matches known malware", name = named(label))
        } else {
            t!(
                "{place, select, home {A scan of your home folder found {found, plural, one {# file} other {# files}}} items {A scan of {items, plural, one {# item} other {# items}} found {found, plural, one {# file} other {# files}}} other {A scan of {name} found {found, plural, one {# file} other {# files}}}}",
                place = place(label),
                items = items(label),
                name = named(label),
                found = entry.found.len()
            )
        }
    }

    fn finding_card(&self, entry: &Entry) -> gtk::Box {
        let first = entry.found.first();
        let card = ui::hbox(18, &["attention", "off"]);
        let square = tinted(&["attention-icon", "tone", "off"], icons::icon("file-alert", 22));
        square.set_valign(gtk::Align::Start);
        card.append(&square);
        let body = ui::vbox(10, &["attention-body"]);
        body.set_hexpand(true);
        let head = ui::vbox(4, &["attention-head"]);
        let title = ui::wrap(10, 10, &["attention-title"]);
        title.append(&pill(&t!("Found"), "off", false, false));
        let h3 = wrapping(&Self::finding_title(entry), &["h3"]);
        h3.set_valign(gtk::Align::Center);
        title.append(&h3);
        head.append(&title);
        // A signature's name reads left to right in any language
        let signature = first.filter(|f| !f.signature.is_empty()).map(|f| ltr(&f.signature)).unwrap_or_default();
        head.append(&ui::limit(&wrapping(&dotted(&[when_first(entry.started_seconds()), signature, entry_done(entry)]), &["p", "soft"]), 640));
        body.append(&head);
        card.append(&body);
        let side = ui::vbox(8, &["attention-side"]);
        side.set_valign(gtk::Align::Center);
        let shown = entry.clone();
        let see = self.button(&t!("See what it found"), Opts { icon: Some("arrow-right"), after: true, ..Default::default() }, move |this| {
            this.show("result", Some(Shown::Entry(Box::new(shown.clone()))))
        });
        see.set_halign(gtk::Align::End);
        side.append(&see);
        let id = entry.id.clone();
        let dismiss = self.link_button(&t!("Dismiss"), None, true, true, move |this| {
            this.dismiss(&id);
            this.render();
        });
        dismiss.set_halign(gtk::Align::End);
        side.append(&dismiss);
        card.append(&side);
        card
    }

    fn check_row(&self, check: &Check) -> gtk::Box {
        let (lines, _) = explain(check);
        let row = ui::hbox(12, &["row"]);
        let dot = dot(if check.state == "ok" { "ok" } else { "info" });
        dot.set_valign(gtk::Align::Start);
        row.append(&dot);
        let text = ui::vbox(2, &["row-text"]);
        text.set_hexpand(true);
        let title = ui::hbox(0, &["row-title-line"]);
        let name = wrapping(&check.title, &["row-title"]);
        name.set_valign(gtk::Align::Center);
        title.append(&name);
        if check.accepted {
            title.append(&pill(&t!("Accepted"), "", true, false));
        }
        text.append(&title);
        text.append(&wrapping(&check.text, &["row-detail"]));
        if !lines.is_empty() {
            text.append(&wrapping(&lines.join(" "), &["row-detail"]));
        }
        if check.command.is_some() {
            let id = check.id.clone();
            let label = if check.switched_off {
                t!("Turn it back on")
            } else if check.group == "sends" {
                t!("Change it")
            } else {
                t!("Run it now")
            };
            text.append(&self.link_button(&label, None, false, true, move |this| backend::run_check(this.state.borrow().report.as_ref(), &id)));
        }
        if check.accepted {
            let id = check.id.clone();
            text.append(&self.link_button(&t!("Show it again"), None, false, true, move |this| backend::accept(this.state.borrow().report.as_ref(), &id, true)));
        }
        row.append(&text);
        row
    }

    fn groups_section(&self, report: &backend::Report, attention: bool) -> Option<gtk::Box> {
        let rows_of = |group: &str| report.checks.iter().filter(|c| c.group == group && !needs_attention(c)).cloned().collect::<Vec<Check>>();
        // Each group across the page, its rows two by two: cards of a few rows and of many, side by side in a grid,
        // left rows of gaps under the shorter ones
        let columns = self.columns.get().clamp(1, 2) as usize;
        let cards = ui::vbox(16, &["groups"]);
        let mut any = false;
        for (group, title) in groups() {
            let rows = rows_of(group);
            if rows.is_empty() {
                continue;
            }
            let on = rows.iter().filter(|c| c.state == "ok").count();
            let card = ui::vbox(0, &["group", "card"]);
            card.set_overflow(gtk::Overflow::Hidden);
            let head = ui::hbox(8, &["group-head"]);
            let h3 = wrapping(&title, &["h3"]);
            h3.set_hexpand(true);
            h3.set_valign(gtk::Align::Center);
            head.append(&h3);
            if on > 0 {
                head.append(&pill(&t!("{count} on", count = on), "ok", true, false));
            }
            card.append(&head);
            let grid = gtk::Grid::builder().column_homogeneous(true).build();
            for (at, check) in rows.iter().enumerate() {
                grid.attach(&self.check_row(check), (at % columns) as i32, (at / columns) as i32, 1, 1);
            }
            // The last line of rows is drawn across the card even when it has one row only
            if rows.len() % columns != 0 {
                grid.attach(&ui::vbox(0, &["row"]), (columns - 1) as i32, (rows.len() / columns) as i32, 1, 1);
            }
            card.append(&grid);
            cards.append(&card);
            any = true;
        }
        if !any {
            return None;
        }
        // What the machine sends is information, on or off, so it does not count against "everything else is on"
        let all_on = groups().iter().filter(|(group, _)| *group != "sends").all(|(group, _)| rows_of(group).iter().all(|c| c.state == "ok"));
        let title = if attention {
            if all_on { t!("Everything else is on") } else { t!("Everything else") }
        } else {
            t!("What is on")
        };
        Some(section(&title, &cards, None))
    }

    fn optional_section(&self, report: &backend::Report) -> Option<gtk::Box> {
        let optional: Vec<&Check> = report.checks.iter().filter(|c| c.group == "optional").collect();
        if optional.is_empty() {
            return None;
        }
        let mut tiles: Vec<gtk::Widget> = optional
            .iter()
            .map(|check| {
                let unavailable = check.state == "unavailable";
                let state_text = if unavailable {
                    check.details.first().filter(|d| !d.is_empty()).cloned().unwrap_or_else(|| t!("Not available here"))
                } else if check.state == "on" {
                    t!("On")
                } else {
                    t!("Off")
                };
                let tile = ui::vbox(12, &["tile", "card"]);
                let square = tinted(&["tile-icon", "tone", "accent"], icons::icon(tile_icon(&check.id), 20));
                square.set_halign(gtk::Align::Start);
                tile.append(&square);
                let text = ui::vbox(4, &["tile-text"]);
                text.set_vexpand(true);
                text.append(&wrapping(&check.title, &["h3"]));
                text.append(&wrapping(&check.text, &["p"]));
                tile.append(&text);
                let foot = ui::hbox(8, &["tile-foot"]);
                let state = wrapping(&state_text, &["tile-state", check.state.as_str()]);
                state.set_hexpand(true);
                state.set_valign(gtk::Align::Center);
                foot.append(&state);
                let id = check.id.clone();
                let label = if check.state == "on" { t!("Change…") } else { t!("Set up…") };
                let button = self.button(&label, Opts { small: true, disabled: unavailable, ..Default::default() }, move |this| {
                    backend::run_check(this.state.borrow().report.as_ref(), &id)
                });
                button.set_valign(gtk::Align::Center);
                foot.append(&button);
                tile.append(&foot);
                tile.upcast()
            })
            .collect();
        let inside = ui::vbox(12, &[]);
        let text = ui::vbox(4, &["tile-text"]);
        text.set_vexpand(true);
        text.append(&wrapping(&t!("What each one does"), &["tile-title"]));
        text.append(&wrapping(&t!("And how to undo it, in the Security page of the manual."), &["tile-note"]));
        inside.append(&text);
        let link = ui::hbox(0, &["link", "content"]);
        link.append(&icons::icon("book", 16));
        link.append(&ui::line(&t!("Open the manual"), &[]));
        link.set_halign(gtk::Align::Start);
        inside.append(&link);
        let manual = gtk::Button::builder().child(&inside).css_classes(["plain", "tile", "manual-tile"]).build();
        manual.connect_clicked(|_| process::manual("security"));
        tiles.push(manual.upcast());
        Some(section(&t!("Off unless you turn it on"), &self.grid(tiles, "tiles", true), Some(&t!("Setting one up opens it in a terminal"))))
    }

    // --- Virus scan -------------------------------------------------------------------------------------------------

    fn scan_page(&self) -> gtk::Widget {
        let (scanner, running, off) = {
            let state = self.state.borrow();
            (state.scanner.clone(), state.running.clone(), state.report.as_ref().is_some_and(|r| r.settings.virus_scan == "off"))
        };
        let busy = running.is_some() || scanner.as_ref().is_some_and(|s| s.machine.running);
        // VIRUS_SCAN=off: nothing scans, and nothing holds the signatures, until its owner turns it on
        let blocked = off || scanner.as_ref().is_none_or(|s| s.signatures.is_none()) || busy;
        let mut children: Vec<gtk::Widget> =
            vec![page_head(&t!("Virus scan"), &format!("{} {}", t!("ClamAV compares your files with known malware."), scan_lead(&self.detection()))).upcast()];
        children.extend(self.notice_widget());
        if off {
            let turn = self.button(&t!("Turn it on…"), Opts { icon: Some("terminal"), primary: true, ..Default::default() }, |this| {
                backend::change(this.state.borrow().switches.as_ref(), Kind::Setting, "VIRUS_SCAN", "", "")
            });
            children.push(
                callout(
                    "info",
                    &t!("The virus scanner is turned off"),
                    &t!("Nothing is scanned, and no virus signatures are kept in memory or downloaded."),
                    Some(actions(vec![turn.upcast()]).upcast()),
                )
                .upcast(),
            );
        }
        if busy {
            let banner = ui::hbox(14, &["banner"]);
            let spinner = icons::icon("spinner", 18);
            spinner.set_spinning(true);
            banner.append(&spinner);
            let what = wrapping(&running.as_ref().map(|r| scanning_title(&r.label)).unwrap_or_else(|| t!("Scanning the whole machine")), &["grow"]);
            what.set_hexpand(true);
            what.set_valign(gtk::Align::Center);
            banner.append(&what);
            let show = self.link_button(&t!("Show it"), None, false, false, |this| this.show("scanning", None));
            show.set_valign(gtk::Align::Center);
            banner.append(&show);
            children.push(banner.upcast());
        }
        children.push(self.scan_strip(scanner.as_ref()).upcast());
        let home = self.choice(
            true,
            "home",
            &t!("Your home folder"),
            &t!("Everything you keep here, apart from caches."),
            vec![self
                .begin_button(&t!("Scan now"), Opts { primary: true, disabled: blocked, ..Default::default() }, |this| Box::pin(async move { this.scan_home().await }))
                .upcast()],
        );
        let pick = self.choice(
            false,
            "folder",
            &t!("A file or folder"),
            &t!("Choose some, or drop them anywhere on this window."),
            vec![
                self.begin_button(&t!("Files…"), Opts { disabled: blocked, ..Default::default() }, |this| Box::pin(async move { this.scan_pick(false).await })).upcast(),
                self.begin_button(&t!("Folders…"), Opts { disabled: blocked, ..Default::default() }, |this| Box::pin(async move { this.scan_pick(true).await })).upcast(),
            ],
        );
        let machine = self.choice(
            false,
            "monitor",
            &t!("The whole machine"),
            &t!("All home and temporary folders, like the weekly scan."),
            vec![self
                .button(&t!("Scan"), Opts { disabled: blocked, ..Default::default() }, |this| {
                    let this = this.clone();
                    glib::spawn_future_local(async move { this.scan_machine().await });
                })
                .upcast()],
        );
        children.push(section(&t!("Scan"), &self.grid(vec![home.upcast(), pick.upcast(), machine.upcast()], "choices", true), None).upcast());
        children.push(section(&t!("Recent scans"), &self.recent_list(), None).upcast());
        Self::page(children)
    }

    fn choice(&self, main: bool, icon: &str, title: &str, text: &str, buttons: Vec<gtk::Widget>) -> gtk::Widget {
        let choice = ui::vbox(14, &["choice", "card"]);
        if main {
            choice.add_css_class("main");
        }
        let square = tinted(&["choice-icon", "tone", "accent"], icons::icon(icon, 21));
        square.set_halign(gtk::Align::Start);
        choice.append(&square);
        let words = ui::vbox(4, &["choice-text"]);
        words.set_vexpand(true);
        words.append(&wrapping(title, &["h3"]));
        words.append(&wrapping(text, &["p"]));
        choice.append(&words);
        choice.append(&actions(buttons));
        if main { art::choice_glow(&choice) } else { choice.upcast() }
    }

    fn strip_cell(tone: &str, icon: &str, label: &str, value: &str) -> gtk::Box {
        let cell = ui::hbox(14, &["strip-cell"]);
        let square = tinted(&["strip-icon", "tone", tone], icons::icon(icon, 19));
        square.set_valign(gtk::Align::Center);
        cell.append(&square);
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        words.append(&wrapping(&label.to_uppercase(), &["label"]));
        // Wrapped, not cut: four cells side by side leave little room at the window's default width
        words.append(&wrapping(value, &["value"]));
        cell.append(&words);
        cell
    }

    /// Cells side by side in a card, then one above the other in a narrow window
    fn strip(&self, cells: Vec<gtk::Box>) -> gtk::Box {
        let narrow = self.columns.get() == 1;
        let strip = gtk::Box::new(if narrow { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal }, 0);
        strip.set_homogeneous(true);
        strip.set_css_classes(&["strip", "card", if narrow { "rows" } else { "columns" }]);
        for cell in cells {
            strip.append(&cell);
        }
        strip
    }

    fn scan_strip(&self, scanner: Option<&backend::Scanner>) -> gtk::Box {
        let Some(scanner) = scanner else {
            let strip = ui::hbox(0, &["strip", "card"]);
            let cell = ui::hbox(14, &["strip-cell"]);
            cell.append(&ui::line(&t!("Checking the scanner…"), &["soft"]));
            strip.append(&cell);
            return strip;
        };
        let (machine, signatures) = (&scanner.machine, scanner.signatures.as_ref());
        let weekly = if machine.running {
            ("accent", t!("Running now"))
        } else if machine.found {
            ("off", t!("Last one found something"))
        } else if machine.status == Some(2) || machine.status.is_some_and(|s| s > 3) {
            ("check", t!("Last one could not run"))
        } else if machine.finished.is_some() {
            ("ok", t!("Last one found nothing"))
        } else {
            ("accent", t!("Sundays, not run yet"))
        };
        self.strip(vec![
            Self::strip_cell(if signatures.is_some_and(|s| !s.stale) { "ok" } else { "check" }, "database", &t!("Signatures"), &signatures_text(signatures)),
            Self::strip_cell(weekly.0, "calendar", &t!("Weekly scan"), &weekly.1),
            Self::strip_cell(
                if signatures.is_some() { "ok" } else { "check" },
                "activity",
                &t!("Scanner"),
                &if signatures.is_some() { t!("Ready") } else { t!("Waiting for signatures") },
            ),
        ])
    }

    fn recent_list(&self) -> gtk::Box {
        let mut rows: Vec<gtk::Widget> = Vec::new();
        if let Some(scanner) = self.state.borrow().scanner.as_ref() {
            rows.push(self.machine_row(&scanner.machine).upcast());
        }
        let history: Vec<Entry> = self.history.borrow().iter().take(6).cloned().collect();
        for entry in &history {
            rows.push(self.entry_row(entry).upcast());
        }
        if rows.is_empty() { empty(&t!("No scans yet.")) } else { list_card(rows) }
    }

    /// A row of a list that opens what it is about
    fn list_row_button(&self, tone: &str, name: &str, meta: &str, outcome: gtk::Label, on_click: impl Fn(&Rc<Security>) + 'static) -> gtk::Button {
        let row = ui::hbox(14, &[]);
        row.append(&status(tone));
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        words.append(&ellipsized(name, &["strong"]));
        words.append(&wrapping(meta, &["small-text"]));
        row.append(&words);
        row.append(&outcome);
        let chevron = icon_for("chevron", 16);
        chevron.add_css_class("chevron");
        row.append(&chevron);
        let button = gtk::Button::builder().child(&row).css_classes(["plain", "list-row"]).build();
        let weak = self.me.clone();
        button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                on_click(&this);
            }
        });
        button
    }

    fn entry_outcome(entry: &Entry) -> (&'static str, gtk::Label) {
        match entry.status.as_str() {
            "found" => (
                "off",
                pill(&if entry.found.is_empty() { t!("Found") } else { t!("{count} found", count = entry.found.len()) }, "off", false, false),
            ),
            "clean" => ("ok", pill(&t!("Nothing found"), "ok", false, false)),
            "failed" => ("check", pill(&t!("Could not run"), "check", false, false)),
            _ => ("info", pill(&t!("Stopped"), "", false, false)),
        }
    }

    fn entry_row(&self, entry: &Entry) -> gtk::Button {
        let (tone, outcome) = Self::entry_outcome(entry);
        let mut meta = vec![when_first(entry.started_seconds()), t!("took {duration}", duration = took(entry.seconds))];
        if entry.files != 0.0 {
            meta.push(t!("{count, plural, one {# file} other {# files}}", count = entry.files));
        }
        let shown = entry.clone();
        self.list_row_button(tone, &scan_name(&entry.label), &meta.join(" · "), outcome, move |this| this.show("result", Some(Shown::Entry(Box::new(shown.clone())))))
    }

    fn machine_outcome(machine: &Machine) -> (&'static str, gtk::Label) {
        if machine.running {
            ("info", pill(&t!("Running"), "", false, false))
        } else if machine.found {
            ("off", pill(&t!("Found something"), "off", false, false))
        } else if machine.status == Some(2) || machine.status.is_some_and(|s| s > 3) {
            ("check", pill(&t!("Could not run"), "check", false, false))
        } else if machine.finished.is_none() {
            ("info", pill(&t!("Not run yet"), "", false, false))
        } else if machine.status == Some(3) {
            ("check", pill(&t!("Old signatures"), "check", false, false))
        } else {
            ("ok", pill(&t!("Nothing found"), "ok", false, false))
        }
    }

    fn machine_row(&self, machine: &Machine) -> gtk::Button {
        let (tone, outcome) = Self::machine_outcome(machine);
        let meta = if machine.running {
            t!("Every home and temporary folder · running now")
        } else if let Some(finished) = machine.finished {
            t!("Weekly, on Sundays · last {when}", when = when(finished))
        } else {
            t!("Weekly, on Sundays")
        };
        let running = machine.running;
        self.list_row_button(tone, &t!("The whole machine"), &meta, outcome, move |this| {
            if running {
                this.show("scanning", None)
            } else {
                this.show("result", Some(Shown::Machine))
            }
        })
    }

    // --- Scanning -----------------------------------------------------------------------------------------------------

    fn stat(value: &gtk::Label, label: &str) -> gtk::Box {
        let stat = ui::vbox(2, &["stat"]);
        value.set_xalign(0.5);
        stat.append(value);
        let words = wrapping(&label.to_uppercase(), &["label"]);
        words.set_xalign(0.5);
        words.set_justify(gtk::Justification::Center);
        stat.append(&words);
        stat
    }

    pub(crate) fn update_live(&self) {
        let progress = self.state.borrow().running.clone();
        let live = self.live.borrow();
        let (Some(progress), Some(files), Some(found), Some(path)) = (progress, &live.files, &live.found, &live.path) else { return };
        files.set_label(&i18n::number(progress.files));
        found.set_label(&i18n::number(progress.found as f64));
        if progress.found > 0 {
            found.add_css_class("off");
        } else {
            found.remove_css_class("off");
        }
        // A path reads left to right in any language, and the words before there is one the language's way
        if !progress.loading && !progress.current.is_empty() {
            path.set_direction(gtk::TextDirection::Ltr);
            path.set_label(&self.tilde(&progress.current));
        } else {
            path.set_direction(gtk::TextDirection::None);
            path.set_label(&if progress.loading { t!("Loading the virus signatures…") } else { t!("Starting…") });
        }
    }

    fn scanning_page(&self) -> gtk::Widget {
        let (progress, machine, asked, signatures) = {
            let state = self.state.borrow();
            (state.running.clone(), state.scanner.as_ref().map(|s| s.machine.clone()), state.machine_asked, state.scanner.as_ref().and_then(|s| s.signatures.clone()))
        };
        if progress.is_none() && !machine.as_ref().is_some_and(|m| m.running) {
            self.state.borrow_mut().page = "scan".into();
            return self.scan_page();
        }
        let since = match &progress {
            Some(progress) => progress.started / 1000.0,
            None => {
                let started = machine.as_ref().and_then(|m| m.started).unwrap_or(0.0).max(asked);
                if started > 0.0 { started } else { backend::now() }
            }
        };
        let elapsed_label = ui::line(&elapsed(since), &["stat-value"]);
        let mut stats: Vec<gtk::Box> = Vec::new();
        {
            let mut live = self.live.borrow_mut();
            live.since = Some(since);
            live.elapsed = Some(elapsed_label.clone());
            if progress.is_some() {
                let files = ui::line("", &["stat-value"]);
                let found = ui::line("", &["stat-value"]);
                let path = ellipsized("", &["code"]);
                path.set_hexpand(false);
                path.set_halign(gtk::Align::Start);
                stats.push(Self::stat(&files, &t!("Files checked")));
                stats.push(Self::stat(&found, &t!("Found")));
                live.files = Some(files);
                live.found = Some(found);
                live.path = Some(path);
            }
        }
        stats.push(Self::stat(&elapsed_label, &t!("Elapsed")));
        let page = ui::vbox(0, &["scanning"]);
        page.set_vexpand(true);
        let centered = |label: gtk::Label| {
            label.set_justify(gtk::Justification::Center);
            label.set_xalign(0.5);
            label
        };
        page.append(&ScanGem::new());
        page.append(&centered(wrapping(&progress.as_ref().map(|p| scanning_title(&p.label)).unwrap_or_else(|| t!("Scanning the whole machine")), &["h1"])));
        let lead = if progress.is_some() {
            t!("You can keep working meanwhile. Nothing is deleted or moved.")
        } else {
            t!("Every home and temporary folder, at low priority so that it stays out of your way. It carries on if you close this window, and nothing is deleted or moved.")
        };
        page.append(&ui::limit_center(&centered(wrapping(&lead, &["lead"])), 560));
        let card = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        card.set_homogeneous(true);
        card.set_css_classes(&["stats", "card"]);
        for stat in stats {
            card.append(&stat);
        }
        page.append(&ui::limit_center(&card, 560));
        let current = ui::vbox(10, &["current-file"]);
        if let Some(path) = self.live.borrow().path.clone() {
            let line = ui::hbox(8, &["current-path"]);
            line.append(&icons::icon("file", 15));
            line.append(&path);
            current.append(&line);
        }
        let bar = Bar::new();
        bar.set_fraction(None);
        bar.update_property(&[gtk::accessible::Property::Label(&t!("Scanning"))]);
        current.append(&bar);
        page.append(&ui::limit_center(&current, 560));
        let button = if progress.is_some() {
            self.button(&t!("Stop the scan"), Opts { icon: Some("stop"), ..Default::default() }, |this| this.stop())
        } else {
            self.button(&t!("Back to Virus scan"), Opts::default(), |this| this.show("scan", None))
        };
        button.set_halign(gtk::Align::Center);
        page.append(&button);
        page.append(&centered(wrapping(&[signatures_note(signatures.as_ref()), t!("caches are skipped")].join(" · "), &["footnote"])));
        self.update_live();
        page.upcast()
    }

    // --- Results --------------------------------------------------------------------------------------------------------

    fn back(&self) -> gtk::Widget {
        self.link_button(&t!("Virus scan"), Some("arrow-left"), true, false, |this| this.show("scan", None)).upcast()
    }

    fn result_head(tone: &str, icon: &str, title: &str, subtitle: &str) -> gtk::Box {
        let head = ui::hbox(20, &["result-head"]);
        let square = tinted(&["result-icon", "tone", tone], icons::icon(icon, 28));
        square.set_valign(gtk::Align::Center);
        head.append(&square);
        let words = ui::vbox(4, &["grow"]);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        words.append(&wrapping(title, &["h1"]));
        if !subtitle.is_empty() {
            words.append(&ui::limit(&wrapping(subtitle, &["lead"]), 640));
        }
        head.append(&words);
        head
    }

    /// A path, the one value that is mono, reads left to right in any language
    fn fact(label: &str, value: &str, mono: bool) -> gtk::Box {
        let cell = ui::hbox(14, &["strip-cell"]);
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.append(&wrapping(&label.to_uppercase(), &["label"]));
        let value = ellipsized(value, if mono { &["value", "mono"] } else { &["value"] });
        if mono {
            value.set_direction(gtk::TextDirection::Ltr);
        }
        words.append(&value);
        cell.append(&words);
        cell
    }

    fn finding_row(&self, item: &Found) -> gtk::Box {
        let outcome = item.outcome.as_deref().and_then(outcome_of);
        // Gone from where it was: there is nothing to show in Files
        let moved = matches!(item.outcome.as_deref(), Some("quarantined" | "deleted"));
        let row = ui::hbox(16, &["finding", "card"]);
        let square = tinted(&["attention-icon", "tone", "off"], icons::icon("file", 21));
        square.set_valign(gtk::Align::Center);
        row.append(&square);
        let words = ui::vbox(4, &["grow"]);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        let name = ui::wrap(10, 10, &["finding-name"]);
        let file = wrapping(&basename(&item.path), &[]);
        file.set_valign(gtk::Align::Center);
        name.append(&file);
        name.append(&pill(&item.signature, "off", false, true));
        if let Some((text, tone)) = &outcome {
            name.append(&pill(text, tone, true, false));
        }
        words.append(&name);
        let path = wrapping(&self.tilde(&item.path), &["code"]);
        path.set_direction(gtk::TextDirection::Ltr);
        words.append(&path);
        // A YARA rule's author is named with every match, as the rule's licence asks
        if let Some(credit) = &item.credit {
            words.append(&wrapping(
                &t!("YARA rule by {author}, {licence}, from {source}", author = credit.author.as_str(), licence = credit.licence.as_str(), source = credit.source.as_str()),
                &["small-text", "faint"],
            ));
        }
        row.append(&words);
        let side = ui::hbox(10, &["actions"]);
        side.set_valign(gtk::Align::Center);
        if !moved {
            let path = item.path.clone();
            side.append(&self.button(&t!("Show in Files"), Opts { icon: Some("folder"), small: true, ..Default::default() }, move |this| this.show_in_files(&path)));
        }
        side.append(&copy_button(&item.path, &t!("Copy path"), &t!("Copy the path of the file")));
        row.append(&side);
        row
    }

    fn scan_result(&self, entry: &Entry) -> gtk::Widget {
        let found = entry.found.len();
        let one = entry.targets.len() == 1;
        let label = &entry.label;
        let title = match entry.status.as_str() {
            "found" if found == 0 => t!(
                "{place, select, home {The scan of your home folder found something} items {The scan of {items, plural, one {# item} other {# items}} found something} other {The scan of {name} found something}}",
                place = place(label),
                items = items(label),
                name = named(label)
            ),
            "found" if entry.single => t!("{name} matches known malware", name = named(label)),
            "found" => t!(
                "{place, select, home {Found {found, plural, one {# file} other {# files}} in your home folder} items {Found {found, plural, one {# file} other {# files}} in {items, plural, one {# item} other {# items}}} other {Found {found, plural, one {# file} other {# files}} in {name}}}",
                place = place(label),
                items = items(label),
                name = named(label),
                found = found
            ),
            "clean" => t!(
                "{place, select, home {No known malware in your home folder} items {No known malware in {items, plural, one {# item} other {# items}}} other {No known malware in {name}}}",
                place = place(label),
                items = items(label),
                name = named(label)
            ),
            "stopped" => t!(
                "{place, select, home {The scan of your home folder was stopped} items {The scan of {items, plural, one {# item} other {# items}} was stopped} other {The scan of {name} was stopped}}",
                place = place(label),
                items = items(label),
                name = named(label)
            ),
            "failed" => t!(
                "{place, select, home {The scan of your home folder could not run} items {The scan of {items, plural, one {# item} other {# items}} could not run} other {The scan of {name} could not run}}",
                place = place(label),
                items = items(label),
                name = named(label)
            ),
            _ => scan_name(label),
        };
        let (tone, icon) = match entry.status.as_str() {
            "found" => ("off", "file-alert"),
            "clean" => ("ok", "mark-ok"),
            "stopped" => ("info", "stop"),
            "failed" => ("check", "alert"),
            _ => ("info", "info"),
        };
        // Joined rather than full-stopped: a time can end in "p.m." already
        let mut subtitle = vec![t!("Scanned {when}", when = when(entry.started_seconds()))];
        if entry.status == "found" && found > 0 && !entry.single {
            subtitle.push(t!("{count, plural, =1 {One file matches known malware} other {# files match known malware}}", count = found));
        }
        if entry.stopped {
            subtitle.push(t!("{count, plural, one {Stopped after # file} other {Stopped after # files}}", count = entry.files));
        }
        let mut children: Vec<gtk::Widget> = vec![self.back(), Self::result_head(tone, icon, &title, &subtitle.join(" · ")).upcast()];
        let target_label = if one {
            if entry.single { t!("File") } else { t!("Folder") }
        } else {
            t!("Scanned")
        };
        let target = if one { self.tilde(&entry.targets[0]) } else { t!("{count, plural, one {# item} other {# items}}", count = entry.targets.len()) };
        children.push(
            self.strip(vec![
                Self::fact(&target_label, &target, one),
                Self::fact(&t!("Files checked"), &i18n::number(entry.files), false),
                Self::fact(&t!("Time"), &took(entry.seconds), false),
                Self::fact(&t!("Signatures"), &signatures_text(entry.signatures.as_ref()), false),
            ])
            .upcast(),
        );
        if entry.status == "found" {
            children.push(callout("info", &entry_done(entry), &done_note(entry), None).upcast());
        }
        if entry.status == "failed" {
            children.push(callout("check", &t!("The scanner could not start"), entry.message.as_deref().unwrap_or(""), None).upcast());
        }
        if entry.errors != 0.0 {
            children.push(
                wrapping(
                    &t!(
                        "{count, plural, one {# file could not be read, and was not checked.} other {# files could not be read, and were not checked.}}",
                        count = entry.errors
                    ),
                    &["soft"],
                )
                .upcast(),
            );
        }
        if found > 0 {
            let stack = ui::vbox(12, &["stack"]);
            for item in &entry.found {
                stack.append(&self.finding_row(item));
            }
            children.push(section(&t!("What it found"), &stack, None).upcast());
        }
        let running = self.state.borrow().running.is_some();
        let id = entry.id.clone();
        let again = self.button(&t!("Scan again"), Opts { primary: true, disabled: running, ..Default::default() }, move |this| {
            let (this, id) = (this.clone(), id.clone());
            glib::spawn_future_local(async move {
                let scanning = this.clone();
                this.begin(async move { scanning.scan_again(&id).await }).await;
            });
        });
        let done = self.button(&t!("Done"), Opts::default(), |this| this.show("scan", None));
        children.push(actions(vec![again.upcast(), done.upcast()]).upcast());
        Self::page(children)
    }

    fn machine_result(&self) -> gtk::Widget {
        let (machine, signatures, running) = {
            let state = self.state.borrow();
            (state.scanner.as_ref().map(|s| s.machine.clone()), state.scanner.as_ref().is_some_and(|s| s.signatures.is_some()), state.running.is_some())
        };
        let Some(machine) = machine else {
            self.state.borrow_mut().page = "scan".into();
            return self.scan_page();
        };
        if machine.running {
            self.state.borrow_mut().page = "scanning".into();
            return self.scanning_page();
        }
        let finished = machine.finished.map(|f| t!("Finished {when}", when = when(f))).unwrap_or_default();
        let mut head = Self::result_head("ok", "mark-ok", &t!("No known malware on this machine"), &finished);
        let mut body: Vec<gtk::Widget> = Vec::new();
        if machine.found {
            head = Self::result_head("off", "file-alert", &t!("The scan of the whole machine found something"), &finished);
            let see = self.button(&t!("See what it found"), Opts { icon: Some("terminal"), ..Default::default() }, |this| {
                backend::run_check(this.state.borrow().report.as_ref(), "virus-scan")
            });
            body.push(
                callout(
                    "info",
                    &detection_done(&self.detection()),
                    &t!("What it found is in a log only an administrator can read, so seeing it asks for your password."),
                    Some(actions(vec![see.upcast()]).upcast()),
                )
                .upcast(),
            );
        } else if machine.finished.is_none() {
            head = Self::result_head("info", "monitor", &t!("The whole machine has not been scanned yet"), &t!("The weekly scan runs on Sundays, or now if you start it."));
        } else if machine.status == Some(2) {
            head = Self::result_head("check", "alert", &t!("There were no signatures to scan with"), &finished);
            body.push(
                callout(
                    "check",
                    &t!("Nothing was scanned"),
                    &t!("The signatures download on their own soon after the machine is installed. If that was a while ago, check its network."),
                    None,
                )
                .upcast(),
            );
        } else if machine.status.is_some_and(|s| s > 3) {
            head = Self::result_head("check", "alert", &t!("The scan of the whole machine could not run"), &finished);
            body.push(callout("check", &t!("Its log says why"), "", Some(command_block("", "journalctl -u amethystora-clamav-scan.service").upcast())).upcast());
        } else if machine.status == Some(3) {
            head = Self::result_head("check", "alert", &t!("Nothing found, with old signatures"), &finished);
            body.push(
                callout("check", &t!("A clean result proves little"), &t!("The signatures are too old for a clean scan to mean much. Check this machine's network."), None)
                    .upcast(),
            );
        }
        let mut children: Vec<gtk::Widget> = vec![self.back(), head.upcast()];
        children.extend(body);
        let again = self.button(&t!("Scan again"), Opts { primary: true, disabled: running || !signatures, ..Default::default() }, |this| {
            let this = this.clone();
            glib::spawn_future_local(async move { this.scan_machine().await });
        });
        let done = self.button(&t!("Done"), Opts::default(), |this| this.show("scan", None));
        children.push(actions(vec![again.upcast(), done.upcast()]).upcast());
        Self::page(children)
    }

    fn result_page(&self) -> gtk::Widget {
        let result = self.state.borrow().result.clone();
        match result {
            None => {
                self.state.borrow_mut().page = "scan".into();
                self.scan_page()
            }
            Some(Shown::Machine) => self.machine_result(),
            Some(Shown::Entry(entry)) => self.scan_result(&entry),
        }
    }

    // --- Network protection ------------------------------------------------------------------------------------------

    fn network_mode(&self) -> String {
        let mode = self.state.borrow().report.as_ref().map(|r| r.settings.network.clone()).unwrap_or_default();
        if mode.is_empty() { "off".into() } else { mode }
    }

    fn network_strip(&self, mode: &str, network: Option<&Network>) -> gtk::Box {
        let on = mode != "off";
        let running = network.is_some_and(|n| n.running);
        let rules = network.and_then(|n| n.rules).filter(|r| *r != 0.0);
        let stale = rules.is_some_and(|rules| backend::now() - rules > 7.0 * 86400.0);
        let week = blocked_since(network, backend::now() - 7.0 * 86400.0);
        let mode_words = match mode {
            "off" => t!("Off"),
            "watch" => t!("Watching"),
            "block" => t!("Blocking"),
            other => other.to_string(),
        };
        self.strip(vec![
            Self::strip_cell(if on { "ok" } else { "info" }, "shield", &t!("Mode"), &mode_words),
            Self::strip_cell(
                if !on {
                    "info"
                } else if running {
                    "ok"
                } else {
                    "check"
                },
                "activity",
                "Suricata",
                &if on && running { t!("Running") } else { t!("Not running") },
            ),
            Self::strip_cell(
                if !on {
                    "info"
                } else if rules.is_some() && !stale {
                    "ok"
                } else {
                    "check"
                },
                "database",
                &t!("Rules"),
                &rules_text(rules),
            ),
            Self::strip_cell("accent", "globe", &t!("Blocked this week"), &i18n::number(week)),
        ])
    }

    /// Leaves the rule out on this machine, after an administrator's password. Declining it changes nothing.
    fn allow_button(&self, event: &NetworkEvent) -> gtk::Button {
        let rule = event.sid.clone();
        let title = t!("Leave rule {rule} out on this machine. Asks for your password.", rule = event.sid.as_str());
        self.button_with(&t!("Allow"), Opts { small: true, title: Some(title), ..Default::default() }, move |this, button| {
            button.set_sensitive(false);
            if let Some(words) = ui::button_words(button) {
                words.set_label(&t!("Allowing…"));
            }
            let (this, rule) = (this.clone(), rule.clone());
            glib::spawn_future_local(async move {
                let outcome = backend::allow_rule(&rule).await;
                this.state.borrow_mut().notice = match outcome {
                    backend::RuleOutcome::Error(error) => error,
                    _ => String::new(),
                };
                let network = backend::network().await;
                this.state.borrow_mut().network = Some(network);
                this.render();
            });
        })
    }

    fn network_row(&self, event: &NetworkEvent, allowed: &[f64]) -> gtk::Box {
        let blocked = event.action == "blocked";
        let is_allowed = event.sid_number.is_some_and(|sid| allowed.contains(&sid));
        // A host's name runs whichever way its own letters do, and an address left to right
        let peer = if !event.host.is_empty() {
            isolate(&event.host)
        } else {
            t!("{source} and {destination}", source = ltr(&event.src), destination = ltr(&event.dest))
        };
        let meta = dotted(&[
            when_first(event.time),
            if event.port.is_empty() { peer } else { t!("{host}, port {port}", host = peer, port = event.port.as_str()) },
            if event.count > 1.0 { t!("{count, plural, one {# time} other {# times}}", count = event.count) } else { String::new() },
            t!("rule {rule}", rule = event.sid.as_str()),
        ]);
        let row = ui::hbox(14, &["list-row"]);
        row.append(&status(if is_allowed {
            "info"
        } else if blocked {
            "off"
        } else {
            "check"
        }));
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        // A rule's name reads left to right in any language
        let name = ellipsized(&event.signature, &["strong"]);
        name.set_direction(gtk::TextDirection::Ltr);
        name.set_tooltip_text(Some(&event.signature));
        words.append(&name);
        words.append(&wrapping(&meta, &["small-text"]));
        row.append(&words);
        let tone = if is_allowed {
            ""
        } else if blocked {
            "off"
        } else {
            "check"
        };
        let words = if is_allowed {
            t!("Allowed")
        } else if blocked {
            t!("Blocked")
        } else {
            t!("Noticed")
        };
        row.append(&pill(&words, tone, true, false));
        if !is_allowed {
            let allow = self.allow_button(event);
            allow.set_valign(gtk::Align::Center);
            row.append(&allow);
        }
        row
    }

    fn network_list(&self, network: Option<&Network>) -> gtk::Box {
        let Some(network) = network else { return empty(&t!("Reading…")) };
        // network.json names the sites this machine talked to, so it is wheel's to read
        if !network.readable {
            return empty(&t!(
                "Only administrators can see what it blocked. {command} shows it after asking for your password.",
                command = "ame security connections"
            ));
        }
        if network.events.is_empty() {
            return empty(&t!("Nothing blocked or noticed yet."));
        }
        list_card(network.events.iter().take(50).map(|event| self.network_row(event, &network.allowed).upcast()).collect())
    }

    fn network_page(&self) -> gtk::Widget {
        let mode = self.network_mode();
        let (network, check) = {
            let state = self.state.borrow();
            (state.network.clone(), state.report.as_ref().and_then(|r| r.checks.iter().find(|c| c.id == "network").cloned()))
        };
        // Something wrong with it comes with its own way into the manual
        let attention = check.as_ref().is_some_and(needs_attention);
        let lead = match mode.as_str() {
            "watch" => t!("Suricata inspects every connection this machine makes and receives, and says what it recognises. Nothing is blocked yet."),
            "block" => t!("Suricata inspects every connection this machine makes and receives, and cuts off the ones to known malware, command servers and exploits."),
            _ => t!(
                "Suricata can inspect every connection this machine makes and receives, and cut off the ones to known malware, command servers and exploits. It is off."
            ),
        };
        let mut children: Vec<gtk::Widget> = vec![page_head(&t!("Network protection"), &lead).upcast()];
        children.extend(self.notice_widget());
        children.push(self.network_strip(&mode, network.as_ref()).upcast());
        if mode == "off" {
            let turn = self.button(&t!("Turn it on…"), Opts { icon: Some("terminal"), primary: true, ..Default::default() }, |this| {
                backend::run_check(this.state.borrow().report.as_ref(), "network-protection")
            });
            children.push(
                callout(
                    "info",
                    &t!("Start by watching"),
                    &t!("Watching blocks nothing, and a week of it shows what blocking would get in the way of. Whatever goes wrong with network protection, connections go on uninspected rather than stop."),
                    Some(actions(vec![turn.upcast()]).upcast()),
                )
                .upcast(),
            );
        }
        if let (true, Some(check)) = (attention, &check) {
            children.push(self.attention_card(check).upcast());
        }
        let note = network.as_ref().filter(|n| !n.events.is_empty()).map(|_| t!("Allowing a rule asks for your password"));
        children.push(section(&t!("Blocked and noticed"), &self.network_list(network.as_ref()), note.as_deref()).upcast());
        let mut buttons: Vec<gtk::Widget> = Vec::new();
        if mode != "off" {
            buttons.push(
                self.button(&t!("Change…"), Opts { icon: Some("terminal"), ..Default::default() }, |this| {
                    backend::run_check(this.state.borrow().report.as_ref(), "network-protection")
                })
                .upcast(),
            );
        }
        if !attention {
            buttons.push(self.manual_link(&t!("Network protection in the manual"), "security#network-protection").upcast());
        }
        children.push(actions(buttons).upcast());
        Self::page(children)
    }

    // --- App permissions -------------------------------------------------------------------------------------------

    /// What came of a change to one app: the list read again, or why it did not happen
    fn after_change(self: &Rc<Self>, outcome: Result<(), String>) {
        let this = self.clone();
        glib::spawn_future_local(async move {
            match outcome {
                Ok(()) => {
                    let permissions = this.reread_permissions().await;
                    let mut state = this.state.borrow_mut();
                    state.notice = permissions.error.clone().unwrap_or_default();
                    if permissions.error.is_none() {
                        state.permissions = Some(permissions);
                    }
                }
                Err(error) => this.state.borrow_mut().notice = error,
            }
            this.render();
        });
    }

    /// One app's network, taken away or given back as an override of this account's
    fn app_network_row(&self, app: &backend::App) -> gtk::Box {
        let row = ui::hbox(14, &["list-row", "container-row"]);
        let mark = status(if app.network_blocked { "ok" } else { "info" });
        mark.set_valign(gtk::Align::Start);
        row.append(&mark);
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.append(&ellipsized(&app.name, &["strong"]));
        let id = wrapping(&app.app, &["small-text"]);
        id.set_direction(gtk::TextDirection::Ltr);
        words.append(&id);
        if app.network_blocked {
            words.append(&wrapping(&t!("Its network is blocked here"), &["small-text"]));
        }
        row.append(&words);
        let allow = app.network_blocked;
        let label = if allow { t!("Give its network back") } else { t!("Block its network") };
        let id = app.app.clone();
        let button = self.button_with(&label, Opts { small: true, ..Default::default() }, move |this, button| {
            button.set_sensitive(false);
            let (this, id) = (this.clone(), id.clone());
            glib::spawn_future_local(async move {
                let list = this.permission_apps();
                let outcome = backend::set_network(&list, &id, allow).await;
                this.after_change(outcome);
            });
        });
        button.set_valign(gtk::Align::Start);
        row.append(&button);
        row
    }

    fn app_row(&self, app: &backend::App) -> gtk::Box {
        let tone = if leaves(app) {
            "off"
        } else if types(app) {
            "check"
        } else {
            "info"
        };
        let row = ui::hbox(14, &["list-row", "container-row"]);
        let mark = status(tone);
        mark.set_valign(gtk::Align::Start);
        row.append(&mark);
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.append(&ellipsized(&app.name, &["strong"]));
        let id = wrapping(&app.app, &["small-text"]);
        id.set_direction(gtk::TextDirection::Ltr);
        words.append(&id);
        for reach in &app.reaches {
            let line = ui::wrap(8, 8, &["small-text", "container-line", "reach-line"]);
            line.append(&wrapping(&reach.text, &["reach", reach_tone(&reach.id)]));
            if reach.granted {
                line.append(&pill(&t!("Granted here"), "", true, false));
            }
            words.append(&line);
        }
        row.append(&words);
        if app.user_overrides {
            let id = app.app.clone();
            let title = t!("Remove every permission this account granted this app, in Flatseal or with {command}", command = "flatpak override");
            let reset = self.button_with(&t!("Take back what you granted"), Opts { small: true, title: Some(title), ..Default::default() }, move |this, button| {
                button.set_sensitive(false);
                let (this, id) = (this.clone(), id.clone());
                glib::spawn_future_local(async move {
                    let list = this.permission_apps();
                    let outcome = backend::reset_permissions(&list, &id).await;
                    this.after_change(outcome);
                });
            });
            reset.set_valign(gtk::Align::Start);
            row.append(&reset);
        }
        row
    }

    /// What every Flatpak app can reach; with `focus`, the app the app grid's What can it reach? asked about comes first
    fn apps_page(&self, focus: Option<&str>) -> gtk::Widget {
        let data = self.state.borrow().permissions.clone();
        let apps = data.as_ref().map(|d| d.apps.clone()).unwrap_or_default();
        let collator = i18n::with(|i18n| i18n.collator());
        let mut reaching: Vec<&backend::App> = apps.iter().filter(|app| !app.reaches.is_empty()).collect();
        reaching.sort_by(|a, b| {
            weight(a).cmp(&weight(b)).then_with(|| match &collator {
                Some(collator) => collator.compare(&a.name, &b.name),
                None => a.name.cmp(&b.name),
            })
        });
        let leaving: Vec<&&backend::App> = reaching.iter().filter(|app| leaves(app)).collect();
        let list = match &data {
            None => empty(&t!("Reading…")),
            Some(data) if data.error.is_some() => empty(data.error.as_deref().unwrap_or("")),
            Some(_) if apps.is_empty() => empty(&t!("No Flatpak apps are installed.")),
            Some(_) if reaching.is_empty() => empty(&t!("No app reaches anything beyond its sandbox without asking.")),
            Some(_) => {
                let me = self.me.clone();
                let values: Vec<backend::App> = reaching.iter().map(|app| (*app).clone()).collect();
                list_card_filling(values, move |app| Some(me.upgrade()?.app_row(app).upcast()))
            }
        };
        let mut children: Vec<gtk::Widget> = vec![page_head(
            &t!("Apps"),
            &t!("What each Flatpak app can reach beyond its sandbox without asking you: what it asked for itself, and what was granted on this machine. What an app asks for through a portal, such as a file you choose for it, is asked each time and is not listed."),
        )
        .upcast()];
        children.extend(self.notice_widget());
        if let Some(id) = focus {
            let found = apps.iter().find(|app| app.app == id);
            let title = found.map(|app| app.name.clone()).filter(|name| !name.is_empty()).unwrap_or_else(|| id.to_string());
            let shown = match (&data, found) {
                (None, _) => empty(&t!("Reading…")),
                (Some(data), _) if data.error.is_some() => empty(data.error.as_deref().unwrap_or("")),
                (Some(_), None) => empty(&t!("No Flatpak app with that ID is installed.")),
                (Some(_), Some(app)) => {
                    let mut rows: Vec<gtk::Widget> = Vec::new();
                    if app.reaches.is_empty() {
                        rows.push(empty(&t!("It reaches nothing beyond its sandbox without asking.")).upcast());
                    } else {
                        rows.push(self.app_row(app).upcast());
                    }
                    if app.network || app.network_blocked {
                        rows.push(self.app_network_row(app).upcast());
                    }
                    list_card(rows)
                }
            };
            children.push(section(&title, &shown, None).upcast());
        }
        if !leaving.is_empty() {
            let title = if leaving.len() == 1 {
                t!("{name} can leave its sandbox", name = isolate(&leaving[0].name))
            } else {
                t!("{count, plural, one {# app can leave its sandbox} other {# apps can leave their sandbox}}", count = leaving.len())
            };
            children.push(
                callout(
                    "check",
                    &title,
                    &t!("Whatever runs in it can do anything you can. If you did not mean it to, change it in Flatseal, or take back what you granted."),
                    None,
                )
                .upcast(),
            );
        }
        let checked = (!apps.is_empty()).then(|| t!("{count, plural, one {# app checked} other {# apps checked}}", count = apps.len()));
        children.push(section(&t!("What apps can reach"), &list, checked.as_deref()).upcast());
        let networked: Vec<&backend::App> = apps.iter().filter(|app| app.network || app.network_blocked).collect();
        if !networked.is_empty() {
            children.push(
                section(
                    &t!("Network"),
                    &list_card(networked.iter().map(|app| self.app_network_row(app).upcast()).collect()),
                    Some(&t!("An app without the network cannot send anything it reads. From its next start.")),
                )
                .upcast(),
            );
        }
        children.push(
            actions(vec![
                self.button(&t!("Change permissions in Flatseal"), Opts { icon: Some("arrow-right"), after: true, ..Default::default() }, |_| {
                    process::launch("flatpak", &["run", backend::FLATSEAL])
                })
                .upcast(),
                self.manual_link(&t!("App permissions in the manual"), "security#app-permissions").upcast(),
            ])
            .upcast(),
        );
        Self::page(children)
    }

    // --- Installed outside Flatpak -------------------------------------------------------------------------------------

    fn container_row(&self, container: &Container) -> gtk::Box {
        let tone = if stale(container) || container.last_upgrade_result == "failed" { "check" } else { "ok" };
        let mut lines: Vec<String> = Vec::new();
        if !container.exported_apps.is_empty() {
            let apps: Vec<String> = container.exported_apps.iter().map(|name| app_name(name)).collect();
            lines.push(t!("In the app grid: {apps}", apps = names(&apps)));
        }
        let from_aur: Vec<String> = container.packages.iter().filter(|name| container.aur_packages.contains(name)).cloned().collect();
        let by_name: Vec<String> = container.packages.iter().filter(|name| !from_aur.contains(name)).cloned().collect();
        if !by_name.is_empty() {
            lines.push(t!("Installed by name: {packages}", packages = names(&by_name)));
        }
        if !from_aur.is_empty() {
            lines.push(if container.aur_pending != 0.0 {
                t!(
                    "{count, plural, one {From the AUR: {packages}, # update waits for you to review} other {From the AUR: {packages}, # updates wait for you to review}}",
                    packages = names(&from_aur),
                    count = container.aur_pending
                )
            } else {
                t!("From the AUR: {packages}", packages = names(&from_aur))
            });
        }
        if !container.files.is_empty() {
            lines.push(t!("Installed from a file: {files}", files = names(&container.files)));
        }
        if !container.exported_bins.is_empty() {
            let commands: Vec<String> = container.exported_bins.iter().map(|file| basename(file)).collect();
            lines.push(t!("Commands: {commands}", commands = names(&commands)));
        }
        let row = ui::hbox(14, &["list-row", "container-row"]);
        let mark = status(tone);
        mark.set_valign(gtk::Align::Start);
        row.append(&mark);
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.append(&ellipsized(&container.name, &["strong"]));
        let mut about = vec![t!("From the template {template}", template = container.template.as_str()), upgraded_text(container)];
        if container.home {
            about.push(t!("a home folder of its own"));
        }
        words.append(&wrapping(&about.join(" · "), &["small-text"]));
        for line in &lines {
            words.append(&wrapping(line, &["small-text", "container-line"]));
        }
        row.append(&words);
        let state = pill(&if container.status == "running" { t!("Running") } else { t!("Stopped") }, "", true, false);
        state.set_valign(gtk::Align::Start);
        row.append(&state);
        row
    }

    fn containers_page(&self) -> gtk::Widget {
        let inventory = self.state.borrow().inventory.clone();
        let containers = inventory.as_ref().map(|i| i.containers.clone()).unwrap_or_default();
        let behind: Vec<&Container> = containers.iter().filter(|c| stale(c)).collect();
        let list = match &inventory {
            None => empty(&t!("Reading…")),
            Some(inventory) if containers.is_empty() => {
                empty(&inventory.error.clone().unwrap_or_else(|| t!("None. Software from another distribution goes into a container with amepkg.")))
            }
            Some(_) => list_card(containers.iter().map(|c| self.container_row(c).upcast()).collect()),
        };
        let mut children: Vec<gtk::Widget> = vec![page_head(
            &t!("Containers"),
            &t!("What is installed outside Flatpak: software from other distributions, each in a container amepkg made, upgraded every day. Containers are not a sandbox: what runs in one runs as you, with your home folder, your display, your sound and your session bus."),
        )
        .upcast()];
        if !behind.is_empty() {
            let title = if behind.len() == 1 {
                t!("{name} has not been upgraded for two weeks", name = isolate(&behind[0].name))
            } else {
                t!(
                    "{count, plural, one {# container has not been upgraded for two weeks} other {# containers have not been upgraded for two weeks}}",
                    count = behind.len()
                )
            };
            children.push(callout("check", &title, &t!("What runs in a container goes without the fixes its distribution has published since."), None).upcast());
        }
        if inventory.as_ref().is_some_and(|i| i.aur) {
            children.push(
                callout(
                    "info",
                    &t!("The AUR is on"),
                    &t!(
                        "Packages from it are built from scripts nobody reviews, and are never upgraded unattended: each waits for you to read what changed. {command} turns it off.",
                        command = "ame apps aur"
                    ),
                    None,
                )
                .upcast(),
            );
        }
        children.push(section(&t!("Installed outside Flatpak"), &list, None).upcast());
        let mut buttons: Vec<gtk::Widget> = Vec::new();
        if !containers.is_empty() {
            // Upgrading asks the containers' package managers, whose output is worth reading, so in a terminal
            buttons.push(
                self.button(&t!("Upgrade them now"), Opts { icon: Some("terminal"), primary: !behind.is_empty(), ..Default::default() }, |_| {
                    process::in_terminal(&[backend::PKG, "upgrade-all"])
                })
                .upcast(),
            );
        }
        buttons.push(self.manual_link(&t!("Containers in the manual"), "software#packages-from-other-distributions").upcast());
        children.push(actions(buttons).upcast());
        Self::page(children)
    }

    // --- Events: what the security watcher found -----------------------------------------------------------------------

    fn event_row(&self, event: &Event, triage: bool) -> gtk::Box {
        // The watcher made the sentence plain text already, and it is only ever shown as text
        let mut facts = vec![time(event.time), short_day(event.time)];
        if !event.account.is_empty() && event.account != "-" {
            facts.push(event.account.clone());
        }
        if !event.key.is_empty() {
            facts.push(event.key.clone());
        }
        if !event.tactic.is_empty() && event.tactic != "-" {
            facts.push(event.tactic.clone());
        }
        // What the event can lead to, each a button whose action looks the event up again by its id
        let mut buttons: Vec<gtk::Button> = Vec::new();
        if event.at && event.mine {
            let id = event.id.clone();
            buttons.push(self.button(
                &t!("Open in Backups"),
                Opts {
                    small: true,
                    primary: event.level == "critical",
                    title: Some(t!("Your home folder as the snapshot from before it has it, to put files back")),
                    ..Default::default()
                },
                move |_| backend::open_backups(&id),
            ));
        }
        if event.allow {
            let id = event.id.clone();
            buttons.push(self.button(
                &t!("Allow…"),
                Opts { small: true, icon: Some("terminal"), title: Some(t!("Stop saying this. Asks first, in a terminal, and then for your password.")), ..Default::default() },
                move |_| {
                    let id = id.clone();
                    glib::spawn_future_local(async move { backend::allow_event(&id).await });
                },
            ));
        }
        if triage && event.level != "quiet" {
            let id = event.id.clone();
            buttons.push(self.button(
                &t!("Ask the agent"),
                Opts { small: true, icon: Some("sparkle"), title: Some(t!("The AI agent explains what this most likely is, in a terminal, and changes nothing")), ..Default::default() },
                move |_| {
                    let id = id.clone();
                    glib::spawn_future_local(async move { backend::triage(&id).await });
                },
            ));
        }
        let row = ui::hbox(14, &["list-row", "container-row"]);
        let mark = status(match event.level.as_str() {
            "critical" => "off",
            "normal" => "check",
            _ => "info",
        });
        mark.set_valign(gtk::Align::Start);
        row.append(&mark);
        let words = ui::vbox(1, &["grow"]);
        words.set_hexpand(true);
        words.append(&wrapping(&event.text, &["strong", "event-text"]));
        let facts = wrapping(&facts.join(" · "), &["small-text"]);
        facts.set_direction(gtk::TextDirection::Ltr);
        words.append(&facts);
        row.append(&words);
        if !buttons.is_empty() {
            let side = ui::hbox(6, &["event-actions"]);
            side.set_valign(gtk::Align::Start);
            for button in buttons {
                side.append(&button);
            }
            row.append(&side);
        }
        row
    }

    fn events_page(&self) -> gtk::Widget {
        let data = self.state.borrow().events.clone();
        // Each row is a good many widgets: the 500 the watcher keeps, drawn at once, stop the window for a second
        let shown = self.state.borrow().events_shown;
        let list = match &data {
            None => empty(&t!("Reading…")),
            Some(data) if !data.readable => empty(&t!(
                "Only administrators can see what the security watcher found. {command} shows it after asking for your password.",
                command = "ame security events"
            )),
            Some(data) if data.events.is_empty() => empty(&t!("Nothing yet. Whatever the watcher finds from now on is listed here.")),
            Some(data) => {
                let me = self.me.clone();
                let triage = data.triage;
                let values: Vec<Event> = data.events.iter().take(shown).cloned().collect();
                list_card_filling(values, move |event| Some(me.upgrade()?.event_row(event, triage).upcast()))
            }
        };
        let count = data.as_ref().filter(|d| !d.events.is_empty()).map(|d| t!("{count, plural, one {# event} other {# events}}", count = d.events.len()));
        let mut children: Vec<gtk::Widget> = vec![
            page_head(
                &t!("Events"),
                &t!("What the security watcher found, newest first: changes to accounts, to who may become root, to what starts by itself and to the security settings, programs that read the keyboard or listen on the network, and checks that turned bad. Each says which account it is about and which part of the watcher found it. If you made the change, or installed something that did, there is nothing to do."),
            )
            .upcast(),
            section(&t!("Found"), &list, count.as_deref()).upcast(),
        ];
        if data.as_ref().is_some_and(|data| data.events.len() > shown) {
            children.push(actions(vec![self.button(&t!("Show more"), Opts::default(), |this| {
                this.state.borrow_mut().events_shown += crate::EVENTS_AT_ONCE;
                this.render();
            })
            .upcast()])
            .upcast());
        }
        children.push(
            actions(vec![
                // Seeing them in a terminal marks them read, after the administrator's password
                self.button(&t!("Mark them read, in a terminal"), Opts { icon: Some("terminal"), ..Default::default() }, |_| {
                    process::in_terminal(&[backend::AME, "security", "events"])
                })
                .upcast(),
                self.manual_link(&t!("The security watcher in the manual"), "security#the-security-watcher").upcast(),
            ])
            .upcast(),
        );
        Self::page(children)
    }

    // --- Settings: every switch -----------------------------------------------------------------------------------------

    fn switch_row(&self, entry: &Setting) -> gtk::Box {
        let cost = cost_text(entry.cost.as_ref());
        let row = ui::hbox(18, &["list-row", "switch-row"]);
        let words = ui::vbox(3, &["grow"]);
        words.set_hexpand(true);
        words.append(&wrapping(&entry.title, &["strong"]));
        words.append(&wrapping(&entry.text, &["small-text"]));
        // What it sends to somebody else, said here as well as before it is turned on
        if let Some(explain) = &entry.explain {
            words.append(&wrapping(explain, &["small-text", "faint"]));
        }
        if !cost.is_empty() {
            words.append(&wrapping(&cost, &["small-text", "faint"]));
        }
        row.append(&words);
        let values: Vec<(String, String)> = entry.allowed.iter().map(|value| (value.clone(), value_words(value))).collect();
        let key = entry.key.clone();
        let group = self.segmented(&entry.title, &values, &entry.value, move |this, value| {
            backend::change(this.state.borrow().switches.as_ref(), Kind::Setting, &key, value, "")
        });
        group.set_valign(gtk::Align::Start);
        row.append(&group);
        row
    }

    /// A prevention switch: on or off, or one of its levels (kernel: off, standard, strict)
    fn enforced_row(&self, entry: &Hardening) -> gtk::Box {
        let on = entry.state == "on";
        let row = ui::hbox(18, &["list-row", "switch-row"]);
        let words = ui::vbox(3, &["grow"]);
        words.set_hexpand(true);
        words.append(&wrapping(&entry.title, &["strong"]));
        words.append(&wrapping(&entry.text, &["small-text"]));
        for item in entry.arguments.iter().flatten() {
            let line = ui::wrap(8, 8, &["small-text", "container-line"]);
            let argument = ui::line(&item.argument, &["mono"]);
            argument.set_direction(gtk::TextDirection::Ltr);
            argument.set_valign(gtk::Align::Center);
            line.append(&argument);
            let item_on = item.state == "on";
            line.append(&pill(&if item_on { t!("On") } else { t!("Off") }, if item_on { "ok" } else { "" }, true, false));
            if !entry.fixed {
                let (name, argument) = (entry.name.clone(), item.argument.clone());
                let link = self.link_button(&if item_on { t!("Turn off…") } else { t!("Turn on…") }, None, false, true, move |this| {
                    backend::change(this.state.borrow().switches.as_ref(), Kind::Hardening, &name, if item_on { "off" } else { "on" }, &argument)
                });
                link.set_valign(gtk::Align::Center);
                line.append(&link);
            }
            words.append(&line);
        }
        row.append(&words);
        if let Some(levels) = &entry.levels {
            let values: Vec<(String, String)> = levels.iter().map(|value| (value.clone(), value_words(value))).collect();
            let name = entry.name.clone();
            let group = self.segmented(&entry.title, &values, &entry.level, move |this, value| {
                backend::change(this.state.borrow().switches.as_ref(), Kind::Hardening, &name, value, "")
            });
            group.set_valign(gtk::Align::Start);
            row.append(&group);
        } else if entry.arguments.is_none() {
            let side = ui::hbox(10, &["switch-side"]);
            side.set_valign(gtk::Align::Start);
            side.append(&pill(&if on { t!("On") } else { t!("Off") }, if on { "ok" } else { "" }, false, false));
            let name = entry.name.clone();
            let button = self.button(&if on { t!("Turn off…") } else { t!("Turn on…") }, Opts { small: true, ..Default::default() }, move |this| {
                backend::change(this.state.borrow().switches.as_ref(), Kind::Hardening, &name, if on { "off" } else { "on" }, "")
            });
            button.set_valign(gtk::Align::Center);
            side.append(&button);
            row.append(&side);
        }
        row
    }

    /// A category's switches: what watches and records, which a profile sets, then what the image enforces, then the
    /// stricter ones, each under its own heading when there are more kinds than one
    fn category_sections(&self, switches: &Switches, group: &str) -> Vec<gtk::Widget> {
        let watching: Vec<gtk::Widget> = switches.settings.iter().filter(|e| e.group == group).map(|e| self.switch_row(e).upcast()).collect();
        let enforced: Vec<gtk::Widget> =
            switches.hardening.iter().filter(|e| e.group == group && e.default != "off").map(|e| self.enforced_row(e).upcast()).collect();
        let stricter: Vec<gtk::Widget> =
            switches.hardening.iter().filter(|e| e.group == group && e.default == "off").map(|e| self.enforced_row(e).upcast()).collect();
        let kinds = [&watching, &enforced, &stricter].iter().filter(|rows| !rows.is_empty()).count();
        if kinds < 2 {
            return vec![list_card(watching.into_iter().chain(enforced).chain(stricter).collect()).upcast()];
        }
        let mut sections: Vec<gtk::Widget> = Vec::new();
        if !watching.is_empty() {
            sections.push(section(&t!("What watches and records"), &list_card(watching), Some(&t!("A profile sets these"))).upcast());
        }
        if !enforced.is_empty() {
            sections.push(section(&t!("What the image enforces"), &list_card(enforced), Some(&t!("No profile changes these"))).upcast());
        }
        if !stricter.is_empty() {
            sections.push(section(&t!("Stricter, off unless you turn them on"), &list_card(stricter), Some(&t!("Each costs something a desktop may need"))).upcast());
        }
        sections
    }

    /// A tile on the Settings page that opens a page of its own
    fn category_tile(&self, icon: &str, title: &str, text: &str, state: &str, page: String) -> gtk::Widget {
        let tile = ui::vbox(12, &["tile", "card"]);
        let square = tinted(&["tile-icon", "tone", "accent"], icons::icon(icon, 20));
        square.set_halign(gtk::Align::Start);
        tile.append(&square);
        let words = ui::vbox(4, &["tile-text"]);
        words.set_vexpand(true);
        words.append(&wrapping(title, &["h3"]));
        words.append(&wrapping(text, &["p"]));
        tile.append(&words);
        let foot = ui::hbox(8, &["tile-foot"]);
        let state = wrapping(state, &["tile-state"]);
        state.set_hexpand(true);
        state.set_valign(gtk::Align::Center);
        foot.append(&state);
        let button = self.button(&t!("Open"), Opts { small: true, ..Default::default() }, move |this| this.show(&page, None));
        button.update_property(&[gtk::accessible::Property::Label(&t!("Open {category}", category = title))]);
        button.set_valign(gtk::Align::Center);
        foot.append(&button);
        tile.append(&foot);
        tile.upcast()
    }

    fn settings_page(&self) -> gtk::Widget {
        let switches = self.state.borrow().switches.clone();
        let mut sections: Vec<gtk::Widget> = Vec::new();
        let mut tiles: Vec<gtk::Widget> = Vec::new();
        match &switches {
            None => sections.push(empty(&t!("Reading…")).upcast()),
            Some(switches) if switches.error.is_some() && switches.settings.is_empty() => sections.push(empty(switches.error.as_deref().unwrap_or("")).upcast()),
            Some(switches) => {
                for (group, icon, title, text) in categories() {
                    let (on, count) = on_count(switches, group);
                    if count > 0 {
                        let state = t!("{on} of {count} on", on = i18n::number(on as f64), count = i18n::number(count as f64));
                        tiles.push(self.category_tile(icon, &title, &text, &state, format!("settings/{group}")));
                    }
                }
            }
        }
        // Says nothing of whether this account has one, as the report does not
        tiles.push(self.category_tile(
            "fingerprint",
            &t!("Under duress"),
            &t!("A finger or a password that unlocks this machine as usual when someone makes you, and runs an action you chose."),
            "",
            "settings/duress".into(),
        ));
        sections.push(section(&t!("By what it is about"), &self.grid(tiles, "tiles", true), None).upcast());
        let profile = switches.as_ref().map(|s| s.profile.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| "custom".into());
        let profiles: Vec<(String, String)> = vec![("light".into(), t!("Light")), ("balanced".into(), t!("Balanced")), ("thorough".into(), t!("Thorough"))];
        let choose = self.segmented(&t!("Profile"), &profiles, &profile, |this, name| {
            backend::change(this.state.borrow().switches.as_ref(), Kind::Profile, name, "", "")
        });
        let off = self.button(&t!("Turn everything off…"), Opts { icon: Some("terminal"), disabled: profile == "off", ..Default::default() }, |this| {
            backend::change(this.state.borrow().switches.as_ref(), Kind::Profile, "off", "", "")
        });
        let defaults = self.button(&t!("Back to the defaults…"), Opts { icon: Some("terminal"), disabled: profile == "default", ..Default::default() }, |this| {
            backend::change(this.state.borrow().switches.as_ref(), Kind::Profile, "default", "", "")
        });
        let mut children: Vec<gtk::Widget> = vec![
            page_head(
                &t!("Settings"),
                &t!("Every security feature is a switch, in case one makes this machine slower or gets in your way. Off costs nothing: nothing runs, nothing is downloaded and nothing is said, and the audit log follows at the next restart. A change opens in a terminal, which says what it changes before it does it."),
            )
            .upcast(),
            callout(
                "info",
                &t!(
                    "{profile, select, off {Everything that can be off is off} default {Every switch is as the image ships it} light {The light profile: the cheapest ways to catch new threats} balanced {The balanced profile} thorough {The thorough profile: everything that looks for new threats} other {Some switches are changed from how the image ships them}}",
                    profile = profile.as_str()
                ),
                &t!("A profile sets what looks for new threats in one step: light records and says the cheapest, balanced adds secrets read and the chains of steps, thorough turns everything on. None of them turns on what sends something to somebody else, and none touches what the image enforces from the start, below, or what you turned on yourself in the overview."),
                Some(actions(vec![choose.upcast(), off.upcast(), defaults.upcast()]).upcast()),
            )
            .upcast(),
        ];
        children.extend(sections);
        children.push(
            wrapping(
                &t!("Edited by hand, {file} takes effect at the next restart, even from a text console when the desktop does not start.", file = "/etc/amethystora/security.conf"),
                &["faint", "small-type"],
            )
            .upcast(),
        );
        children.push(actions(vec![self.manual_link(&t!("Switches in the manual"), "security#switches").upcast()]).upcast());
        Self::page(children)
    }

    fn settings_back(&self) -> gtk::Widget {
        self.link_button(&t!("Settings"), Some("arrow-left"), true, false, |this| this.show("settings", None)).upcast()
    }

    /// One category's switches, a page of their own
    fn category_page(&self, group: &str) -> gtk::Widget {
        let Some((_, _, title, text)) = categories().into_iter().find(|(id, ..)| *id == group) else { return self.settings_page() };
        let switches = self.state.borrow().switches.clone();
        let mut children: Vec<gtk::Widget> = vec![self.settings_back(), page_head(&title, &text).upcast()];
        match &switches {
            None => children.push(empty(&t!("Reading…")).upcast()),
            Some(switches) if switches.error.is_some() && switches.settings.is_empty() => children.push(empty(switches.error.as_deref().unwrap_or("")).upcast()),
            Some(switches) => children.extend(self.category_sections(switches, group)),
        }
        children.push(actions(vec![self.manual_link(&t!("Switches in the manual"), "security#switches").upcast()]).upcast());
        Self::page(children)
    }

    /// A step of `ame security duress`, in a terminal
    fn duress_row(&self, title: &str, text: &str, label: &str, step: &'static str) -> gtk::Widget {
        let row = ui::hbox(18, &["list-row", "switch-row"]);
        let words = ui::vbox(3, &["grow"]);
        words.set_hexpand(true);
        words.append(&wrapping(title, &["strong"]));
        words.append(&wrapping(text, &["small-text"]));
        row.append(&words);
        let button = self.button(label, Opts { icon: Some("terminal"), small: true, ..Default::default() }, move |_| backend::duress(step));
        button.update_property(&[gtk::accessible::Property::Label(title)]);
        button.set_valign(gtk::Align::Start);
        row.append(&button);
        row.upcast()
    }

    /// The duress finger and password: every step in a terminal, and nothing read, so that the page looks the same
    /// whether this account has one or not
    fn duress_page(&self) -> gtk::Widget {
        let finger = list_card(vec![
            self.duress_row(
                &t!("Set up a duress finger"),
                &t!("Enrols one more finger on the reader, checks with a touch of each that the reader tells it from your others, and asks for its action. It needs an ordinary finger enrolled first, and a reader that compares a touch with every finger at once."),
                &t!("Set up…"),
                "setup",
            ),
            self.duress_row(&t!("Test my duress finger"), &t!("Touch the reader once: it says whether that was the duress finger, and never runs the action."), &t!("Test…"), "test"),
            self.duress_row(&t!("Change the duress finger's action"), &t!("Shows the new action in full before it is kept."), &t!("Change…"), "action-fingerprint"),
            self.duress_row(&t!("Remove my duress finger"), &t!("It stops running the action, and you choose whether it stays on the reader as an ordinary finger."), &t!("Remove…"), "remove"),
        ]);
        let password = list_card(vec![
            self.duress_row(
                &t!("Set up a duress password"),
                &t!("A second password, only yours, typed twice and kept only as a hash, then its action. It needs no fingerprint reader. Keep it different from your login password: the same one never runs the action."),
                &t!("Set up…"),
                "setup-password",
            ),
            self.duress_row(&t!("Change the duress password's action"), &t!("Shows the new action in full before it is kept."), &t!("Change…"), "action-password"),
            self.duress_row(&t!("Remove my duress password"), &t!("Your login password is not touched."), &t!("Remove…"), "remove-password"),
        ]);
        let everyone = list_card(vec![self.duress_row(
            &t!("Turn off for everyone"),
            &t!("Forgets every account's duress finger, password and both actions. The fingers stay, as ordinary ones."),
            &t!("Turn off…"),
            "off",
        )]);
        let children: Vec<gtk::Widget> = vec![
            self.settings_back(),
            page_head(
                &t!("Under duress"),
                &t!("A duress finger or password unlocks this machine at the login screen and the lock screen like your own, so nothing looks different. At sudo, administrator prompts, a text console and over SSH it is refused, as one that did not match. Either way the action you chose runs as root, once until the next start, and nothing about it reaches the journal. Each step opens in a terminal and asks for an administrator's password; this page shows nothing about whether you have one."),
            )
            .upcast(),
            callout(
                "info",
                &t!("What it cannot do"),
                &t!("It cannot stop anyone asking for your password instead, nor help against a copy of the disk made before, and anyone with root on this machine can read how it is set up. In many places, destroying data while compelled by the authorities is an offence of its own. What the action does is yours to decide: none of the examples destroys anything."),
                None,
            )
            .upcast(),
            section(&t!("Duress finger"), &finger, None).upcast(),
            section(&t!("Duress password"), &password, None).upcast(),
            section(&t!("Every account"), &everyone, None).upcast(),
            actions(vec![self.manual_link(&t!("Duress in the manual"), "security#a-duress-finger-or-password").upcast()]).upcast(),
        ];
        Self::page(children)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_paths() {
        assert_eq!(app_name("GIMP (on debian)"), "GIMP");
        assert_eq!(app_name("GIMP"), "GIMP");
        assert_eq!(app_name("A (on b) c"), "A (on b) c");
        assert_eq!(basename("/var/home/ana/x.exe"), "x.exe");
        assert_eq!(basename("/"), "/");
        assert_eq!(nav_of("scanning"), "scan");
        assert_eq!(recipe("containers"), "amepkg containers list");
        assert_eq!(manual_page("key-db"), "hardware#secure-boot");
        assert_eq!(manual_page("whatever"), "security");
        assert_eq!(mark("check"), "mark-alert");
    }

    #[test]
    fn opens_every_category_but_duress() {
        let ids: Vec<&str> = categories().iter().map(|(id, ..)| *id).collect();
        assert_eq!(ids, backend::CATEGORIES.to_vec());
        for id in ids {
            assert!(backend::openable(&format!("settings/{id}")));
        }
        assert!(backend::openable("settings"));
        assert!(!backend::openable("settings/duress"));
        assert!(!backend::openable("settings/"));
        assert!(!backend::openable("result"));
    }

    #[test]
    fn opens_the_apps_page_on_one_flatpak() {
        assert!(backend::openable("apps/org.gnome.TextEditor"));
        assert!(backend::openable("apps/com.example.My_App-2"));
        assert_eq!(nav_of("apps/org.gnome.TextEditor"), "apps");
        assert_eq!(recipe("apps/org.gnome.TextEditor"), "ame security apps");
        for wrong in ["apps/", "apps/firefox", "apps/-org.x.Y", "apps/org..x", "apps/org.x.Y/../z", "apps/org.x Y.z", "apps/1org.x.y"] {
            assert!(!backend::openable(wrong), "{wrong}");
        }
        assert!(!backend::openable(&format!("apps/org.{}.x", "a".repeat(300))));
    }

    #[test]
    fn finds_open_findings() {
        let entry = |id: &str, status: &str, place: &str| Entry { id: id.into(), status: status.into(), targets: vec![place.into()], ..Default::default() };
        // A later clean scan of the same place clears an older finding; the list is newest first
        let history = vec![entry("c", "clean", "/a"), entry("b", "found", "/a"), entry("a", "found", "/b")];
        assert_eq!(open_finding(&history).map(|e| e.id.as_str()), Some("a"));
        let mut dismissed = history.clone();
        dismissed[2].dismissed = true;
        assert!(open_finding(&dismissed).is_none());
        let check = Check { group: "updates".into(), state: "check".into(), command: Some("ame update".into()), details: vec!["a".into(), "Run:".into()], ..Default::default() };
        assert_eq!(explain(&check), (vec!["a".to_string()], "Run:".to_string()));
        assert!(needs_attention(&check));
    }

    #[test]
    fn says_how_long() {
        assert_eq!(took(42.0), "42 s");
        assert_eq!(took(300.0), "5 min");
        assert_eq!(took(4800.0), "1 h 20 min");
        assert_eq!(clock(125.0), "2:05");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(days_ago(-3.0), i18n::days(0));
    }
}
