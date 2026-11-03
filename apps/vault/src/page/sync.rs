//! Sync with a Vaultwarden or Bitwarden server. Connecting asks for a password and says what it costs, so it is `ame vault sync connect`
//! in a terminal, and so is accepting what the service stopped to ask about (`ame vault sync now accept`). Hidden vaults are never
//! offered it. The page shows what `sync-status` answers: `connected`, and then `address`, `email`, `linked`, `running`,
//! `last_success_ms`, `next_in_seconds` and `problem`, which is `{name, detail}`. Without a connection, or while the service has not
//! answered, it says "Not connected".

use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gtk, i18n, process, t, ui};
use serde_json::{json, Value};

use super::{Page, Vault, Width};
use crate::list;
use crate::widgets::{self, btn, say};

pub struct SyncPage {
    vault: Rc<Vault>,
    root: gtk::ScrolledWindow,
    head_chip: gtk::Label,
    server: gtk::Box,
    conflicts: gtk::Box,
    columns: gtk::Box,
    me: std::cell::RefCell<std::rc::Weak<SyncPage>>,
}

fn kind_row(name: &str, state: &str, tone: &str) -> gtk::Box {
    let row = ui::hbox(8, &["sync-kind"]);
    let words = ui::line(name, &[]);
    words.set_hexpand(true);
    row.append(&words);
    row.append(&widgets::chip(state, tone));
    row
}

/// What the last run stopped on, in words, and whether the person can accept it (`ame vault sync now accept`)
fn problem_words(name: &str, count: u64) -> (String, bool) {
    match name {
        "many-gone" => (t!("{count, plural, one {# item has gone from the server, or been put in its trash.} other {# items have gone from the server, or been put in its trash.}} If you removed them there, accept it.", count = count), true),
        "many-new" => (t!("{count, plural, one {# item has come from the server.} other {# items have come from the server.}} If you added them there, accept it.", count = count), true),
        "unreachable" | "timeout" | "server-down" => (t!("The server did not answer. The vault goes on, and tries again later."), false),
        "untrusted" => (t!("The server's certificate is not trusted."), false),
        "rate-limited" => (t!("The server asks for fewer tries. Wait a while."), false),
        "login-expired" | "keys-changed" | "cannot-open" | "identity-changed" => (t!("The login has to be proved again. Disconnect, and connect again."), false),
        "kdf-lowered" | "kdf-too-weak" => (t!("The server asks for less work to make the key than it did. Nothing was sent."), false),
        "local-unreadable" => (t!("Some items of the vault did not read, so nothing is sent. Restore the vault from a backup."), false),
        _ => (t!("The last sync stopped. `ame vault sync status` says why."), false),
    }
}

impl SyncPage {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let page = ui::vbox(18, &["page"]);
        let head = ui::hbox(12, &["page-head"]);
        let title = widgets::h1(&t!("Sync"));
        title.set_hexpand(true);
        head.append(&title);
        let head_chip = widgets::chip(&t!("Not connected"), "");
        head.append(&head_chip);
        page.append(&head);
        let columns = ui::hbox(16, &[]);
        let server = widgets::card();
        server.set_hexpand(true);
        columns.append(&server);
        let goes = widgets::card();
        goes.set_hexpand(true);
        goes.append(&widgets::h2(&t!("What goes there")));
        for (name, state, tone) in [
            (t!("Passwords"), t!("Syncs"), "ok"),
            (t!("Authenticator codes"), t!("Syncs"), "ok"),
            (t!("Backup codes"), t!("Syncs"), "ok"),
            // Each on its own page: off unless chosen, and then only sealed
            (t!("Keys"), t!("Sealed, if chosen"), ""),
            (t!("Certificates"), t!("Sealed, if chosen"), ""),
            (t!("Protected files"), t!("Off"), ""),
        ] {
            goes.append(&kind_row(&name, &state, tone));
        }
        columns.append(&goes);
        page.append(&columns);
        let conflicts = ui::vbox(0, &[]);
        page.append(&conflicts);
        page.append(&widgets::p(&t!("The server sees only encrypted items, but it can try to trick an app. Use a server you trust, and keep your most sensitive items set to Sealed."), &["soft", "small"]));
        let root = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&page).build();
        let this = Rc::new(SyncPage { vault: vault.clone(), root, head_chip, server, conflicts, columns, me: std::cell::RefCell::new(std::rc::Weak::new()) });
        this.me.replace(Rc::downgrade(&this));
        this
    }

    fn not_connected(&self) {
        let server = &self.server;
        ui::clear(server);
        server.append(&widgets::h2(&t!("Your server")));
        let address = widgets::entry("", "https://");
        address.set_direction(gtk::TextDirection::Ltr);
        server.append(&widgets::field(&t!("Address"), &address, &[]));
        server.append(&widgets::p(&t!("Whoever opens this vault can reach that whole account, so a dedicated account is best. Its address is named under what this computer sends. The login happens in a terminal."), &["soft", "small"]));
        let error = widgets::problem();
        server.append(&error);
        let connect = btn(&t!("Connect"), Some("refresh"), &["primary"], move || {
            let typed = address.text().trim().to_string();
            if !typed.starts_with("https://") {
                say(&error, &t!("The address starts with https://."));
                return;
            }
            say(&error, "");
            process::in_terminal(&["ame", "vault", "sync", "connect", &typed]);
        });
        connect.set_halign(gtk::Align::Start);
        server.append(&connect);
    }

    fn connected(&self, status: &Value) {
        let server = &self.server;
        ui::clear(server);
        server.append(&widgets::h2(&t!("Your server")));
        let pairs = widgets::Pairs::new();
        pairs.add(&t!("Address"), status["address"].as_str().unwrap_or(""), true);
        pairs.add(&t!("Account"), status["email"].as_str().unwrap_or(""), true);
        if let Some(linked) = status["linked"].as_u64() {
            pairs.add(&t!("Items kept in step"), &i18n::number(linked as f64), false);
        }
        if status["running"] == json!(true) {
            pairs.add(&t!("Now"), &t!("Syncing…"), false);
        } else if let Some(last) = status["last_success_ms"].as_i64().filter(|last| *last > 0) {
            pairs.add(&t!("Last sync"), &list::long_date(last), false);
        } else {
            pairs.add(&t!("Last sync"), &t!("Not yet"), false);
        }
        server.append(&pairs.grid);
        let row = ui::hbox(8, &[]);
        row.append(&btn(&t!("Sync now"), Some("refresh"), &["primary"], || process::in_terminal(&["ame", "vault", "sync", "now"])));
        row.append(&btn(&t!("Disconnect"), None, &["danger"], || process::in_terminal(&["ame", "vault", "sync", "off"])));
        server.append(&row);
        ui::clear(&self.conflicts);
        // What the last run did to items changed in both places: both were kept
        let report = &status["last"]["report"];
        let conflicts = report["conflicts"].as_u64().unwrap_or(0);
        // What the server was not sent, and so is on this computer only: said, never left to be found missing
        let refused = report["refused_total"].as_u64().unwrap_or(0);
        let problem = &status["problem"];
        if conflicts == 0 && refused == 0 && !problem.is_object() {
            return;
        }
        let card = widgets::card();
        card.append(&widgets::h2(&t!("Needs a look")));
        if conflicts > 0 {
            card.append(&ui::label(&t!("{count, plural, one {# item changed here and on the server. Both are kept.} other {# items changed here and on the server. Both are kept.}}", count = conflicts), &[]));
        }
        if refused > 0 {
            card.append(&ui::label(&t!("{count, plural, one {# item did not go to the server, and stays on this computer only. Its page says why.} other {# items did not go to the server, and stay on this computer only. Each one's page says why.}}", count = refused), &[]));
        }
        if let Some(name) = problem["name"].as_str() {
            let count = problem["detail"]["count"].as_u64().unwrap_or(0);
            let (words, accept) = problem_words(name, count);
            let said = ui::label(&words, &[]);
            said.set_hexpand(true);
            card.append(&said);
            // The service stopped to ask the person, and a person is who answers: in a terminal, where it is asked
            if accept {
                let button = btn(&t!("Accept it"), Some("check"), &["primary", "small"], || process::in_terminal(&["ame", "vault", "sync", "now", "accept"]));
                button.set_halign(gtk::Align::Start);
                card.append(&button);
            }
        }
        self.conflicts.append(&card);
    }
}

impl Page for SyncPage {
    fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    fn reload(&self) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            let status = vault.call(json!({"op": "sync-status"})).await;
            let Some(page) = weak.upgrade() else { return };
            match status {
                Ok(status) if status["connected"] == json!(true) => {
                    page.head_chip.set_label(&t!("Connected"));
                    page.head_chip.add_css_class("ok");
                    page.connected(&status);
                }
                _ => {
                    page.head_chip.set_label(&t!("Not connected"));
                    page.head_chip.remove_css_class("ok");
                    page.not_connected();
                }
            }
        });
    }

    fn width(&self, width: Width) {
        self.columns.set_orientation(if width == Width::Full { gtk::Orientation::Horizontal } else { gtk::Orientation::Vertical });
    }
}
