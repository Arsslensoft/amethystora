//! The questions and forms laid over the window: sending files or a link, answering a notification, the text a
//! phone shared, and the confirmation before a phone is unpaired. They are layers over the page, as the other
//! apps' are, and close with Escape or a click beside them.
//!
//! What goes to a phone is chosen here and checked again by the program: files by their file: addresses, which
//! the program refuses for a folder; a link only if it is a web, mail or telephone address; a reply as the text
//! typed. What came from a phone (a notification, a shared text) is shown as text in a label, with web addresses
//! made links that open in the browser when clicked, and nothing else.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amethystora_ui::{gio, glib, gtk, process, t, ui};
use gtk::prelude::*;

use crate::cli::Sending;
use crate::icons::icon;
use crate::link::Repliable;
use crate::model::{self, bdi, Transfer};
use crate::widgets::{self, Look};
use crate::{failure_text, Phone};

/// Which part of the send dialog shows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Files,
    Link,
}

/// A file chosen to be sent
#[derive(Debug, Clone)]
pub struct Picked {
    pub uri: String,
    pub name: String,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// Choosing what to send, and to which phone
    Choosing,
    /// Sent: these transfers, in the order of the files
    Sending(Vec<String>),
    /// A link went
    Linked,
}

/// The widgets of one file's progress, to change in place as the transfer moves
struct ProgressRow {
    status: gtk::Label,
    bar: ui::Bar,
    cancel: gtk::Button,
}

pub struct SendState {
    pub device: Option<String>,
    pub tab: Tab,
    pub files: Vec<Picked>,
    pub link: String,
    pub phase: Phase,
    pub error: Option<String>,
    pub note: Option<String>,
    sending: bool,
    rows: HashMap<String, ProgressRow>,
}

/// Where the notification to answer is, as it is looked for
#[derive(Debug, Clone)]
pub enum Found {
    Looking,
    There(Repliable),
    Gone,
}

pub struct ReplyState {
    pub device: String,
    pub notification: String,
    pub found: Found,
    pub text: String,
    pub error: Option<String>,
    sending: bool,
    /// The text the phone was given, while the card says so before it goes
    sent: Option<String>,
}

/// A layer over the window
#[derive(Clone)]
pub enum Dialog {
    Unpair(String),
    Send(Rc<RefCell<SendState>>),
    Reply(Rc<RefCell<ReplyState>>),
    Text { device: String, text: String },
}

/// At most this many files in one go
const MOST_FILES: usize = 100;

impl Phone {
    // --- The layer -------------------------------------------------------------------------------------

    pub(crate) fn close_dialog(&self) {
        self.state.borrow_mut().dialog = None;
        self.render_dialog();
    }

    fn set_dialog(&self, dialog: Dialog) {
        self.state.borrow_mut().dialog = Some(dialog);
        self.render_dialog();
    }

    pub(crate) fn render_dialog(&self) {
        if let Some(layer) = self.dialog_layer.take() {
            self.parts.root.remove_overlay(&layer);
        }
        let behind = self.parts.root.child().expect("the window");
        let Some(dialog) = self.state.borrow().dialog.clone() else {
            // What is behind it is the page again, to a screen reader too
            behind.update_state(&[gtk::accessible::State::Hidden(false)]);
            return;
        };
        let (body, focus) = match &dialog {
            Dialog::Unpair(id) => self.unpair_body(id),
            Dialog::Send(st) => self.send_body(st),
            Dialog::Reply(st) => self.reply_body(st),
            Dialog::Text { device, text } => self.text_body(device, text),
        };
        body.update_property(&[gtk::accessible::Property::Modal(true)]);
        // The reply is the board's card, 380 wide; the others are forms
        let width = ui::limit_center(&body, if matches!(dialog, Dialog::Reply(_)) { 380 } else { 560 });
        width.add_css_class("dialog-width");
        // In the middle of the window
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
        let (weak, target, question) = (self.weak(), back.downgrade(), body.downgrade());
        click.connect_released(move |gesture, _, x, y| {
            let (Some(this), Some(back), Some(question)) = (weak.upgrade(), target.upgrade(), question.upgrade()) else { return };
            let inside = back.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|widget| widget == question || widget.is_ancestor(&question));
            if !inside {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                this.close_dialog();
            }
        });
        back.add_controller(click);
        let layer = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).css_classes(["page-scroll"]).child(&back).build();
        self.parts.root.add_overlay(&layer);
        behind.update_state(&[gtk::accessible::State::Hidden(true)]);
        self.dialog_layer.replace(Some(layer.upcast()));
        if let Some(focus) = focus {
            focus.grab_focus();
        }
    }

    fn dialog_box() -> gtk::Box {
        widgets::role_box(gtk::Orientation::Vertical, 14, &["dialog"], gtk::AccessibleRole::Dialog)
    }

    fn dialog_actions(buttons: Vec<gtk::Button>) -> amethystora_ui::adw::WrapBox {
        widgets::actions(buttons.into_iter().map(|b| b.upcast()).collect(), &["actions", "end"])
    }

    /// The phones change under an open dialog: it is drawn again for the ones that depend on them
    pub(crate) fn dialogs_devices_changed(&self) {
        let dialog = self.state.borrow().dialog.clone();
        match dialog {
            Some(Dialog::Send(st)) => {
                let choosing = st.borrow().phase == Phase::Choosing;
                if choosing {
                    self.render_dialog();
                }
            }
            Some(Dialog::Reply(_)) | Some(Dialog::Text { .. }) | Some(Dialog::Unpair(_)) => self.render_dialog(),
            None => {}
        }
    }

    // --- Unpair ----------------------------------------------------------------------------------------

    pub(crate) fn open_unpair(self: &Rc<Self>, id: &str) {
        self.set_dialog(Dialog::Unpair(id.to_string()));
    }

    fn unpair_body(&self, id: &str) -> (gtk::Box, Option<gtk::Widget>) {
        let name = self.link.device(id).map(|d| bdi(&d.title())).unwrap_or_default();
        let body = Self::dialog_box();
        body.append(&widgets::heading(&t!("Unpair {phone}?", phone = name), "h2"));
        body.append(&ui::label(&t!("The phone will no longer see this computer, and its notifications, texts and files stop coming. To use it again, pair it again and compare the key on both screens."), &["soft"]));
        let id = id.to_string();
        let unpair = self.button(&t!("Unpair"), Look { primary: true, danger: true, icon: Some("unlink"), ..Default::default() }, move |this| {
            this.close_dialog();
            this.unpair_confirmed(&id);
        });
        body.append(&Self::dialog_actions(vec![self.button(&t!("Cancel"), Look::default(), |this| this.close_dialog()), unpair.clone()]));
        (body, Some(unpair.upcast()))
    }

    // --- Send ------------------------------------------------------------------------------------------

    /// The send dialog, over the page, with what the command line gave it. With `now` (the dock's drop), files for
    /// the phone named are sent at once, as the Send button would, and the window is shown only when something needs
    /// the person: no such phone, a file left out, a folder, a link.
    pub(crate) fn open_send(self: &Rc<Self>, device: Option<String>, what: Sending, now: bool) {
        let sendable = model::sendable(&self.link.devices());
        let named = device.filter(|id| sendable.iter().any(|d| &d.id == id));
        let chosen = named.clone().or_else(|| (sendable.len() == 1).then(|| sendable[0].id.clone()));
        let st = Rc::new(RefCell::new(SendState {
            device: chosen,
            tab: Tab::Files,
            files: Vec::new(),
            link: String::new(),
            phase: Phase::Choosing,
            error: None,
            note: None,
            sending: false,
            rows: HashMap::new(),
        }));
        match &what {
            Sending::Link(link) => {
                let mut st = st.borrow_mut();
                st.tab = Tab::Link;
                st.link = link.clone();
            }
            Sending::Files(uris) => {
                let (this, st, uris) = (self.clone(), st.clone(), uris.clone());
                let at_once = now && named.is_some();
                glib::spawn_future_local(async move {
                    this.add_files(&st, &uris).await;
                    let whole = {
                        let s = st.borrow();
                        s.note.is_none() && !s.files.is_empty() && s.files.len() == uris.len()
                    };
                    if at_once && whole {
                        this.send_at_once(&st);
                    } else if now {
                        this.shell.window.present();
                    }
                });
            }
            Sending::Nothing => {}
        }
        if now && !matches!(what, Sending::Files(_)) {
            self.shell.window.present();
        }
        self.set_dialog(Dialog::Send(st));
    }

    /// The dock's drop: sent as the Send button sends, without the window; it is shown if the sending fails. A window
    /// that was never shown has done its work once the program has the files, and closes.
    fn send_at_once(self: &Rc<Self>, st: &Rc<RefCell<SendState>>) {
        let (this, st) = (self.clone(), st.clone());
        glib::spawn_future_local(async move {
            this.send(&st).await;
            let sent = matches!(st.borrow().phase, Phase::Sending(_));
            if !sent {
                this.shell.window.present();
            } else if !this.shell.window.is_visible() {
                this.shell.app.quit();
            }
        });
    }

    /// Reads what each file is, and keeps those that can be sent: regular files this account can read
    async fn add_files(self: &Rc<Self>, st: &Rc<RefCell<SendState>>, uris: &[String]) {
        let mut folders = 0;
        let mut unreadable = 0;
        let mut found = Vec::new();
        for uri in uris {
            let file = gio::File::for_uri(uri);
            if !file.is_native() {
                unreadable += 1;
                continue;
            }
            let attributes = "standard::display-name,standard::size,standard::type";
            match file.query_info_future(attributes, gio::FileQueryInfoFlags::NONE, glib::Priority::DEFAULT).await {
                Ok(info) if info.file_type() == gio::FileType::Regular => {
                    found.push(Picked { uri: file.uri().to_string(), name: model::clean(&info.display_name(), 255), size: info.size() });
                }
                Ok(info) if info.file_type() == gio::FileType::Directory => folders += 1,
                _ => unreadable += 1,
            }
        }
        {
            let mut st = st.borrow_mut();
            for picked in found {
                if st.files.len() < MOST_FILES && !st.files.iter().any(|f| f.uri == picked.uri) {
                    st.files.push(picked);
                }
            }
            st.tab = Tab::Files;
            st.note = match (folders, unreadable) {
                (0, 0) => None,
                (folders, 0) => Some(t!("{count, plural, one {A folder cannot be sent: send the files in it.} other {# folders cannot be sent: send the files in them.}}", count = folders)),
                (_, unreadable) => Some(t!("{count, plural, one {One item could not be read, and was left out.} other {# items could not be read, and were left out.}}", count = unreadable)),
            };
        }
        self.redraw_send(st);
    }

    /// Draws the send dialog again if it is the one open
    fn redraw_send(&self, st: &Rc<RefCell<SendState>>) {
        let same = matches!(&self.state.borrow().dialog, Some(Dialog::Send(open)) if Rc::ptr_eq(open, st));
        if same {
            self.render_dialog();
        }
    }

    fn choose_files(self: &Rc<Self>, st: &Rc<RefCell<SendState>>) {
        let (this, st) = (self.clone(), st.clone());
        glib::spawn_future_local(async move {
            let chooser = gtk::FileDialog::builder().title(t!("Choose files to send")).accept_label(t!("Choose")).modal(true).build();
            let window = this.shell.gtk_window();
            let Ok(chosen) = chooser.open_multiple_future(Some(&window)).await else { return };
            let uris: Vec<String> = (0..chosen.n_items()).filter_map(|at| chosen.item(at).and_downcast::<gio::File>()).map(|file| file.uri().to_string()).collect();
            this.add_files(&st, &uris).await;
        });
    }

    fn send_body(&self, st: &Rc<RefCell<SendState>>) -> (gtk::Box, Option<gtk::Widget>) {
        let phase = st.borrow().phase.clone();
        match phase {
            Phase::Choosing => self.send_choosing(st),
            Phase::Sending(ids) => self.send_progress(st, &ids),
            Phase::Linked => self.send_linked(st),
        }
    }

    fn send_choosing(&self, st: &Rc<RefCell<SendState>>) -> (gtk::Box, Option<gtk::Widget>) {
        let sendable = model::sendable(&self.link.devices());
        // The phone chosen goes away: the choice goes with it
        {
            let mut state = st.borrow_mut();
            if state.device.as_ref().is_some_and(|id| !sendable.iter().any(|d| &d.id == id)) {
                state.device = None;
            }
            if state.device.is_none() && sendable.len() == 1 {
                state.device = Some(sendable[0].id.clone());
            }
        }
        let (device, tab, files, link, error, note) = {
            let s = st.borrow();
            (s.device.clone(), s.tab, s.files.clone(), s.link.clone(), s.error.clone(), s.note.clone())
        };
        let body = Self::dialog_box();
        let target = device.as_ref().and_then(|id| sendable.iter().find(|d| &d.id == id));
        body.append(&widgets::heading(
            &match target {
                Some(device) => t!("Send to {phone}", phone = bdi(&device.title())),
                None => t!("Send to a phone"),
            },
            "h2",
        ));
        if sendable.is_empty() {
            body.append(&widgets::callout("check", &t!("No phone can take files now"), &t!("Pair a phone and connect it first. The phones that take files are listed on the Phones page.")));
            let close = self.button(&t!("Close"), Look::default(), |this| this.close_dialog());
            body.append(&Self::dialog_actions(vec![close.clone()]));
            return (body, Some(close.upcast()));
        }
        // Who is nearby, when there is more than one: round avatars, the one chosen ringed (R7-27)
        if sendable.len() > 1 {
            // A row that wraps, so that a name has the room to be read and a dozen phones are two rows
            let options = amethystora_ui::adw::WrapBox::builder().child_spacing(6).line_spacing(6).accessible_role(gtk::AccessibleRole::RadioGroup).css_classes(["nearby"]).build();
            options.set_halign(gtk::Align::Center);
            for phone in &sendable {
                let chosen = device.as_deref() == Some(phone.id.as_str());
                let content = ui::vbox(6, &[]);
                let disc = ui::hbox(0, &["avatar", "nearby-disc"]);
                disc.set_halign(gtk::Align::Center);
                disc.set_hexpand(false);
                let mark = crate::icons::device(&phone.kind, 26);
                mark.set_hexpand(true);
                disc.append(&mark);
                content.append(&disc);
                let name = ui::line(&phone.title(), &["strong", "small"]);
                name.set_wrap(true);
                name.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                name.set_lines(2);
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                name.set_max_width_chars(12);
                name.set_justify(gtk::Justification::Center);
                name.set_halign(gtk::Align::Center);
                content.append(&name);
                let option = gtk::Button::builder().accessible_role(gtk::AccessibleRole::Radio).child(&content).width_request(96).css_classes(["plain", "nearby-pick"]).build();
                if chosen {
                    option.add_css_class("checked");
                }
                option.update_state(&[gtk::accessible::State::Checked(if chosen { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
                let (id, st, weak) = (phone.id.clone(), st.clone(), self.weak());
                option.connect_clicked(move |_| {
                    st.borrow_mut().device = Some(id.clone());
                    if let Some(this) = weak.upgrade() {
                        this.render_dialog();
                    }
                });
                options.append(&option);
            }
            body.append(&options);
        }

        // Files or a link
        let tabs = ui::hbox(0, &["tabs"]);
        for (which, label, glyph) in [(Tab::Files, t!("Files"), "file"), (Tab::Link, t!("A link"), "link")] {
            let content = ui::hbox(8, &[]);
            content.append(&icon(glyph, 16));
            content.append(&ui::line(&label, &[]));
            let button = gtk::Button::builder().child(&content).css_classes(["plain", "tab"]).build();
            if tab == which {
                button.add_css_class("current");
            }
            let (st, weak) = (st.clone(), self.weak());
            button.connect_clicked(move |_| {
                {
                    let mut state = st.borrow_mut();
                    state.tab = which;
                    state.error = None;
                }
                if let Some(this) = weak.upgrade() {
                    this.render_dialog();
                }
            });
            tabs.append(&button);
        }
        body.append(&tabs);

        let send_button = self.button(&t!("Send"), Look { primary: true, icon: Some("send"), disabled: device.is_none() || (tab == Tab::Files && files.is_empty()) || (tab == Tab::Link && model::sendable_link(&link).is_none()), ..Default::default() }, {
            let st = st.clone();
            move |this| this.send_now(&st)
        });
        let focus: Option<gtk::Widget>;
        match tab {
            Tab::Files => {
                if files.is_empty() {
                    body.append(&ui::label(&t!("Choose the files to send. A folder cannot be sent, only the files in it."), &["soft"]));
                } else {
                    let list = ui::vbox(0, &["picked"]);
                    for (at, picked) in files.iter().enumerate() {
                        let row = ui::hbox(10, &["picked-row"]);
                        row.append(&icon("file", 18));
                        let name = ui::line(&picked.name, &["grow"]);
                        name.set_hexpand(true);
                        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                        row.append(&name);
                        row.append(&ui::line(&model::size(picked.size), &["faint", "small"]));
                        let (st, weak) = (st.clone(), self.weak());
                        row.append(&widgets::icon_button("close", 14, &t!("Leave out"), move || {
                            st.borrow_mut().files.remove(at);
                            if let Some(this) = weak.upgrade() {
                                this.render_dialog();
                            }
                        }));
                        list.append(&row);
                    }
                    let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).max_content_height(220).propagate_natural_height(true).css_classes(["page-scroll", "picked-scroll"]).child(&list).build();
                    body.append(&scroller);
                }
                if let Some(note) = note {
                    body.append(&ui::label(&note, &["faint", "small"]));
                }
                let choose = self.button(
                    &if files.is_empty() { t!("Choose files…") } else { t!("Add more files…") },
                    Look { icon: Some("plus"), primary: files.is_empty(), ..Default::default() },
                    {
                        let st = st.clone();
                        move |this| this.choose_files(&st)
                    },
                );
                body.append(&widgets::actions(vec![choose.clone().upcast()], &["actions"]));
                focus = Some(if files.is_empty() { choose.upcast() } else { send_button.clone().upcast() });
            }
            Tab::Link => {
                let entry = gtk::Entry::builder().placeholder_text("https://").text(&link).hexpand(true).css_classes(["field-entry"]).input_purpose(gtk::InputPurpose::Url).build();
                entry.update_property(&[gtk::accessible::Property::Label(&t!("The link to send"))]);
                entry.set_direction(gtk::TextDirection::Ltr);
                let (state, button) = (st.clone(), send_button.clone());
                entry.connect_changed(move |entry| {
                    let text = entry.text().to_string();
                    button.set_sensitive(model::sendable_link(&text).is_some());
                    state.borrow_mut().link = text;
                });
                let weak = self.weak();
                let state = st.clone();
                entry.connect_activate(move |_| {
                    if let Some(this) = weak.upgrade() {
                        this.send_now(&state);
                    }
                });
                body.append(&entry);
                body.append(&ui::label(&t!("The phone opens it with whatever handles web, mail or telephone addresses."), &["faint", "small"]));
                focus = Some(entry.upcast());
            }
        }
        if let Some(error) = error {
            body.append(&widgets::callout("failed", "", &error));
        }
        body.append(&Self::dialog_actions(vec![self.button(&t!("Cancel"), Look::default(), |this| this.close_dialog()), send_button]));
        (body, focus)
    }

    /// Sends what was chosen: the transfers begin, or the program says why not
    fn send_now(self: &Rc<Self>, st: &Rc<RefCell<SendState>>) {
        let (this, st) = (self.clone(), st.clone());
        glib::spawn_future_local(async move { this.send(&st).await });
    }

    /// What the Send button does, done
    async fn send(self: &Rc<Self>, st: &Rc<RefCell<SendState>>) {
        {
            let mut state = st.borrow_mut();
            if state.sending || state.device.is_none() {
                return;
            }
            state.sending = true;
            state.error = None;
        }
        let this = self;
        {
            let (device, tab, files, link) = {
                let s = st.borrow();
                (s.device.clone().unwrap_or_default(), s.tab, s.files.clone(), s.link.clone())
            };
            let result = match tab {
                Tab::Files => {
                    let uris: Vec<String> = files.iter().map(|f| f.uri.clone()).collect();
                    this.link.send_files(&device, &uris).await.map(Phase::Sending)
                }
                Tab::Link => match model::sendable_link(&link) {
                    Some(url) => this.link.send_url(&device, &url).await.map(|()| Phase::Linked),
                    None => Ok(Phase::Choosing),
                },
            };
            {
                let mut state = st.borrow_mut();
                state.sending = false;
                match result {
                    Ok(Phase::Choosing) => state.error = Some(t!("That does not look like a web, mail or telephone address.")),
                    Ok(phase) => state.phase = phase,
                    Err(failure) => state.error = Some(failure_text(&failure)),
                }
            }
            this.redraw_send(st);
        }
    }

    fn send_progress(&self, st: &Rc<RefCell<SendState>>, ids: &[String]) -> (gtk::Box, Option<gtk::Widget>) {
        let (device, files) = {
            let s = st.borrow();
            (s.device.clone().unwrap_or_default(), s.files.clone())
        };
        let phone = self.link.device(&device).map(|d| bdi(&d.title())).unwrap_or_default();
        let body = Self::dialog_box();
        body.append(&widgets::heading(&t!("Sending to {phone}", phone = phone), "h2"));
        let list = ui::vbox(0, &["picked"]);
        let mut rows = HashMap::new();
        for (at, id) in ids.iter().enumerate() {
            let name = files.get(at).map(|f| f.name.clone()).unwrap_or_default();
            let row = ui::vbox(6, &["picked-row", "progress-row"]);
            let line = ui::hbox(10, &[]);
            line.append(&icon("upload", 18));
            let words = ui::line(&name, &["grow"]);
            words.set_hexpand(true);
            words.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            line.append(&words);
            let status = ui::line(&t!("Waiting…"), &["faint", "small"]);
            line.append(&status);
            let (weak, device, transfer) = (self.weak(), device.clone(), id.clone());
            let cancel = widgets::icon_button("close", 14, &t!("Cancel this file"), move || {
                if let Some(this) = weak.upgrade() {
                    let (device, transfer) = (device.clone(), transfer.clone());
                    glib::spawn_future_local(async move {
                        let _ = this.link.cancel_transfer(&device, &transfer).await;
                    });
                }
            });
            line.append(&cancel);
            row.append(&line);
            let bar = ui::Bar::new();
            row.append(&bar);
            list.append(&row);
            let progress = ProgressRow { status, bar, cancel };
            if let Some(transfer) = self.link.transfer(id) {
                update_row(&progress, &transfer);
            }
            rows.insert(id.clone(), progress);
        }
        st.borrow_mut().rows = rows;
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).max_content_height(320).propagate_natural_height(true).css_classes(["page-scroll", "picked-scroll"]).child(&list).build();
        body.append(&scroller);
        body.append(&ui::label(&t!("You can close this: the files keep going."), &["faint", "small"]));
        let done = self.button(&t!("Done"), Look { primary: true, icon: Some("check"), ..Default::default() }, |this| this.close_dialog());
        body.append(&Self::dialog_actions(vec![done.clone()]));
        (body, Some(done.upcast()))
    }

    fn send_linked(&self, st: &Rc<RefCell<SendState>>) -> (gtk::Box, Option<gtk::Widget>) {
        let (device, link) = {
            let s = st.borrow();
            (s.device.clone().unwrap_or_default(), s.link.trim().to_string())
        };
        let phone = self.link.device(&device).map(|d| bdi(&d.title())).unwrap_or_default();
        let body = Self::dialog_box();
        body.append(&widgets::heading(&t!("Link sent"), "h2"));
        body.append(&ui::label(&t!("{phone} has the link. It opens it with whatever handles it.", phone = phone), &["soft"]));
        let shown = ui::label(&model::clean(&link, 512), &["code", "quoted"]);
        shown.set_selectable(true);
        shown.set_direction(gtk::TextDirection::Ltr);
        body.append(&shown);
        let done = self.button(&t!("Done"), Look { primary: true, icon: Some("check"), ..Default::default() }, |this| this.close_dialog());
        body.append(&Self::dialog_actions(vec![done.clone()]));
        (body, Some(done.upcast()))
    }

    /// A transfer moved on: the row of the open send dialog that has it changes in place
    pub(crate) fn dialog_transfer(&self, transfer: &Transfer) {
        let Some(Dialog::Send(st)) = self.state.borrow().dialog.clone() else { return };
        let state = st.borrow();
        if let Some(row) = state.rows.get(&transfer.id) {
            update_row(row, transfer);
        }
    }

    // --- Reply -----------------------------------------------------------------------------------------

    pub(crate) fn open_reply(self: &Rc<Self>, device: &str, notification: &str) {
        let st = Rc::new(RefCell::new(ReplyState { device: device.to_string(), notification: notification.to_string(), found: Found::Looking, text: String::new(), error: None, sending: false, sent: None }));
        self.set_dialog(Dialog::Reply(st.clone()));
        let (this, device, notification) = (self.clone(), device.to_string(), notification.to_string());
        glib::spawn_future_local(async move {
            let found = match this.link.repliable(&device).await {
                Ok(list) => list.into_iter().find(|n| n.id == notification).map_or(Found::Gone, Found::There),
                Err(_) => Found::Gone,
            };
            st.borrow_mut().found = found;
            let same = matches!(&this.state.borrow().dialog, Some(Dialog::Reply(open)) if Rc::ptr_eq(open, &st));
            if same {
                this.render_dialog();
            }
        });
    }

    fn reply_body(&self, st: &Rc<RefCell<ReplyState>>) -> (gtk::Box, Option<gtk::Widget>) {
        let (device_id, found, text, error, sent) = {
            let s = st.borrow();
            (s.device.clone(), s.found.clone(), s.text.clone(), s.error.clone(), s.sent.clone())
        };
        let device = self.link.device(&device_id);
        let phone = device.as_ref().map(|d| bdi(&d.title())).unwrap_or_else(|| t!("the phone"));
        let connected = device.as_ref().is_some_and(|d| d.paired && d.connected);
        if let Found::There(notification) = &found {
            return self.reply_card(st, notification, connected, &phone, &text, error, sent);
        }
        // Still looking, or the notification is gone: the card waits, or says so
        let body = Self::dialog_box();
        body.append(&widgets::heading(&t!("Reply to {app}", app = t!("a notification")), "h2"));
        body.append(&ui::label(&t!("From {phone}", phone = phone), &["faint", "small"]));
        if let Found::Gone = &found {
            body.append(&widgets::callout("check", &t!("That notification is gone"), &t!("It was answered or dismissed on the phone, or the phone is not connected. There is nothing to reply to.")));
        } else {
            let working = widgets::role_box(gtk::Orientation::Horizontal, 10, &["working"], gtk::AccessibleRole::Status);
            let mark = icon("spinner", 18);
            mark.set_spinning(true);
            working.append(&mark);
            working.append(&ui::label(&t!("Asking the phone…"), &[]));
            body.append(&working);
        }
        body.append(&Self::dialog_actions(vec![self.button(&t!("Cancel"), Look::default(), |this| this.close_dialog())]));
        (body, None)
    }

    /// The reply as a card (board 32, third card): who wrote and what, then a line to answer on and Send; once sent,
    /// what the phone was given, and the card goes by itself. The protocol has no word back from the phone, so the
    /// card says the phone is sending it, not that it was sent.
    #[allow(clippy::too_many_arguments)]
    fn reply_card(&self, st: &Rc<RefCell<ReplyState>>, notification: &Repliable, connected: bool, phone: &str, text: &str, error: Option<String>, sent: Option<String>) -> (gtk::Box, Option<gtk::Widget>) {
        let body = Self::dialog_box();
        body.add_css_class("reply-card");
        body.set_spacing(10);
        let sender = if notification.title.trim().is_empty() { notification.app.trim() } else { notification.title.trim() };
        let sender = model::clean(sender, model::MOST_NAME).replace('\n', " ");
        let top = ui::hbox(10, &[]);
        // The app's icon from the phone, when the program kept one; the message glyph otherwise
        let avatar = ui::hbox(0, &["reply-avatar"]);
        match &notification.icon {
            Some(path) => {
                avatar.add_css_class("picture");
                avatar.set_overflow(gtk::Overflow::Hidden);
                let picture = gtk::Image::from_file(path);
                picture.set_pixel_size(30);
                avatar.append(&picture);
            }
            None => {
                let mark = icon("message", 16);
                mark.set_hexpand(true);
                mark.set_valign(gtk::Align::Center);
                avatar.append(&mark);
            }
        }
        avatar.set_valign(gtk::Align::Start);
        top.append(&avatar);
        let words = ui::vbox(0, &[]);
        words.set_hexpand(true);
        // A notification that names neither who wrote nor its app is still the phone's
        let (heading, hint) = if sender.trim().is_empty() {
            (t!("A message from your phone"), t!("Write a reply"))
        } else {
            (t!("{name} · from your phone", name = bdi(&sender)), t!("Reply to {name}", name = bdi(&sender)))
        };
        let title = ui::label(&heading, &["reply-title"]);
        title.set_xalign(0.0);
        words.append(&title);
        if !notification.text.trim().is_empty() {
            let message = self.linked_label(&notification.text);
            message.add_css_class("reply-message");
            words.append(&message);
        }
        top.append(&words);
        body.append(&top);
        if let Some(said) = sent {
            let done = widgets::role_box(gtk::Orientation::Horizontal, 8, &["reply-sent"], gtk::AccessibleRole::Status);
            done.append(&amethystora_ui::icon::Icon::new(crate::icons::data("check"), 16, 3.0));
            let words = ui::label(&t!("Your phone is sending it: “{text}”", text = said), &[]);
            words.set_xalign(0.0);
            words.set_hexpand(true);
            done.append(&words);
            body.append(&done);
            return (body, None);
        }
        let row = ui::hbox(8, &["reply-row"]);
        let entry = gtk::Entry::builder().placeholder_text(&hint).text(text).hexpand(true).css_classes(["field-entry"]).build();
        entry.update_property(&[gtk::accessible::Property::Label(&hint)]);
        entry.set_sensitive(connected);
        let send = self.button(&t!("Send"), Look { primary: true, disabled: !connected || text.trim().is_empty(), ..Default::default() }, {
            let st = st.clone();
            move |this| this.reply_now(&st)
        });
        send.add_css_class("reply-send");
        {
            let (state, button, weak) = (st.clone(), send.clone(), self.weak());
            entry.connect_changed(move |entry| {
                let text = entry.text().to_string();
                state.borrow_mut().text = text.clone();
                let connected = weak.upgrade().and_then(|this| this.link.device(&state.borrow().device)).is_some_and(|d| d.paired && d.connected);
                button.set_sensitive(connected && !text.trim().is_empty());
            });
            let (state, weak) = (st.clone(), self.weak());
            entry.connect_activate(move |_| {
                if let Some(this) = weak.upgrade().filter(|_| !state.borrow().text.trim().is_empty()) {
                    this.reply_now(&state);
                }
            });
        }
        row.append(&entry);
        row.append(&send);
        body.append(&row);
        if !connected {
            body.append(&widgets::callout("check", "", &t!("{phone} is not connected, so a reply cannot be sent now.", phone = phone.to_string())));
        }
        if let Some(error) = error {
            body.append(&widgets::callout("failed", "", &error));
        }
        (body, Some(entry.upcast()))
    }

    fn reply_now(self: &Rc<Self>, st: &Rc<RefCell<ReplyState>>) {
        let (device, notification, text) = {
            let mut s = st.borrow_mut();
            if s.sending || s.text.trim().is_empty() {
                return;
            }
            s.sending = true;
            s.error = None;
            (s.device.clone(), s.notification.clone(), s.text.clone())
        };
        let (this, st) = (self.clone(), st.clone());
        glib::spawn_future_local(async move {
            let result = this.link.reply_to_notification(&device, &notification, &text).await;
            st.borrow_mut().sending = false;
            match result {
                Ok(()) => {
                    st.borrow_mut().sent = Some(text);
                    this.render_dialog();
                    // The card says it for two seconds, then goes, unless the person closed it or opened another
                    glib::timeout_future(std::time::Duration::from_secs(2)).await;
                    let same = matches!(&this.state.borrow().dialog, Some(Dialog::Reply(open)) if Rc::ptr_eq(open, &st));
                    if same {
                        this.close_dialog();
                    }
                }
                Err(failure) => {
                    st.borrow_mut().error = Some(failure_text(&failure));
                    this.render_dialog();
                }
            }
        });
    }

    // --- Text a phone shared ---------------------------------------------------------------------------

    pub(crate) fn open_text(self: &Rc<Self>, device: &str, text: &str) {
        self.set_dialog(Dialog::Text { device: device.to_string(), text: model::clean(text, 64 * 1024) });
    }

    fn text_body(&self, device: &str, text: &str) -> (gtk::Box, Option<gtk::Widget>) {
        let phone = self.link.device(device).map(|d| bdi(&d.title()));
        let link = model::only_link(text);
        let body = Self::dialog_box();
        body.append(&widgets::heading(
            &match (&phone, &link) {
                (Some(phone), Some(_)) => t!("{phone} shared a link", phone = phone),
                (Some(phone), None) => t!("{phone} shared this text", phone = phone),
                (None, Some(_)) => t!("A phone shared a link"),
                (None, None) => t!("A phone shared this text"),
            },
            "h2",
        ));
        let label = self.linked_label(text);
        label.set_selectable(true);
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).overlay_scrolling(false).max_content_height(340).propagate_natural_height(true).css_classes(["page-scroll", "shared-text"]).child(&label).build();
        body.append(&scroller);
        body.append(&ui::label(&t!("Nothing is opened for you. A web address opens in the browser when you click it."), &["faint", "small"]));
        let copy_text = text.to_string();
        let copy = ui::copy_button(&copy_text, icon("copy", 16), &t!("Copy"), &t!("Copied"), &t!("Copy the text"), &["btn"]);
        let mut buttons = vec![copy];
        if let Some(url) = link.filter(|url| model::openable(url)) {
            let window = self.shell.gtk_window();
            buttons.push(self.button(&t!("Open the link"), Look { icon: Some("external"), ..Default::default() }, move |_| process::open_uri(Some(&window), &url)));
        }
        let close = self.button(&t!("Close"), Look { primary: true, ..Default::default() }, |this| this.close_dialog());
        buttons.push(close.clone());
        body.append(&Self::dialog_actions(buttons));
        (body, Some(close.upcast()))
    }

    /// A phone's words in a label: all of them text, with the web addresses in them links that open in the
    /// browser when they are clicked
    pub(crate) fn linked_label(&self, text: &str) -> gtk::Label {
        let label = gtk::Label::builder()
            .use_markup(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .natural_wrap_mode(gtk::NaturalWrapMode::None)
            .xalign(0.0)
            .halign(gtk::Align::Fill)
            .selectable(true)
            .build();
        label.set_markup(&model::linkify_markup(text));
        ui::no_hyphens(&label);
        ui::link_accessibles(&label);
        let window = self.shell.gtk_window();
        label.connect_activate_link(move |_, uri| {
            if model::openable(uri) {
                process::open_uri(Some(&window), uri);
            }
            glib::Propagation::Stop
        });
        label
    }
}

/// A row of the progress list as the transfer says it is
fn update_row(row: &ProgressRow, transfer: &Transfer) {
    row.status.set_label(&transfer.status());
    row.bar.set_fraction(if transfer.running() { transfer.fraction() } else { transfer.fraction().or(Some(0.0)) });
    row.cancel.set_visible(transfer.running());
    row.status.remove_css_class("bad");
    if transfer.state == "failed" {
        row.status.add_css_class("bad");
    }
}
