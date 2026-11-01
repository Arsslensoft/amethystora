//! Amethystora Phone: the windows of the phone link (plans/PLAN-amethystora-connect.md): the phones and their
//! pairing, the texts of a phone, sending a file or a link, answering a notification, and text a phone shared.
//! It is a client of the program amethystora-connect on the session bus and shows what that says: link.rs reads
//! it, model.rs holds what it says as plain data, and the pages (phones.rs, messages.rs, dialogs.rs) draw it.
//!
//! The window is started by the program's notification buttons, by the top bar's menu, by Files' Open With and by
//! hand, and takes its work from the command line (cli.rs), one window for all of them. Everything a phone says
//! is text: it is shown in labels, never as markup, and nothing a phone sent is opened or run by itself.

mod cli;
mod dialogs;
mod icons;
mod link;
mod messages;
mod model;
mod phones;
mod widgets;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::frame::{self, NavButton};
use amethystora_ui::{glib, gtk, i18n, process, store, t, ui};
use serde_json::json;

use cli::{Command, Problem};
use dialogs::Dialog;
use icons::icon;
use link::{Event, Failure, Link};
use model::Situation;
use widgets::Look;

pub fn main() -> glib::ExitCode {
    app::run(
        Spec {
            name: "phone",
            id: "org.amethystora.Phone",
            css: include_str!("phone.css"),
            width: 1120,
            height: 780,
            min_width: 820,
            min_height: 560,
        },
        Phone::new,
    )
}

/// The two pages of the sidebar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Phones,
    Messages,
}

impl Page {
    fn name(self) -> &'static str {
        match self {
            Page::Phones => "phones",
            Page::Messages => "messages",
        }
    }
}

/// What the last thing done here came to
#[derive(Debug, Clone)]
pub struct Banner {
    pub failed: bool,
    pub text: String,
}

/// What the window keeps between drawings
pub struct State {
    pub page: Page,
    /// Phones with a call under way, so that a second click does nothing
    pub busy: HashSet<String>,
    /// "Look again" is asked and the answer has not come
    pub looking: bool,
    pub banner: Option<Banner>,
    pub dialog: Option<Dialog>,
    /// A phone a command pointed at, marked for a moment
    pub attention: Option<String>,
}

/// The parts of the window made once
pub struct Parts {
    pub root: gtk::Overlay,
    pub phones_nav: NavButton,
    pub phones_badge: gtk::Label,
    pub messages_nav: NavButton,
    pub look_again: gtk::Button,
    pub banner: gtk::Box,
    pub loading: gtk::Box,
    pub off: gtk::Box,
    pub phones: phones::Parts,
    pub messages: messages::Parts,
}

pub struct Phone {
    me: Weak<Phone>,
    pub(crate) shell: Rc<Shell>,
    pub(crate) link: Rc<Link>,
    pub(crate) parts: Parts,
    pub(crate) state: RefCell<State>,
    pub(crate) texts: messages::Texts,
    pub(crate) dialog_layer: RefCell<Option<gtk::Widget>>,
    pub(crate) countdowns: RefCell<Vec<(String, gtk::Label)>>,
    pub(crate) countdown_timer: RefCell<Option<glib::SourceId>>,
    banner_timer: RefCell<Option<glib::SourceId>>,
    redraw: Cell<bool>,
    /// The bus has said whether the program is there
    known: Cell<bool>,
    /// The program is there and has said which phones it knows
    loaded: Cell<bool>,
    /// When the page was last drawn for a transfer that only moved on
    progress_drawn: Cell<f64>,
    /// The program's phones have been read once, so that a command can name one
    settled: Cell<bool>,
    pending: RefCell<Option<Command>>,
    started: Cell<bool>,
}

fn upgrade(me: &Weak<Phone>) -> Option<Rc<Phone>> {
    me.upgrade()
}

impl Parts {
    fn new(me: &Weak<Phone>) -> Parts {
        let brand = amethystora_ui::brand::brand(&t!("Phone"), 21.0);
        brand.add_css_class("brand");
        brand.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Phone"))]);
        let holder = ui::hbox(0, &[]);
        holder.append(&brand);
        let sidebar = frame::Sidebar::new(&holder, 18);

        let nav = frame::nav(3, None, &t!("Phone"));
        let phones_nav = NavButton::new(icon("phone", 20), &t!("Phones"), 12);
        let phones_badge = frame::badge();
        phones_nav.append(&phones_badge);
        let messages_nav = NavButton::new(icon("message", 20), &t!("Messages"), 12);
        for (button, page) in [(&phones_nav, Page::Phones), (&messages_nav, Page::Messages)] {
            let weak = me.clone();
            button.button.connect_clicked(move |_| {
                if let Some(this) = upgrade(&weak) {
                    this.show(page);
                }
            });
            nav.append(&button.button);
        }
        sidebar.body.append(&nav);

        sidebar.foot.set_spacing(12);
        let manual = ui::button(&t!("Phone in the manual"), Some(icon("book", 18).mirror_in_rtl()), &["link", "quiet"], || open_manual("phone"));
        manual.set_halign(gtk::Align::Start);
        sidebar.foot.append(&manual);
        let terminal = frame::terminal(&t!("Also in a terminal:"), "ame desktop phone");
        // The command reads left to right, at the start of its line: the right in a right-to-left window
        if let Some(code) = terminal.last_child().and_downcast::<gtk::Label>().filter(|_| i18n::is_rtl_now()) {
            code.set_xalign(1.0);
        }
        sidebar.foot.append(&terminal);

        // The top bar: looking again, with the window's own buttons
        let topbar_row = ui::hbox(0, &["topbar"]);
        let tools = ui::hbox(14, &[]);
        tools.set_hexpand(true);
        tools.set_halign(gtk::Align::End);
        tools.set_valign(gtk::Align::Center);
        let weak = me.clone();
        let look_again = widgets::button(&t!("Look again"), Look { icon: Some("refresh"), small: true, ..Default::default() }, move || {
            if let Some(this) = upgrade(&weak) {
                this.look_again();
            }
        });
        look_again.set_tooltip_text(Some(&t!("Announce this computer to the phones on your home networks again")));
        tools.append(&look_again);
        topbar_row.append(&tools);
        let controls = frame::controls();
        controls.set_valign(gtk::Align::Center);
        topbar_row.append(&controls);

        let banner = ui::vbox(0, &["banner-slot"]);
        banner.set_visible(false);

        let loading = ui::vbox(12, &["loading"]);
        loading.set_valign(gtk::Align::Center);
        loading.set_halign(gtk::Align::Center);
        loading.set_vexpand(true);
        let gem = amethystora_ui::gem::Gem::plain(34.0);
        gem.add_css_class("gem");
        loading.append(&gem);
        loading.append(&ui::line(&t!("Looking for the phone link…"), &[]));

        let off = ui::vbox(0, &["off"]);
        off.set_vexpand(true);
        off.set_visible(false);
        let phones = phones::Parts::new();
        let messages = messages::Parts::new(me);

        let content = ui::vbox(0, &["view"]);
        content.set_vexpand(true);
        content.append(&loading);
        content.append(&off);
        content.append(&phones.scroller);
        content.append(&messages.root);

        let main = ui::vbox(0, &["main"]);
        main.append(&topbar_row);
        main.append(&banner);
        main.append(&content);

        sidebar.root.set_vexpand(true);
        let column = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).propagate_natural_width(true).child(&sidebar.root).build();
        let framed = frame::with_sidebar(&column, &main, -1, false);
        let root = gtk::Overlay::new();
        root.set_child(Some(&framed));

        Parts { root, phones_nav, phones_badge, messages_nav, look_again, banner, loading, off, phones, messages }
    }
}

impl App for Phone {
    /// The dock's drop on a phone (`send --now` with files) and ringing a phone show no window unless they need the person
    fn presents(&self, args: &[String]) -> bool {
        !matches!(cli::parse(args, std::path::Path::new("/")), Ok(Command::Send { now: true, what: cli::Sending::Files(_), .. }) | Ok(Command::Ring { .. }))
    }

    // A file on the command line is relative to where the launch was made from, a second launch's own folder included
    fn open_in(&self, args: &[String], cwd: Option<PathBuf>) {
        let cwd = cwd.or_else(|| std::env::current_dir().ok()).unwrap_or_else(amethystora_ui::palette::home);
        let first = !self.started.replace(true);
        match cli::parse(args, &cwd) {
            Ok(mut command) => {
                // Opened with nothing to do, the window is as it was left
                if first && args.is_empty() {
                    let left = store::load("phone");
                    if left.get("page").and_then(|page| page.as_str()) == Some("messages") {
                        let device = left.get("device").and_then(|d| d.as_str()).filter(|d| cli::valid_device(d)).map(str::to_string);
                        command = Command::Messages { device };
                    }
                }
                self.pending.replace(Some(command));
                if first {
                    self.render();
                }
                if self.settled.get() {
                    self.rc().run_pending();
                }
            }
            Err(problem) => {
                if first {
                    self.render();
                }
                self.say(true, problem_text(&problem));
            }
        }
    }

    /// Back from elsewhere: what the program knows may have moved on
    fn focus(&self) {
        if self.known.get() && self.link.present() {
            self.link.refresh();
        }
    }

    fn reload(&self) {
        self.link.refresh();
        self.rc().texts_reload();
    }
}

/// What a command line that was not taken says
fn problem_text(problem: &Problem) -> String {
    match problem {
        Problem::Unknown(word) => t!("The phone window does not know “{command}”. It opens the phones, the messages, a file or link to send, a reply, or a pairing.", command = model::bdi(&model::clean(word, 40))),
        Problem::Usage(_) => t!("That command is missing something, or has more than it takes."),
        Problem::Id => t!("That is not the name of a phone or a notification."),
        Problem::Mixed => t!("A link is sent on its own: files and a link cannot be sent together."),
    }
}

/// The Manual on a page, started by its path (nothing is looked for on PATH, where an account can put its own) and in
/// a unit of its own, so that it neither ends with this window, which the program starts in a unit of its own that
/// ends with it, nor belongs to the program
fn open_manual(page: &str) {
    if !process::valid_manual_page(page) {
        return;
    }
    let manager = std::env::var_os("XDG_RUNTIME_DIR").map(|dir| std::path::Path::new(&dir).join("systemd/private"));
    if manager.is_some_and(|socket| socket.exists()) {
        process::launch("/usr/bin/systemd-run", &["--user", "--collect", "--quiet", "--", "/usr/bin/amethystora-manual", page]);
    } else {
        process::launch("/usr/bin/amethystora-manual", &[page]);
    }
}

/// What a call that failed says, in the window's words
pub fn failure_text(failure: &Failure) -> String {
    match failure {
        Failure::NotAvailable => t!("That is not possible now: the phone is not connected, or does not take it."),
        Failure::Gone => t!("The phone link is off. Turn it on with ame desktop phone on."),
        Failure::Other(_) => t!("That did not work."),
    }
}

impl Phone {
    fn new(shell: Rc<Shell>) -> Rc<Phone> {
        let this = Rc::new_cyclic(|me: &Weak<Phone>| {
            let weak = me.clone();
            let link = Link::new(move |event| {
                if let Some(this) = upgrade(&weak) {
                    this.on_event(event);
                }
            });
            Phone {
                me: me.clone(),
                shell: shell.clone(),
                link,
                parts: Parts::new(me),
                state: RefCell::new(State { page: Page::Phones, busy: HashSet::new(), looking: false, banner: None, dialog: None, attention: None }),
                texts: messages::Texts::default(),
                dialog_layer: RefCell::new(None),
                countdowns: RefCell::new(Vec::new()),
                countdown_timer: RefCell::new(None),
                banner_timer: RefCell::new(None),
                redraw: Cell::new(false),
                known: Cell::new(false),
                loaded: Cell::new(false),
                progress_drawn: Cell::new(0.0),
                settled: Cell::new(false),
                pending: RefCell::new(None),
                started: Cell::new(false),
            }
        });
        shell.window.set_title(Some(&t!("Amethystora Phone")));
        shell.window.set_content(Some(&this.parts.root));
        shell.narrow(1000.0, "narrow");
        this.keys();
        this.link.start();
        this
    }

    /// This window, from a method that only has &self
    pub(crate) fn rc(&self) -> Rc<Phone> {
        self.me.upgrade().expect("the window is open")
    }

    /// A closure for a widget's signal that runs `f` on this window while it is open
    pub(crate) fn go(&self, f: impl Fn(&Rc<Phone>) + 'static) -> impl Fn() + 'static {
        let weak = self.me.clone();
        move || {
            if let Some(this) = upgrade(&weak) {
                f(&this);
            }
        }
    }

    pub(crate) fn weak(&self) -> Weak<Phone> {
        self.me.clone()
    }

    /// A button that runs `f` on this window
    pub(crate) fn button(&self, label: &str, look: Look, f: impl Fn(&Rc<Phone>) + 'static) -> gtk::Button {
        widgets::button(label, look, self.go(f))
    }

    /// Escape closes a dialog
    fn keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(this) = upgrade(&me) else { return glib::Propagation::Proceed };
            if key == amethystora_ui::gdk::Key::Escape && this.state.borrow().dialog.is_some() {
                this.close_dialog();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.shell.window.add_controller(keys);
    }

    // --- What the program says -------------------------------------------------------------------------

    fn on_event(self: &Rc<Self>, event: Event) {
        match event {
            Event::Presence => {
                self.known.set(true);
                self.loaded.set(false);
                if !self.link.present() {
                    self.state.borrow_mut().busy.clear();
                    self.texts_reset();
                    self.settle();
                }
                self.render();
            }
            Event::Devices => {
                // The program has said which phones it knows: a command can name one now
                self.loaded.set(true);
                self.settle();
                self.schedule_render();
                self.dialogs_devices_changed();
                self.texts_devices_changed();
            }
            Event::Transfer(transfer) => {
                self.dialog_transfer(&transfer);
                // Progress comes often: the page is drawn again for it twice a second at most, and at once when a
                // transfer begins or ends
                let now = i18n::now();
                if transfer.state != "progress" || now - self.progress_drawn.get() > 0.5 {
                    self.progress_drawn.set(now);
                    self.schedule_render();
                }
            }
            Event::Conversations(device) => self.texts_conversations_changed(&device),
            Event::Messages(device, thread) => self.texts_messages_changed(&device, thread),
        }
    }

    /// The first reading is done, or there is nothing to read: run what the command line asked
    fn settle(self: &Rc<Self>) {
        if !self.settled.replace(true) {
            self.run_pending();
        }
    }

    fn run_pending(self: &Rc<Self>) {
        let Some(command) = self.pending.take() else { return };
        match command {
            Command::Phones => self.show(Page::Phones),
            Command::Messages { device } => {
                if let Some(device) = device {
                    self.texts.remember(&device);
                }
                self.show(Page::Messages);
            }
            Command::Send { device, what, now } => {
                self.show_if_first(Page::Phones);
                self.open_send(device, what, now);
            }
            Command::Reply { device, notification } => {
                self.show_if_first(Page::Phones);
                self.open_reply(&device, &notification);
            }
            Command::Pair { device } => {
                self.show(Page::Phones);
                self.pair_command(&device);
            }
            Command::Text { device, text } => {
                self.show_if_first(Page::Phones);
                self.open_text(&device, &text);
            }
            Command::Ring { device } => self.ring_command(device),
        }
    }

    /// `amethystora-phone ring`: the phone named, or every phone that can ring, is rung without the window, which a
    /// window never shown then closes; it shows, saying why, when the link is off, no phone can ring, or one failed
    fn ring_command(self: &Rc<Self>, device: Option<String>) {
        let shown_before = self.shell.window.is_visible();
        let ringable: Vec<model::Device> = model::listed(&self.link.devices())
            .into_iter()
            .filter(|d| d.situation() == Situation::Ready && d.has("ring") && device.as_ref().is_none_or(|id| &d.id == id))
            .collect();
        if !self.link.present() || ringable.is_empty() {
            self.show(Page::Phones);
            self.shell.window.present();
            if self.link.present() {
                self.say(true, t!("No phone can ring now. Pair a phone, connect it, and try again."));
            }
            return;
        }
        let this = self.clone();
        glib::spawn_future_local(async move {
            let mut failed = None;
            for phone in &ringable {
                if let Err(failure) = this.link.ring(&phone.id).await {
                    failed = Some(failure);
                }
            }
            match failed {
                Some(failure) => {
                    this.show(Page::Phones);
                    this.shell.window.present();
                    this.say(true, failure_text(&failure));
                }
                None if !shown_before && !this.shell.window.is_visible() => this.shell.app.quit(),
                None => {
                    let names: Vec<String> = ringable.iter().map(|d| model::bdi(&d.title())).collect();
                    this.say(false, t!("Ringing {phone}. It rings until Found it is pressed on it.", phone = i18n::list(&names)));
                }
            }
        });
    }

    /// A dialog opened from the command line is over the page the window was left on, or the phones when it opens
    fn show_if_first(&self, page: Page) {
        if !self.link.present() {
            self.state.borrow_mut().page = page;
            self.render();
        }
    }

    // --- Drawing the window ----------------------------------------------------------------------------

    /// Draws again at the next idle moment, once for however many things changed before it
    pub(crate) fn schedule_render(&self) {
        if !self.redraw.replace(true) {
            let weak = self.me.clone();
            glib::idle_add_local_once(move || {
                if let Some(this) = upgrade(&weak) {
                    this.redraw.set(false);
                    this.render();
                }
            });
        }
    }

    pub(crate) fn render(&self) {
        self.render_chrome();
        let page = self.state.borrow().page;
        if self.known.get() && self.link.present() && self.loaded.get() {
            match page {
                Page::Phones => self.render_phones(),
                Page::Messages => self.render_texts(),
            }
        }
    }

    /// The sidebar, the top bar and which of the views shows
    fn render_chrome(&self) {
        let (page, banner) = {
            let state = self.state.borrow();
            (state.page, state.banner.clone())
        };
        let known = self.known.get();
        let present = self.link.present();
        let ready = known && present && self.loaded.get();
        let parts = &self.parts;
        parts.phones_nav.set_current(page == Page::Phones);
        parts.messages_nav.set_current(page == Page::Messages);
        let asking = self.link.devices().iter().filter(|d| d.situation() == Situation::Asked).count();
        parts.phones_badge.set_visible(asking > 0);
        parts.phones_badge.set_label(&i18n::number(asking as f64));
        parts.phones_badge.update_property(&[gtk::accessible::Property::Label(&t!("{count, plural, one {# phone asks to pair} other {# phones ask to pair}}", count = asking))]);
        // In the empty page the same button is the page's own, so the bar has none there
        let listed = !model::listed(&self.link.devices()).is_empty();
        parts.look_again.set_visible(ready && page == Page::Phones && listed);
        parts.look_again.set_sensitive(!self.state.borrow().looking);
        parts.loading.set_visible(!known || (present && !ready));
        parts.off.set_visible(known && !present);
        parts.phones.scroller.set_visible(ready && page == Page::Phones);
        parts.messages.root.set_visible(ready && page == Page::Messages);
        if known && !present {
            self.render_off();
        }
        self.render_banner(banner);
    }

    /// The phone link is not running: what it is and how to turn it on
    fn render_off(&self) {
        let off = &self.parts.off;
        ui::clear(off);
        let gem = amethystora_ui::gem::Gem::plain(46.0);
        gem.add_css_class("gem");
        let view = ui::vbox(28, &["page"]);
        let how = ui::vbox(8, &[]);
        how.append(&widgets::centered(ui::label(&t!("To turn it on, run this in a terminal:"), &["soft"])));
        how.append(&widgets::command("ame desktop phone on"));
        let manual = widgets::link_button(&t!("What the phone link does"), Some("book"), || open_manual("phone"));
        manual.set_halign(gtk::Align::Center);
        view.append(&widgets::empty(vec![
            gem.upcast(),
            widgets::centered(widgets::heading(&t!("The phone link is off"), "h2")).upcast(),
            widgets::centered(ui::label(&t!("Your Android phone can send its notifications, texts and files here, and take yours, through the KDE Connect app. The link listens for phones only while it is on, and only on the networks you named home."), &[])).upcast(),
            how.upcast(),
            manual.upcast(),
        ]));
        // Another program holds the link's name: nothing is asked of it, and the person is told why the link is off
        if self.link.untrusted() {
            view.append(&widgets::callout(
                "check",
                &t!("Another program is using the phone link's name"),
                &t!("A program that is not the phone link has taken its name on the session bus. This window does not talk to it, and sends it nothing."),
            ));
        }
        off.append(&widgets::page_column(&view));
    }

    // --- The banner ------------------------------------------------------------------------------------

    /// Says what was done, for a few seconds, over the page
    pub(crate) fn say(&self, failed: bool, text: String) {
        self.state.borrow_mut().banner = Some(Banner { failed, text });
        self.render_banner(self.state.borrow().banner.clone());
        if let Some(timer) = self.banner_timer.take() {
            timer.remove();
        }
        let weak = self.me.clone();
        let seconds = if failed { 12 } else { 5 };
        let timer = glib::timeout_add_seconds_local_once(seconds, move || {
            if let Some(this) = upgrade(&weak) {
                this.banner_timer.take();
                this.state.borrow_mut().banner = None;
                this.render_banner(None);
            }
        });
        self.banner_timer.replace(Some(timer));
    }

    fn render_banner(&self, banner: Option<Banner>) {
        let slot = &self.parts.banner;
        ui::clear(slot);
        slot.set_visible(banner.is_some());
        let Some(banner) = banner else { return };
        let row = ui::hbox(10, &["banner"]);
        if banner.failed {
            row.add_css_class("failed");
        }
        row.append(&icon(if banner.failed { "alert" } else { "check" }, 18));
        let text = ui::label(&banner.text, &[]);
        text.set_hexpand(true);
        row.append(&text);
        let weak = self.me.clone();
        row.append(&widgets::icon_button("close", 16, &t!("Dismiss"), move || {
            if let Some(this) = upgrade(&weak) {
                this.state.borrow_mut().banner = None;
                this.render_banner(None);
            }
        }));
        let wrap = ui::vbox(0, &["banner-wrap"]);
        wrap.append(&row);
        slot.append(&wrap);
    }

    // --- Pages -----------------------------------------------------------------------------------------

    pub(crate) fn show(&self, page: Page) {
        self.state.borrow_mut().page = page;
        self.render();
        if page == Page::Messages {
            self.rc().texts_enter();
        }
        self.remember();
    }

    /// How the window was left, for the next time it is opened with nothing to do
    pub(crate) fn remember(&self) {
        let page = self.state.borrow().page;
        store::save("phone", &json!({ "page": page.name(), "device": self.texts.device().unwrap_or_default() }));
    }

    // --- Looking again ---------------------------------------------------------------------------------

    fn look_again(&self) {
        if std::mem::replace(&mut self.state.borrow_mut().looking, true) {
            return;
        }
        self.render_chrome();
        let this = self.rc();
        glib::spawn_future_local(async move {
            let result = this.link.announce().await;
            // The phones answer within a moment or not at all: the button rests for a short while
            glib::timeout_future(Duration::from_millis(1500)).await;
            this.state.borrow_mut().looking = false;
            this.render_chrome();
            if let Err(failure) = result {
                this.say(true, failure_text(&failure));
            }
        });
    }
}
