//! Amethystora Help (round 7, plans/ui-ideas/r7/spec-help.md): two pages for when something is not as it should be.
//!
//! **Something's wrong** (R7-36): pick what is wrong in plain words. /usr/libexec/amethystora-wrong looks, and says what it
//! found in a sentence and one thing to press, shown with its `ame` command. Anything to do with security only opens
//! Security. **What changed** (R7-32): one list of what changed on this computer, worked out by
//! /usr/libexec/amethystora-changes when the page opens, each with Undo.
//!
//! Everything shown is what a helper said, kept in memory while the window is open and never written; the window passes
//! back only what the helper gave (backend.rs checks a command again before it runs). No agent, no forum, and a terminal
//! only for a command that asks something.

mod backend;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::frame::{self, NavButton};
use amethystora_ui::i18n::{self, DateStyle};
use amethystora_ui::icon::{Icon, IconData, Shape};
use amethystora_ui::{glib, gtk, process, t, ui};

use backend::{Change, Changes, Found, Topic};

pub fn main() -> glib::ExitCode {
    app::run(Spec { name: "help", id: "org.amethystora.Help", css: include_str!("help.css"), width: 980, height: 720, min_width: 620, min_height: 480 }, Help::new)
}

macro_rules! glyph {
    ($name:ident, $d:literal) => {
        static $name: IconData = IconData { view: 24.0, shapes: &[Shape { d: $d, fill: false, stroke: true, width: None }] };
    };
}
glyph!(ALERT, "M12 4 3 20h18L12 4ZM12 10v5M12 18v.5");
glyph!(HISTORY, "M3.5 12a8.5 8.5 0 1 0 2.5-6M3.5 4v4.5H8M12 7.5V12l3 2");
glyph!(BOOK, "M5 4.5h11a3 3 0 0 1 3 3V20H8a3 3 0 0 1-3-3V4.5ZM5 17a3 3 0 0 1 3-3h11");
glyph!(FILE, "M6.5 3.5H14l4.5 4.5v12.5h-12V3.5ZM14 3.5V8h4.5");
glyph!(APP, "M4.5 4.5h6v6h-6zM13.5 4.5h6v6h-6zM4.5 13.5h6v6h-6zM13.5 13.5h6v6h-6z");
glyph!(SETTING, "M4 7h9M17 7h3M4 17h3M11 17h9M15 5v4M9 15v4");
glyph!(UPDATE, "M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14");
glyph!(SHIELD, "M12 3.5 5 6v5.5c0 4.2 2.8 7.5 7 9 4.2-1.5 7-4.8 7-9V6l-7-2.5Z");
glyph!(AGENT, "M12 3.5 13.8 10 20.5 12l-6.7 2L12 20.5 10.2 14 3.5 12l6.7-2L12 3.5Z");
glyph!(SPINNER, "M12 4a8 8 0 1 1-8 8");

fn icon(data: &'static IconData, size: i32) -> Icon {
    Icon::new(data, size, 1.8)
}

/// What a look came to: being read, or read
#[derive(Clone)]
enum Look {
    Reading,
    Read(Option<Found>),
}

#[derive(Clone)]
enum List {
    None,
    Reading,
    Read(Option<Rc<Changes>>),
}

struct Page {
    page: &'static str,
    topics: Vec<Topic>,
    topic: String,
    look: Option<Look>,
    list: List,
    /// A sentence about what was just pressed, and whether it went wrong
    said: Option<(bool, String)>,
    /// What is being done, by what pressed it: the fix, or an item's id
    busy: String,
}

pub struct Help {
    me: Weak<Help>,
    shell: Rc<Shell>,
    state: RefCell<Page>,
    /// Something opened elsewhere (a terminal, Security): read again when the window has the focus back
    away: Cell<bool>,
    wrong_nav: NavButton,
    changed_nav: NavButton,
    content: gtk::Box,
}

impl App for Help {
    fn open(&self, args: &[String]) {
        let page = backend::page_argument(args).unwrap_or(self.state.borrow().page);
        self.show(page);
    }

    fn focus(&self) {
        if self.away.replace(false) {
            self.reload();
        }
    }

    fn reload(&self) {
        let (page, topic) = {
            let state = self.state.borrow();
            (state.page, state.topic.clone())
        };
        if page == "changed" {
            self.read_changes();
        } else if !topic.is_empty() {
            self.look_at(&topic);
        }
    }
}

impl Help {
    fn new(shell: Rc<Shell>) -> Rc<Help> {
        let brand = amethystora_ui::brand::brand(&t!("Help"), 21.0);
        brand.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Help"))]);
        let sidebar = frame::Sidebar::new(&brand, 18);
        let nav = frame::nav(3, None, &t!("Help"));
        let wrong_nav = NavButton::new(icon(&ALERT, 20), &t!("Something's wrong"), 12);
        let changed_nav = NavButton::new(icon(&HISTORY, 20), &t!("What changed"), 12);
        nav.append(&wrong_nav.button);
        nav.append(&changed_nav.button);
        sidebar.body.append(&nav);
        sidebar.foot.set_spacing(12);
        let manual = ui::button(&t!("In the manual"), Some(icon(&BOOK, 18).mirror_in_rtl()), &["link", "quiet"], || process::manual("troubleshooting#when-something-is-wrong"));
        manual.set_halign(gtk::Align::Start);
        sidebar.foot.append(&manual);
        sidebar.foot.append(&frame::terminal(&t!("Also in a terminal:"), "ame system doctor"));
        let column = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&sidebar.root).propagate_natural_width(true).build();

        let top = ui::hbox(0, &["topbar"]);
        let filler = ui::hbox(0, &[]);
        filler.set_hexpand(true);
        top.append(&filler);
        top.append(&frame::controls());
        let content = ui::vbox(16, &["page"]);
        let clamp = amethystora_ui::adw::Clamp::builder().maximum_size(760).tightening_threshold(760).child(&content).build();
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
        let main = ui::vbox(0, &["main"]);
        main.append(&top);
        main.append(&scroller);
        shell.window.set_title(Some(&t!("Amethystora Help")));
        shell.window.set_content(Some(&frame::with_sidebar(&column, &main, -1, false)));

        let this = Rc::new_cyclic(|me: &Weak<Help>| Help {
            me: me.clone(),
            shell: shell.clone(),
            state: RefCell::new(Page { page: "wrong", topics: Vec::new(), topic: String::new(), look: None, list: List::None, said: None, busy: String::new() }),
            away: Cell::new(false),
            wrong_nav,
            changed_nav,
            content,
        });
        for (button, page) in [(&this.wrong_nav.button, "wrong"), (&this.changed_nav.button, "changed")] {
            let weak = this.me.clone();
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.show(page);
                }
            });
        }
        let weak = this.me.clone();
        glib::spawn_future_local(async move {
            let topics = backend::topics().await;
            if let Some(this) = weak.upgrade() {
                this.state.borrow_mut().topics = topics;
                this.render();
            }
        });
        this
    }

    fn show(&self, page: &'static str) {
        let unread = {
            let mut state = self.state.borrow_mut();
            state.page = page;
            state.said = None;
            matches!(state.list, List::None)
        };
        self.shell.window.set_title(Some(&if page == "changed" { t!("What changed") } else { t!("Something's wrong") }));
        if page == "changed" && unread {
            self.read_changes();
        } else {
            self.render();
        }
    }

    // --- Something's wrong --------------------------------------------------------------------------------------------

    fn look_at(&self, topic: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.topic = topic.to_string();
            state.look = Some(Look::Reading);
            state.said = None;
        }
        self.render();
        let (weak, topic) = (self.me.clone(), topic.to_string());
        glib::spawn_future_local(async move {
            let found = backend::look(&topic).await;
            let Some(this) = weak.upgrade() else { return };
            // Another was picked meanwhile: that one's answer is the one to show
            if this.state.borrow().topic == topic {
                this.state.borrow_mut().look = Some(Look::Read(found));
                this.render();
            }
        });
    }

    fn press_fix(&self, fix: &backend::Fix) {
        if fix.terminal {
            self.away.set(true);
            backend::run_in_terminal(&fix.command);
            return;
        }
        self.state.borrow_mut().busy = "fix".into();
        self.render();
        let (weak, command) = (self.me.clone(), fix.command.clone());
        glib::spawn_future_local(async move {
            let (ok, last) = backend::run_fix(&command).await;
            let Some(this) = weak.upgrade() else { return };
            let topic = {
                let mut state = this.state.borrow_mut();
                state.busy.clear();
                state.topic.clone()
            };
            // What it finds now is the proof, so it looks again and says that under what was done
            this.look_at(&topic);
            let said = if ok { t!("Done. It looked again:") } else if last.is_empty() { t!("That did not work.") } else { last };
            this.state.borrow_mut().said = Some((!ok, said));
            this.render();
        });
    }

    fn wrong_page(&self) {
        let (topics, chosen, look, said, busy) = {
            let state = self.state.borrow();
            (state.topics.clone(), state.topic.clone(), state.look.clone(), state.said.clone(), !state.busy.is_empty())
        };
        self.content.append(&heading(&t!("Something's wrong"), "h1"));
        self.content.append(&ui::label(&t!("Pick what is wrong. This looks, says what it found, and offers one thing to press. Until you press, it only reads this computer."), &["soft"]));
        self.content.append(&ui::label(&t!("No internet is the one exception: unless NetworkManager's own check says the internet works, it sends your router one test message, and once that check has failed it looks up the one name the check uses, never while the check is off."), &["soft"]));
        let list = ui::vbox(0, &["card", "topics"]);
        list.set_overflow(gtk::Overflow::Hidden);
        for topic in topics {
            let id = topic.id.clone();
            let weak = self.me.clone();
            let row = ui::button(&topic.label, None, if id == chosen { &["plain", "topic", "current"] } else { &["plain", "topic"] }, move || {
                if let Some(this) = weak.upgrade() {
                    this.look_at(&id);
                }
            });
            list.append(&row);
        }
        self.content.append(&list);
        if let Some((failed, text)) = said {
            self.content.append(&ui::label(&text, if failed { &["said", "failed"] } else { &["said"] }));
        }
        match look {
            None => {}
            Some(Look::Reading) => self.content.append(&reading(&t!("Looking…"))),
            Some(Look::Read(None)) => self.content.append(&ui::label(&t!("That could not be looked at."), &["said", "failed"])),
            Some(Look::Read(Some(found))) => self.content.append(&self.found_card(&found, busy)),
        }
    }

    fn found_card(&self, found: &Found, busy: bool) -> gtk::Box {
        let card = ui::vbox(12, &["card", "found", if found.state == "fine" { "fine" } else { "problem" }]);
        // What a helper quotes (a device's name, a program's) is text here, never markup
        let words = ui::label(&found.words, &["found-words"]);
        words.set_selectable(true);
        card.append(&words);
        if let Some(fix) = found.fix.clone().filter(|fix| backend::fix_allowed(&fix.command)) {
            let weak = self.me.clone();
            let shown = fix.command.join(" ");
            let pressed = fix.clone();
            let button = ui::button(&fix.label, None, &["btn", "primary"], move || {
                if let Some(this) = weak.upgrade() {
                    this.press_fix(&pressed);
                }
            });
            button.set_sensitive(!busy);
            button.set_halign(gtk::Align::Start);
            card.append(&button);
            let command = ui::label(&shown, &["code"]);
            command.set_direction(gtk::TextDirection::Ltr);
            command.set_selectable(true);
            card.append(&command);
        } else if let Some(security) = found.security.clone().filter(|security| backend::security_allowed(&security.page)) {
            let button = self.security_button(&security);
            button.set_halign(gtk::Align::Start);
            card.append(&button);
        }
        card
    }

    fn security_button(&self, security: &backend::OpensSecurity) -> gtk::Button {
        let weak = self.me.clone();
        let page = security.page.clone();
        ui::button(&security.label, Some(icon(&SHIELD, 16)), &["btn"], move || {
            if let Some(this) = weak.upgrade() {
                this.away.set(true);
            }
            backend::open_security(&page);
        })
    }

    // --- What changed -------------------------------------------------------------------------------------------------

    fn read_changes(&self) {
        self.state.borrow_mut().list = List::Reading;
        self.render();
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let changes = backend::changes().await;
            if let Some(this) = weak.upgrade() {
                this.state.borrow_mut().list = List::Read(changes.map(Rc::new));
                this.render();
            }
        });
    }

    fn press_undo(&self, change: &Change, terminal: bool) {
        if terminal {
            self.away.set(true);
            backend::undo_in_terminal(&change.id);
            return;
        }
        self.state.borrow_mut().busy = change.id.clone();
        self.render();
        let (weak, id) = (self.me.clone(), change.id.clone());
        glib::spawn_future_local(async move {
            let undone = backend::undo(&id).await;
            let Some(this) = weak.upgrade() else { return };
            {
                let mut state = this.state.borrow_mut();
                state.busy.clear();
                state.said = Some(match undone {
                    Some(undone) => (!undone.ok, undone.words),
                    None => (true, t!("That did not work.")),
                });
            }
            this.read_changes();
        });
    }

    fn changed_page(&self) {
        let (list, said, busy) = {
            let state = self.state.borrow();
            (state.list.clone(), state.said.clone(), state.busy.clone())
        };
        self.content.append(&heading(&t!("What changed"), "h1"));
        self.content.append(&ui::label(&t!("What changed on this computer over the last month, in one list. Undo adds back and never deletes."), &["soft"]));
        if let Some((failed, text)) = said {
            self.content.append(&ui::label(&text, if failed { &["said", "failed"] } else { &["said"] }));
        }
        let changes = match list {
            List::None | List::Reading => {
                self.content.append(&reading(&t!("Reading what the computer kept…")));
                return;
            }
            List::Read(None) => {
                self.content.append(&ui::label(&t!("That could not be read."), &["said", "failed"]));
                return;
            }
            List::Read(Some(changes)) => changes,
        };
        if !changes.snapshots {
            let note = ui::vbox(8, &["callout"]);
            note.append(&ui::label(&t!("A file deleted outright and a setting as it was need the hourly snapshots, which are off. Security turns them on."), &[]));
            let button = self.security_button(&backend::OpensSecurity { label: t!("Open Security"), page: "settings".into() });
            button.set_halign(gtk::Align::Start);
            note.append(&button);
            self.content.append(&note);
        }
        if changes.items.is_empty() {
            self.content.append(&ui::label(&t!("Nothing this computer keeps a record of has changed in the last month."), &["soft"]));
            return;
        }
        let mut day = String::new();
        let mut card: Option<gtk::Box> = None;
        for change in &changes.items {
            let this_day = day_of(change.time);
            if this_day != day || card.is_none() {
                self.content.append(&heading(&this_day, "day"));
                let new = ui::vbox(0, &["card", "changes"]);
                new.set_overflow(gtk::Overflow::Hidden);
                self.content.append(&new);
                card = Some(new);
                day = this_day;
            }
            card.as_ref().expect("made above").append(&self.change_row(change, &busy));
        }
    }

    fn change_row(&self, change: &Change, busy: &str) -> gtk::Box {
        let row = ui::hbox(12, &["change"]);
        let mark = icon(
            match change.kind.as_str() {
                "file" => &FILE,
                "app" => &APP,
                "setting" => &SETTING,
                "update" => &UPDATE,
                "agent" => &AGENT,
                "security" => &SHIELD,
                _ => &HISTORY,
            },
            20,
        );
        mark.set_valign(gtk::Align::Start);
        row.append(&mark);
        let words = ui::vbox(1, &[]);
        words.set_hexpand(true);
        words.append(&ui::label(&change.what, &["strong"]));
        let time = i18n::date(change.time, DateStyle::Time);
        let detail = if change.detail.is_empty() { time } else { format!("{time} · {}", change.detail) };
        words.append(&ui::label(&detail, &["small", "faint"]));
        row.append(&words);
        if let Some(undo) = change.undo.clone() {
            let weak = self.me.clone();
            let (pressed, terminal) = (change.clone(), undo.terminal);
            let button = ui::button(&undo.label, None, &["btn", "small"], move || {
                if let Some(this) = weak.upgrade() {
                    this.press_undo(&pressed, terminal);
                }
            });
            button.set_sensitive(busy.is_empty());
            button.set_valign(gtk::Align::Center);
            row.append(&button);
        } else if let Some(security) = change.security.as_ref().filter(|security| backend::security_allowed(&security.page)) {
            let button = self.security_button(security);
            button.set_valign(gtk::Align::Center);
            row.append(&button);
        }
        row
    }

    fn render(&self) {
        ui::clear(&self.content);
        let page = self.state.borrow().page;
        self.wrong_nav.set_current(page == "wrong");
        self.changed_nav.set_current(page == "changed");
        if page == "changed" {
            self.changed_page();
        } else {
            self.wrong_page();
        }
    }
}

fn heading(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder().label(text).accessible_role(gtk::AccessibleRole::Heading).wrap(true).xalign(0.0).css_classes([class]).build()
}

fn reading(text: &str) -> gtk::Box {
    let working = ui::hbox(10, &["working"]);
    working.update_property(&[gtk::accessible::Property::Label(text)]);
    let mark = icon(&SPINNER, 18);
    mark.set_spinning(true);
    working.append(&mark);
    working.append(&ui::label(text, &[]));
    working
}

/// "Today", "Yesterday", or the day and month
fn day_of(seconds: f64) -> String {
    let days = i18n::days_from_today(seconds);
    if days == 0 || days == -1 {
        return i18n::capital(&i18n::days(days));
    }
    let same_year = i18n::local(seconds).map(|l| l.year) == i18n::local(i18n::now()).map(|l| l.year);
    i18n::date(seconds, if same_year { DateStyle::WeekdayDayMonth } else { DateStyle::WeekdayDayMonthYear })
}
