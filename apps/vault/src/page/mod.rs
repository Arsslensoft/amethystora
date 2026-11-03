//! The window: first run, the closed screen, or the sidebar and the page shown. Nothing here holds more than the list of names, and
//! a page is only made when it is first shown and thrown away when the vault closes, so that a closed vault costs next to nothing.
//!
//! The window learns that the vault has closed from the screen locking and the machine going to sleep (the bus), from being told
//! `closed` by any answer, on coming back to the front, and from one deadline a little after the service's idle time, which is the
//! only timer it has while nothing is shown.

mod add;
mod auth;
mod browser;
mod certs;
mod codes;
mod export;
mod first;
mod gate;
mod generator;
mod keys;
mod passwords;
mod protect;
mod sync;
mod vaults;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::rc::{Rc, Weak};
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{App, Shell};
use amethystora_ui::frame::NavButton;
use amethystora_ui::{adw, brand, gio, glib, gtk, i18n, process, store as saved, t, ui};
use serde_json::{json, Value};

use crate::icons::icon;
use crate::list::{ListPage, Row};
use crate::logos;
use crate::rpc::{self, Answer, Trouble};
use crate::secret::{self, Secrets};
use crate::widgets;

/// What the sidebar goes to
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Passwords,
    Authenticator,
    Codes,
    Keys,
    Certificates,
    Protected,
    Sync,
    Vaults,
    Browser,
}

impl Section {
    /// `amethystora-vault NAME`, and how it is remembered
    fn name(self) -> &'static str {
        match self {
            Section::Passwords => "passwords",
            Section::Authenticator => "authenticator",
            Section::Codes => "codes",
            Section::Keys => "keys",
            Section::Certificates => "certificates",
            Section::Protected => "protected",
            Section::Sync => "sync",
            Section::Vaults => "vaults",
            Section::Browser => "browser",
        }
    }

    fn from_name(name: &str) -> Option<Section> {
        NAV.iter().map(|(section, _, _)| *section).chain([Section::Vaults, Section::Browser]).find(|section| section.name() == name)
    }

    fn words(self) -> String {
        match self {
            Section::Passwords => t!("Passwords"),
            Section::Authenticator => t!("Authenticator"),
            Section::Codes => t!("Backup codes"),
            Section::Keys => t!("Keys"),
            Section::Certificates => t!("Certificates"),
            Section::Protected => t!("Protected files"),
            Section::Sync => t!("Sync"),
            Section::Vaults => t!("Your vaults"),
            Section::Browser => t!("In your browser"),
        }
    }
}

/// The sidebar's sections, their icons and the kind of item each counts
const NAV: [(Section, &str, Option<&str>); 7] = [
    (Section::Passwords, "pass", Some("password")),
    (Section::Authenticator, "bolt", Some("authenticator")),
    (Section::Codes, "codes", Some("backup-codes")),
    (Section::Keys, "key", Some("key")),
    (Section::Certificates, "cert", Some("certificate")),
    (Section::Protected, "lock", Some("protected")),
    (Section::Sync, "refresh", None),
];

/// How wide the window is: a list beside its detail, one of them at a time, or the sidebar as icons too
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Full,
    Narrow,
    Tiny,
}

/// A page of the window
pub trait Page {
    fn widget(&self) -> gtk::Widget;
    /// Everything read again
    fn reload(&self);
    fn shown(&self) {}
    fn left(&self) {}
    /// The window came to the front or went away, while this page is shown
    fn focus(&self, _active: bool) {}
    fn width(&self, _width: Width) {}
    /// Ctrl+N, or `amethystora-vault PAGE new`
    fn add(&self) {}
    /// Ctrl+F
    fn find(&self) {}
    /// Show this item, once the list has it
    fn select(&self, _id: &str) {}
}

/// By name, the language's own way, then the newest first
pub fn sort_rows(rows: &mut [Row]) {
    i18n::with(|i18n| {
        let collator = i18n.collator();
        rows.sort_by(|a, b| {
            let by_name = match &collator {
                Some(collator) => collator.compare(&a.title, &b.title),
                None => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            };
            by_name.then(b.changed.cmp(&a.changed))
        });
    });
}

pub struct Vault {
    pub me: Weak<Vault>,
    pub shell: Rc<Shell>,
    pub toasts: adw::ToastOverlay,
    /// `wait`, `first`, `closed` or `open`
    pub root: gtk::Stack,
    pub secrets: Secrets,
    pub main: gtk::Stack,
    pages: RefCell<HashMap<Section, Rc<dyn Page>>>,
    pub section: Cell<Section>,
    nav: RefCell<Vec<(Section, NavButton, gtk::Label)>>,
    vaults_button: RefCell<Vec<(Section, NavButton)>>,
    sidebar: gtk::Box,
    nav_scroller: gtk::ScrolledWindow,
    foot: gtk::Box,
    foot_words: gtk::Label,
    foot_state: gtk::Box,
    foot_lock: RefCell<Option<gtk::Button>>,
    manual_button: RefCell<Option<gtk::Button>>,
    /// What `status` last said
    pub status: RefCell<Value>,
    pub width: Cell<Width>,
    /// Items whose code this opening has already been let see, which show without being asked for again. Never one set to ask every
    /// time: that is asked for by a click, each time, and by nothing else.
    pub approved: RefCell<HashSet<String>>,
    close_check: RefCell<Option<glib::SourceId>>,
    /// A recovery key is on the screen, and nothing the service says may take it away: the draft it belongs to is not a vault yet
    recovery_up: Cell<bool>,
    /// What the closed screen was made for, so that coming back to the window does not make it again
    closed_key: RefCell<String>,
    /// Which vaults were open when the service last said, to read the pages again when they change
    open_ids: RefCell<String>,
    /// The toasts on the screen, and for a removal that can be undone the flag that says it is not to be done
    toasts_live: RefCell<Vec<(adw::Toast, Option<Rc<Cell<bool>>>)>>,
    /// The words shown after a moment of waiting for the service, and the moment
    waiting: gtk::Label,
    wait_timer: RefCell<Option<glib::SourceId>>,
    /// Which vault an authenticator code or backup codes are being added to, while the sheet of ways in or what follows it is up
    pub add_target: RefCell<Option<Rc<Target>>>,
    watches: RefCell<Vec<gio::SignalSubscription>>,
    /// The gem of the closed screen, which moves while a vault is opening
    pub gate_gem: RefCell<Option<amethystora_ui::gem::Gem>>,
    /// What was asked for on the command line, once the vault is open
    wanted: RefCell<Option<(Section, bool)>>,
    first_open: Cell<bool>,
}

/// Where the sidebar goes in the window, the window's buttons and the toasts over all of it
pub fn build(shell: Rc<Shell>) -> Rc<Vault> {
    let root = gtk::Stack::builder().hhomogeneous(false).vhomogeneous(false).hexpand(true).vexpand(true).build();
    // What shows while the service has not answered. Its words come only after a moment, so that an answer that is quick is not preceded
    // by a flash of them.
    let waiting = ui::label(&t!("Waiting for the vault’s service…"), &["soft"]);
    waiting.set_visible(false);
    waiting.set_halign(gtk::Align::Center);
    waiting.set_valign(gtk::Align::Center);
    waiting.set_hexpand(true);
    waiting.set_xalign(0.5);
    waiting.set_justify(gtk::Justification::Center);
    // The page itself is always visible, which is what a stack shows first; only the words come late
    let wait_page = ui::hbox(0, &[]);
    wait_page.append(&waiting);
    root.add_named(&wait_page, Some("wait"));
    let handle = gtk::WindowHandle::builder().child(&root).build();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&handle));
    let controls = amethystora_ui::frame::controls();
    controls.set_halign(gtk::Align::End);
    controls.set_valign(gtk::Align::Start);
    controls.add_css_class("floating");
    overlay.add_overlay(&controls);
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&overlay));
    shell.window.set_content(Some(&toasts));
    shell.window.set_title(Some(&t!("Amethystora Vault")));
    crate::measure::first_frame(&shell.window);

    let main = gtk::Stack::builder().hhomogeneous(false).vhomogeneous(false).hexpand(true).vexpand(true).build();
    let foot = ui::vbox(10, &["foot-card"]);
    let vault = Rc::new_cyclic(|me| Vault {
        me: me.clone(),
        shell: shell.clone(),
        toasts,
        root,
        secrets: Secrets::default(),
        main,
        pages: RefCell::new(HashMap::new()),
        section: Cell::new(Section::Passwords),
        nav: RefCell::new(Vec::new()),
        vaults_button: RefCell::new(Vec::new()),
        sidebar: ui::vbox(22, &["sidebar"]),
        nav_scroller: gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build(),
        foot,
        foot_words: ui::label("", &["soft", "small"]),
        foot_state: ui::hbox(8, &["foot-state"]),
        foot_lock: RefCell::new(None),
        manual_button: RefCell::new(None),
        status: RefCell::new(Value::Null),
        width: Cell::new(Width::Full),
        approved: RefCell::new(HashSet::new()),
        close_check: RefCell::new(None),
        recovery_up: Cell::new(false),
        closed_key: RefCell::new(String::new()),
        open_ids: RefCell::new(String::new()),
        toasts_live: RefCell::new(Vec::new()),
        waiting,
        wait_timer: RefCell::new(None),
        add_target: RefCell::new(None),
        watches: RefCell::new(Vec::new()),
        gate_gem: RefCell::new(None),
        wanted: RefCell::new(None),
        first_open: Cell::new(true),
    });
    vault.build_frame();
    vault.widths();
    vault.keys();
    vault.watch_session();
    logos::init(&vault);
    let weak = vault.me.clone();
    shell.window.connect_is_active_notify(move |window| {
        if let Some(this) = weak.upgrade() {
            this.active_changed(window.is_active());
        }
    });
    // A secret copied in the last 30 seconds goes with the window: nothing else would clear it, and the desktop's clipboard keeps
    // what an owner that has gone leaves
    shell.window.connect_close_request(|window| {
        secret::clear_if_ours();
        let display = gtk::prelude::WidgetExt::display(window);
        display.sync();
        display.flush();
        glib::Propagation::Proceed
    });
    // The waiting words come after a moment, if nothing has been drawn by then
    let weak = vault.me.clone();
    vault.wait_timer.replace(Some(glib::timeout_add_local_once(Duration::from_millis(700), move || {
        if let Some(this) = weak.upgrade() {
            this.wait_timer.take();
            this.waiting.set_visible(true);
        }
    })));
    vault
}

impl App for Vault {
    fn open(&self, args: &[String]) {
        let section = args.first().and_then(|name| Section::from_name(name));
        let adding = args.get(1).is_some_and(|word| word == "new");
        if self.first_open.replace(false) {
            self.wanted.replace(section.map(|section| (section, adding)));
            // Where it was left, and when the last export was, read off the window's thread before the first screen is chosen
            self.spawn(async move |this| {
                let state = gio::spawn_blocking(|| saved::load("vault")).await.unwrap_or(Value::Null);
                export::remember_last(state["last_export"].as_i64().unwrap_or(0));
                if let Some(last) = state["section"].as_str().and_then(Section::from_name).filter(|_| section.is_none()) {
                    this.section.set(last);
                }
                this.check();
            });
            return;
        }
        if let Some(section) = section {
            if self.is_open() {
                self.go(section);
                if adding {
                    self.current_page(|page| page.add());
                }
            } else {
                self.wanted.replace(Some((section, adding)));
            }
        }
    }

    fn reload(&self) {
        if self.is_open() {
            self.count_items();
            self.current_page(|page| page.reload());
        }
    }

    fn palette(&self) {
        // The stones of letters are drawn in code, for a light window or a dark one
        self.root.queue_draw();
    }
}

impl Vault {
    pub fn spawn<F: Future<Output = ()> + 'static>(&self, make: impl FnOnce(Rc<Vault>) -> F) {
        if let Some(this) = self.me.upgrade() {
            glib::spawn_future_local(make(this));
        }
    }


    /// Whether the window has the focus, or gets it within a moment. A prompt that is being answered takes the focus while it is up and
    /// gives it back as it goes, so an answer waits a little for that; one that comes to a window nobody is looking at is not shown.
    pub async fn active_soon(&self) -> bool {
        for _ in 0..30 {
            if self.shell.window.is_active() {
                return true;
            }
            glib::timeout_future(Duration::from_millis(100)).await;
        }
        false
    }

    pub fn is_open(&self) -> bool {
        self.root.visible_child_name().as_deref() == Some("open")
    }

    /// Words in a toast: text, never markup, and without what draws nothing
    pub fn say(&self, words: &str) {
        self.add_toast(adw::Toast::builder().title(widgets::clean(words)).use_markup(false).build(), None);
    }

    /// A toast on the screen. For one that stands for something not yet done, `cancel` is the flag that says it is not to be: the
    /// vault closing sets it, and takes the toast away.
    pub fn add_toast(&self, toast: adw::Toast, cancel: Option<Rc<Cell<bool>>>) {
        self.toasts_live.borrow_mut().push((toast.clone(), cancel));
        let weak = self.me.clone();
        toast.connect_dismissed(move |gone| {
            if let Some(this) = weak.upgrade() {
                this.toasts_live.borrow_mut().retain(|(toast, _)| toast != gone);
            }
        });
        self.toasts.add_toast(toast);
    }

    /// One operation of the service. When it says the vault has closed, the window goes to the closed screen. Only what the service
    /// counts as use keeps the vault open, so only that moves the window's deadline.
    pub async fn call(&self, request: Value) -> Result<Answer, Trouble> {
        let used = rpc::is_use(&request);
        let answer = rpc::ask(request).await;
        match &answer {
            Err(Trouble::Closed | Trouble::NotSetUp) => self.check(),
            Ok(_) if used => self.arm_close_check(),
            _ => {}
        }
        answer
    }

    /// What the service says now decides which screen shows
    pub fn check(&self) {
        self.spawn(async |this| {
            let status = rpc::ask(json!({"op": "status"})).await;
            this.apply_status(status);
        });
    }

    fn stop_waiting(&self) {
        if let Some(timer) = self.wait_timer.take() {
            timer.remove();
        }
        self.waiting.set_visible(false);
    }

    /// The first vault's id while it is open, which is where logos are kept
    pub fn first_open_vault(&self) -> Option<String> {
        let status = self.status.borrow();
        status["vaults"].as_array().into_iter().flatten().find(|vault| vault["first"] == json!(true) && vault["open"] == json!(true)).and_then(|vault| vault["id"].as_str().map(str::to_string))
    }

    /// The ids of the vaults that are hidden, so that nothing is asked of a site for what they hold
    pub fn hidden_vaults(&self) -> HashSet<String> {
        let status = self.status.borrow();
        status["vaults"].as_array().into_iter().flatten().filter(|vault| vault["hidden"] == json!(true)).filter_map(|vault| vault["id"].as_str().map(str::to_string)).collect()
    }

    /// The name of an open vault, for a question that has to say which
    pub fn vault_name(&self, id: &str) -> String {
        let status = self.status.borrow();
        let found = status["vaults"].as_array().into_iter().flatten().find(|vault| vault["id"].as_str() == Some(id));
        widgets::clean(found.and_then(|vault| vault["name"].as_str()).unwrap_or(""))
    }

    fn apply_status(&self, status: Result<Answer, Trouble>) {
        self.stop_waiting();
        let status = match status {
            Ok(answer) => answer.0.clone(),
            Err(_) => {
                // The service could not be reached: as closed, and said on the closed screen. A recovery key being written down
                // stays, and so does what the person is typing: a service that is slow is not a closed vault.
                if self.recovery_up.get() {
                    return;
                }
                self.leave_open();
                self.show_closed(Some(&t!("The vault's service could not be reached.")));
                return;
            }
        };
        let set_up = status["set_up"] == json!(true);
        crate::measure::screen_ready("the first screen");
        let vaults = status["vaults"].as_array().cloned().unwrap_or_default();
        let any_open = vaults.iter().any(|vault| vault["open"] == json!(true));
        let open_ids = vaults.iter().filter(|vault| vault["open"] == json!(true)).filter_map(|vault| vault["id"].as_str()).collect::<Vec<_>>().join(",");
        self.status.replace(status);
        logos::set_first(if any_open { self.first_open_vault() } else { None });
        // The Vaults page draws from the status, and so does what says which vault something goes in
        if let Some(page) = self.pages.borrow().get(&Section::Vaults).cloned() {
            page.reload();
        }
        if self.recovery_up.get() {
            return;
        }
        if !set_up {
            self.leave_open();
            if self.root.visible_child_name().as_deref() != Some("first") {
                self.show_first();
            }
        } else if any_open {
            self.update_foot();
            if !self.is_open() {
                self.show_open();
            } else if *self.open_ids.borrow() != open_ids {
                // Another vault opened or closed: what the pages list is not what it was
                self.count_items();
                self.reload_current();
            }
            self.open_ids.replace(open_ids);
            if self.close_check.borrow().is_none() {
                self.arm_close_check();
            }
        } else {
            self.leave_open();
            let key = self.methods().join(",");
            if self.root.visible_child_name().as_deref() != Some("closed") || *self.closed_key.borrow() != key {
                self.show_closed(None);
            }
        }
        self.drop_first_page();
    }

    /// The page the recovery key was on is taken out of the window once it is not the one shown, so that the key is not left in it
    fn drop_first_page(&self) {
        if !self.recovery_up.get() && self.root.visible_child_name().as_deref() != Some("first") {
            if let Some(old) = self.root.child_by_name("first") {
                self.root.remove(&old);
            }
        }
    }

    /// What an open vault left on the screen taken away, if there was any
    fn leave_open(&self) {
        if self.is_open() || !self.pages.borrow().is_empty() {
            self.closed();
        }
    }

    /// One deadline, a little after the service's idle time, and no timer otherwise: a vault that closes by itself leaves no names on
    /// the screen for long. Every answer that is use moves it, and nothing else.
    fn arm_close_check(&self) {
        let minutes = self.status.borrow()["idle_minutes"].as_u64().unwrap_or(15);
        let weak = self.me.clone();
        let source = glib::timeout_add_seconds_local_once((minutes as u32 * 60).saturating_add(5), move || {
            if let Some(this) = weak.upgrade() {
                this.close_check.take();
                if this.is_open() {
                    this.check();
                }
            }
        });
        if let Some(earlier) = self.close_check.replace(Some(source)) {
            earlier.remove();
        }
    }

    /// Everything of an open vault taken off the screen and out of memory: what is shown, the sheets that are up (a password being
    /// edited, a setting of a hidden vault, a camera), the toasts, and the logos held
    pub fn closed(&self) {
        self.secrets.hide_all();
        secret::clear_if_ours();
        self.approved.borrow_mut().clear();
        widgets::close_sheets();
        self.add_target.take();
        let toasts = std::mem::take(&mut *self.toasts_live.borrow_mut());
        for (toast, cancel) in toasts {
            if let Some(cancel) = cancel {
                cancel.set(true);
            }
            toast.dismiss();
        }
        logos::forget();
        self.open_ids.take();
        if let Some(check) = self.close_check.take() {
            check.remove();
        }
        let pages = std::mem::take(&mut *self.pages.borrow_mut());
        for (_, page) in pages {
            page.left();
            self.main.remove(&page.widget());
        }
    }

    /// The vault shown after an opening: the screen is decided again by what the service says
    pub fn opened(&self) {
        self.check();
    }

    fn active_changed(&self, active: bool) {
        if !active {
            // The page first, which notes what was showing; then everything secret goes
            self.current_page(|page| page.focus(false));
            self.secrets.hide_all();
            return;
        }
        if !self.first_open.get() {
            // It may have closed while the window was elsewhere. This changes the screen only if it did.
            self.check();
            if let Some(this) = self.me.upgrade() {
                logos::resume(&this);
            }
        }
        self.current_page(|page| page.focus(true));
    }

    fn current_page(&self, act: impl FnOnce(&dyn Page)) {
        let page = self.pages.borrow().get(&self.section.get()).cloned();
        if let Some(page) = page {
            act(page.as_ref());
        }
    }

    /// The screen locking and the machine going to sleep close the vault in the service, and take the names off the screen here at
    /// once. The buses' signals cost nothing while nothing happens. Never a blocking call to get to a bus.
    fn watch_session(&self) {
        self.spawn(async |this| {
            let lock = {
                let weak = this.me.clone();
                move || {
                    if let Some(this) = weak.upgrade() {
                        if this.is_open() {
                            this.closed();
                            this.show_closed(None);
                        }
                    }
                }
            };
            if let Ok(session) = gio::bus_get_future(gio::BusType::Session).await {
                let lock = lock.clone();
                this.watches.borrow_mut().push(session.subscribe_to_signal(
                    None,
                    Some("org.gnome.ScreenSaver"),
                    Some("ActiveChanged"),
                    Some("/org/gnome/ScreenSaver"),
                    None,
                    gio::DBusSignalFlags::NONE,
                    move |signal| {
                        if signal.parameters.get::<(bool,)>().is_some_and(|(active,)| active) {
                            lock();
                        }
                    },
                ));
            }
            if let Ok(system) = gio::bus_get_future(gio::BusType::System).await {
                this.watches.borrow_mut().push(system.subscribe_to_signal(
                    Some("org.freedesktop.login1"),
                    Some("org.freedesktop.login1.Manager"),
                    Some("PrepareForSleep"),
                    Some("/org/freedesktop/login1"),
                    None,
                    gio::DBusSignalFlags::NONE,
                    move |signal| {
                        if signal.parameters.get::<(bool,)>().is_some_and(|(sleeping,)| sleeping) {
                            lock();
                        }
                    },
                ));
            }
        });
    }

    fn keys(&self) {
        let keys = gtk::ShortcutController::new();
        keys.set_scope(gtk::ShortcutScope::Global);
        let bind = |trigger: &str, act: Box<dyn Fn(&Vault)>| {
            let weak = self.me.clone();
            let action = gtk::CallbackAction::new(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    if this.is_open() {
                        act(&this);
                    }
                }
                glib::Propagation::Stop
            });
            keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger), Some(action)));
        };
        bind("<Control>l", Box::new(|this| this.lock_now()));
        bind("<Control>f", Box::new(|this| this.current_page(|page| page.find())));
        // Not over a sheet that is up already: they would stack
        bind("<Control>n", Box::new(|this| {
            if !widgets::any_sheet() {
                this.current_page(|page| page.add())
            }
        }));
        self.shell.window.add_controller(keys);
    }

    /// The names come off the screen first, and the service is asked after: a slow service must not leave them up
    pub fn lock_now(&self) {
        if self.is_open() {
            self.closed();
            self.show_closed(None);
        }
        self.spawn(async |this| {
            let _ = rpc::ask(json!({"op": "lock"})).await;
            this.check();
        });
    }

    // --- The frame ---------------------------------------------------------------------------------------------------------

    fn build_frame(&self) {
        let mark = brand::brand(&t!("Vault"), 21.0);
        mark.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Vault"))]);
        mark.set_halign(gtk::Align::Start);
        self.sidebar.append(&mark);

        let nav = ui::vbox(2, &["nav"]);
        nav.update_property(&[gtk::accessible::Property::Label(&t!("What the vault holds"))]);
        for (section, glyph, _) in NAV {
            let button = NavButton::new(icon(glyph, 17), &section.words(), 10);
            button.button.update_property(&[gtk::accessible::Property::Label(&section.words())]);
            let count = amethystora_ui::frame::badge();
            count.add_css_class("count");
            button.append(&count);
            let weak = self.me.clone();
            button.button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.go(section);
                }
            });
            nav.append(&button.button);
            self.nav.borrow_mut().push((section, button, count));
        }
        // Apart from what the vault holds: the vaults themselves, and the browser
        for (at, (section, glyph)) in [(Section::Vaults, "vaults"), (Section::Browser, "globe")].into_iter().enumerate() {
            let button = NavButton::new(icon(glyph, 17), &section.words(), 10);
            button.button.update_property(&[gtk::accessible::Property::Label(&section.words())]);
            let weak = self.me.clone();
            button.button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.go(section);
                }
            });
            if at == 0 {
                button.button.add_css_class("nav-apart");
            }
            nav.append(&button.button);
            self.vaults_button.borrow_mut().push((section, button));
        }
        self.nav_scroller.set_child(Some(&nav));
        self.sidebar.append(&self.nav_scroller);

        let state_words = ui::line(&t!("Open"), &[]);
        self.foot_state.append(&icon("lock", 14));
        self.foot_state.append(&state_words);
        self.foot.append(&self.foot_state);
        self.foot.append(&self.foot_words);
        let weak = self.me.clone();
        let lock = widgets::btn(&t!("Lock now"), None, &["small"], move || {
            if let Some(this) = weak.upgrade() {
                this.lock_now();
            }
        });
        self.foot.append(&lock);
        self.foot_lock.replace(Some(lock));
        self.sidebar.append(&self.foot);
        let manual = ui::button(&t!("Vault in the manual"), Some(icon("book", 15)), &["link", "quiet"], || process::manual("vault"));
        if let Some(content) = manual.child().and_then(|child| child.downcast::<gtk::Box>().ok()) {
            content.set_spacing(8);
        }
        manual.set_halign(gtk::Align::Start);
        self.sidebar.append(&manual);
        self.manual_button.replace(Some(manual));

        let layout = ui::hbox(0, &["layout"]);
        self.sidebar.set_hexpand(false);
        let column = crate::layout::Exact::new(&self.sidebar);
        column.set_hexpand(false);
        layout.append(&column);
        layout.append(&self.main);
        self.root.add_named(&layout, Some("open"));
    }

    /// Three widths, as libadwaita's breakpoints say, of which one applies at a time
    fn widths(&self) {
        for (limit, width, class) in [(1000.0, Width::Narrow, "narrow"), (760.0, Width::Tiny, "tiny")] {
            let condition = adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth, limit, adw::LengthUnit::Px);
            let breakpoint = adw::Breakpoint::new(condition);
            let weak = self.me.clone();
            breakpoint.connect_apply(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.shell.window.add_css_class(class);
                    this.set_width(width);
                }
            });
            let weak = self.me.clone();
            breakpoint.connect_unapply(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.shell.window.remove_css_class(class);
                    if this.width.get() == width {
                        this.set_width(Width::Full);
                    }
                }
            });
            self.shell.window.add_breakpoint(breakpoint);
        }
    }

    fn set_width(&self, width: Width) {
        self.width.set(width);
        let wide = width != Width::Tiny;
        // With the names gone, a tooltip says what each button is
        for (section, button, count) in self.nav.borrow().iter() {
            button.label.set_visible(wide);
            button.button.set_tooltip_text(if wide { None } else { Some(section.words()) }.as_deref());
            count.set_visible(wide && !count.label().is_empty());
        }
        for (section, button) in self.vaults_button.borrow().iter() {
            button.label.set_visible(wide);
            button.button.set_tooltip_text(if wide { None } else { Some(section.words()) }.as_deref());
        }
        self.foot_words.set_visible(wide);
        self.foot_state.set_visible(wide);
        if let Some(manual) = &*self.manual_button.borrow() {
            manual.set_visible(wide);
        }
        for page in self.pages.borrow().values() {
            page.width(width);
        }
    }

    fn update_foot(&self) {
        let minutes = self.status.borrow()["idle_minutes"].as_u64().unwrap_or(15);
        self.foot_words.set_label(&t!("Closes after {minutes, plural, one {# minute} other {# minutes}} without use, or when the screen locks.", minutes = minutes));
    }

    fn show_open(&self) {
        self.root.set_visible_child_name("open");
        self.count_items();
        let wanted = self.wanted.take();
        let section = wanted.map_or(self.section.get(), |(section, _)| section);
        self.go_now(section);
        if wanted.is_some_and(|(_, adding)| adding) {
            self.current_page(|page| page.add());
        }
    }

    /// The numbers in the sidebar, from the list: nothing is opened for them
    pub fn count_items(&self) {
        self.spawn(async |this| {
            let Ok(answer) = this.call(json!({"op": "list"})).await else { return };
            let mut counts: HashMap<String, u64> = HashMap::new();
            for item in answer["items"].as_array().into_iter().flatten() {
                if let Some(kind) = item["kind"].as_str() {
                    *counts.entry(kind.to_string()).or_default() += 1;
                }
            }
            let wide = this.width.get() != Width::Tiny;
            for (at, (_, _, kind)) in NAV.iter().enumerate() {
                let nav = this.nav.borrow();
                let count = &nav[at].2;
                let number = kind.and_then(|kind| counts.get(kind)).copied().unwrap_or(0);
                count.set_label(&if number > 0 { i18n::number(number as f64) } else { String::new() });
                count.set_visible(wide && number > 0);
            }
        });
    }

    /// The page for a section, made the first time
    fn page(&self, section: Section) -> Rc<dyn Page> {
        if let Some(page) = self.pages.borrow().get(&section) {
            return page.clone();
        }
        let Some(this) = self.me.upgrade() else { unreachable!("the window holds what it calls") };
        let page: Rc<dyn Page> = match section {
            Section::Passwords => passwords::Passwords::new(&this),
            Section::Authenticator => auth::Authenticator::new(&this),
            Section::Codes => codes::Codes::new(&this),
            Section::Keys => keys::Keys::new(&this),
            Section::Certificates => certs::Certificates::new(&this),
            Section::Protected => protect::Protected::new(&this),
            Section::Sync => sync::SyncPage::new(&this),
            Section::Vaults => vaults::Vaults::new(&this),
            Section::Browser => browser::Browser::new(&this),
        };
        self.main.add_named(&page.widget(), Some(section.name()));
        page.width(self.width.get());
        page.reload();
        self.pages.borrow_mut().insert(section, page.clone());
        page
    }

    pub fn go(&self, section: Section) {
        if section == self.section.get() && self.main.visible_child_name().as_deref() == Some(section.name()) {
            return;
        }
        self.go_now(section);
    }

    fn go_now(&self, section: Section) {
        self.current_page(|page| page.left());
        self.secrets.hide_all();
        self.section.set(section);
        // A page already made is read again as it is shown: the vault may have been changed from the command line or the browser's
        // extension, or another vault opened, since it was drawn. A page made now has just been read.
        let existed = self.pages.borrow().contains_key(&section);
        let page = self.page(section);
        self.main.set_visible_child_name(section.name());
        for (at, button, _) in self.nav.borrow().iter() {
            button.set_current(*at == section);
        }
        for (at, button) in self.vaults_button.borrow().iter() {
            button.set_current(*at == section);
        }
        page.shown();
        if existed {
            page.reload();
            self.count_items();
        }
        // Written off the window's thread, and not waited for
        let name = section.name();
        glib::spawn_future_local(async move {
            let _ = gio::spawn_blocking(move || {
                let mut state = saved::load("vault");
                if !state.is_object() {
                    state = json!({});
                }
                state["section"] = json!(name);
                saved::save("vault", &state);
            })
            .await;
        });
    }

    // --- What several pages do ---------------------------------------------------------------------------------------------

    /// A removal that can be undone for a few seconds: the row goes at once, and the service is only asked, which always takes the
    /// person's approval, once Undo has gone. Refused, the row comes back.
    pub fn remove_with_undo(&self, list: &Rc<ListPage>, kind: &'static str, id: &str, title: &str, reload: impl Fn() + 'static) {
        let id = id.to_string();
        list.hide_row(&id);
        // The name is text: a toast's title is markup unless it is told not to be
        let toast = adw::Toast::builder().title(t!("“{name}” will be removed.", name = widgets::clean(title))).use_markup(false).button_label(t!("Undo")).timeout(5).build();
        let undone = Rc::new(Cell::new(false));
        let marked = undone.clone();
        let shown = list.clone();
        let undo_id = id.clone();
        toast.connect_button_clicked(move |_| {
            marked.set(true);
            shown.show_row(&undo_id);
        });
        let weak = self.me.clone();
        let list = list.clone();
        let reload: Rc<dyn Fn()> = Rc::new(reload);
        let cancelled = undone.clone();
        toast.connect_dismissed(move |_| {
            if undone.get() {
                return;
            }
            let (Some(this), id, list, reload) = (weak.upgrade(), id.clone(), list.clone(), reload.clone()) else { return };
            glib::spawn_future_local(async move {
                let removed = this.call(json!({"op": "remove", "kind": kind, "id": id})).await;
                list.show_row(&id);
                match removed {
                    Ok(_) => {
                        this.count_items();
                        reload();
                    }
                    Err(trouble) => this.say(&trouble.words()),
                }
            });
        });
        self.add_toast(toast, Some(cancelled));
    }

    /// A choice of how often an item asks, on the words the service keeps. `Never ask` first says what it gives up.
    pub fn approval_card(&self, kind: &'static str, row: &Row) -> gtk::Box {
        let card = widgets::card();
        card.append(&widgets::h2(&t!("Before each use")));
        let current = Rc::new(RefCell::new(row.approval.clone()));
        let id = row.id.clone();
        let weak = self.me.clone();
        let choices = [("ask", t!("Ask every time")), ("while-open", t!("While open")), ("never", t!("Never ask"))];
        let seg = widgets::seg(&choices.iter().map(|(value, words)| (*value, words.clone())).collect::<Vec<_>>(), &row.approval, move |value| {
            let Some(this) = weak.upgrade() else { return };
            let (value, id, current) = (value.to_string(), id.clone(), current.clone());
            glib::spawn_future_local(async move {
                if value == "never" {
                    let sure = this
                        .confirm(
                            &t!("Never ask for this item?"),
                            &t!("Any program of this account can then take it without anyone being asked, for as long as the vault is open."),
                            &t!("Never ask"),
                            true,
                        )
                        .await;
                    if !sure {
                        this.reload_current();
                        return;
                    }
                }
                match this.call(json!({"op": "set-approval", "kind": kind, "id": id, "approval": value})).await {
                    Ok(_) => {
                        current.replace(value);
                        this.reload_current();
                    }
                    Err(trouble) => {
                        this.say(&trouble.words());
                        this.reload_current();
                    }
                }
            });
        });
        card.append(&seg);
        card
    }

    /// Whether the item goes to the sync server, shown only while one is connected: off, sealed (encrypted once more under this
    /// vault's key) or, for what syncs by default, as it is. A change is asked for, as the service asks it
    pub fn sync_card(&self, kind: &'static str, row: &Row) -> gtk::Box {
        let card = widgets::card();
        card.set_visible(false);
        card.append(&widgets::h2(&t!("Sync")));
        let (weak, id, approval, shown) = (self.me.clone(), row.id.clone(), row.approval.clone(), card.clone());
        glib::spawn_future_local(async move {
            let Some(this) = weak.upgrade() else { return };
            let Ok(status) = this.call(json!({"op": "sync-status"})).await else { return };
            if status["connected"] != json!(true) {
                return;
            }
            let by_default = amethystora_vault_core::sync::kinds::ON_BY_DEFAULT.contains(&kind);
            let current = match status["choices"][format!("{kind}.{id}")].as_str() {
                Some(choice @ ("on" | "off" | "sealed")) => choice.to_string(),
                _ if by_default && approval != "ask" => "on".to_string(),
                _ => "off".to_string(),
            };
            let this_item = format!("{kind}.{id}");
            let mut choices = vec![("off", t!("Off")), ("sealed", t!("Sealed"))];
            if by_default {
                choices.insert(0, ("on", t!("Syncs")));
            }
            let weak = this.me.clone();
            shown.append(&widgets::seg(&choices, &current, move |value| {
                let Some(this) = weak.upgrade() else { return };
                let (value, id) = (value.to_string(), id.clone());
                glib::spawn_future_local(async move {
                    if let Err(trouble) = this.call(json!({"op": "sync-choose", "kind": kind, "id": id, "choice": value})).await {
                        this.say(&trouble.words());
                    }
                    this.reload_current();
                });
            }));
            shown.append(&widgets::p(&t!("Sealed: encrypted once more with this vault's own key, so the account on the server cannot open it."), &["soft", "small"]));
            // The last run left it here: why, in the person's words
            let why = status["last"]["report"]["refused"].as_array().into_iter().flatten().find(|refused| refused["item"] == json!(this_item)).and_then(|refused| refused["why"].as_str());
            if let Some(why) = why {
                let words = match why {
                    "too-large-to-seal" => t!("It did not go: sealed, it is larger than one note on the server can hold, so it stays on this computer only."),
                    _ => t!("It did not go to the server at the last sync, and stays on this computer only."),
                };
                shown.append(&widgets::p(&words, &["warn", "small"]));
            }
            shown.set_visible(true);
        });
        card
    }

    pub fn reload_current(&self) {
        self.current_page(|page| page.reload());
    }

    pub async fn confirm(&self, heading: &str, body: &str, yes: &str, danger: bool) -> bool {
        let alert = adw::AlertDialog::new(Some(heading), Some(body));
        alert.add_responses(&[("cancel", &t!("Cancel")), ("yes", yes)]);
        alert.set_response_appearance("yes", if danger { adw::ResponseAppearance::Destructive } else { adw::ResponseAppearance::Suggested });
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        widgets::track(alert.upcast_ref());
        alert.choose_future(Some(&self.shell.window)).await == "yes"
    }

    /// The head of a detail: the item's stone, name, a line under it, and what goes at its end
    pub fn detail_head(&self, list: &Rc<ListPage>, row: &Row, sub: &str, after: &[gtk::Widget]) -> gtk::Box {
        let head = ui::hbox(14, &["detail-head"]);
        if list.narrow.get() {
            let list = list.clone();
            head.append(&widgets::icon_btn("back", &t!("Back to the list"), move || list.back()));
        }
        let avatar = widgets::Avatar::new(&row.title, row.hue);
        crate::logos::attach(&avatar, row.host.as_deref());
        head.append(&avatar);
        let words = ui::vbox(0, &[]);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        words.append(&widgets::h1(&row.title));
        if !sub.is_empty() {
            words.append(&widgets::data(sub, &["soft", "small"]));
        }
        head.append(&words);
        for widget in after {
            head.append(widget);
        }
        head
    }

    /// Where a detail's cards go
    pub fn detail_body() -> gtk::Box {
        ui::vbox(16, &["detail"])
    }

    /// A secret on the clipboard, and said so
    pub fn copy_secret(&self, text: &[u8]) {
        secret::copy(zeroize::Zeroizing::new(text.to_vec()));
        self.say(&t!("Copied. It leaves the clipboard in 30 seconds."));
    }

    /// The page shown read again, with this item chosen once it is there
    pub fn reload_and_select(&self, id: Option<String>) {
        let page = self.pages.borrow().get(&self.section.get()).cloned();
        let Some(page) = page else { return };
        page.reload();
        // The list comes back from the service in its own time: the page chooses the item when it is there
        if let Some(id) = id {
            page.select(&id);
        }
    }

    /// Which open vault something new goes in: the only one that is open, or the one chosen from a list that starts at the first vault
    /// (never a hidden one unless it is chosen). Every write names it, so nothing lands anywhere by accident.
    pub fn target(&self) -> Rc<Target> {
        let status = self.status.borrow();
        let open: Vec<(String, String, bool, bool)> = status["vaults"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|vault| vault["open"] == json!(true))
            .map(|vault| (vault["id"].as_str().unwrap_or("").to_string(), widgets::clean(vault["name"].as_str().unwrap_or("")), vault["first"] == json!(true), vault["hidden"] == json!(true)))
            .collect();
        let start = open.iter().position(|(_, _, first, _)| *first).or_else(|| open.iter().position(|(_, _, _, hidden)| !hidden)).unwrap_or(0);
        let choice = (open.len() > 1).then(|| {
            let names: Vec<&str> = open.iter().map(|(_, name, _, _)| name.as_str()).collect();
            let choice = gtk::DropDown::from_strings(&names);
            choice.set_selected(start as u32);
            choice
        });
        Rc::new(Target { choice, ids: open.into_iter().map(|(id, ..)| id).collect(), start })
    }
}

/// The vault something new is written to. With one open it is that one, and no question is asked.
pub struct Target {
    choice: Option<gtk::DropDown>,
    ids: Vec<String>,
    start: usize,
}

impl Target {
    /// The vault's id, for a request's `vault`
    pub fn id(&self) -> Option<String> {
        let at = self.choice.as_ref().map_or(self.start, |choice| choice.selected() as usize);
        self.ids.get(at).cloned()
    }

    /// The question that goes in a form, when there is more than one vault to ask about
    pub fn field(&self) -> Option<gtk::Box> {
        self.choice.as_ref().map(|choice| widgets::field(&t!("In which vault"), choice, &[]))
    }

    /// Names the vault in a request that writes
    pub fn name(&self, request: &mut Value) {
        if let Some(id) = self.id() {
            request["vault"] = json!(id);
        }
    }
}
