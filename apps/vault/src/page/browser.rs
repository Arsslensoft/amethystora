//! In your browser: instructions, and nothing more. Vault has no extension of its own: Bitwarden's works with the same account sync
//! uses. The page says to connect sync first and shows the address to give the extension (`sync-status`'s `connected` and `address`),
//! where the extension comes from, and what fills there. Nothing here installs anything or writes a file.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gtk, process, t, ui};
use serde_json::json;

use super::{Page, Section, Vault};
use crate::widgets::{self, btn};

/// Each browser's own store, which the window opens in the browser as the scan page opens a link
const FIREFOX_STORE: &str = "https://addons.mozilla.org/firefox/addon/bitwarden-password-manager/";
const CHROMIUM_STORE: &str = "https://chromewebstore.google.com/detail/bitwarden-password-manager/nngceckbapebfimnlniiiahkandclblb";
const EDGE_STORE: &str = "https://microsoftedge.microsoft.com/addons/detail/jbkfoedolllekgbhcbcoahefnbanhhlh";

pub struct Browser {
    vault: Rc<Vault>,
    root: gtk::ScrolledWindow,
    /// Step 1, which says whether sync is connected and what to type if it is
    server: gtk::Box,
    me: RefCell<Weak<Browser>>,
}

impl Browser {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let page = ui::vbox(18, &["page"]);
        let head = ui::vbox(4, &["page-head"]);
        head.append(&widgets::h1(&t!("In your browser")));
        head.append(&widgets::p(&t!("Vault works with Bitwarden's browser extension, which fills passwords and sign-in codes in your browser. It reads the same account Sync uses, so connect Sync first."), &["soft"]));
        page.append(&head);

        let server = widgets::card();
        page.append(&server);

        let get = widgets::card();
        get.append(&widgets::h2(&t!("2. Install the extension")));
        get.append(&widgets::p(&t!("Install Bitwarden's extension from your browser's own store. Vault does not install it for you."), &["soft", "small"]));
        let row = ui::wrap(8, 8, &[]);
        row.append(&btn(&t!("For Firefox"), Some("down"), &[], || crate::scan::open_uri(FIREFOX_STORE)));
        row.append(&btn(&t!("For Chrome, Chromium and Brave"), Some("down"), &[], || crate::scan::open_uri(CHROMIUM_STORE)));
        row.append(&btn(&t!("For Edge"), Some("down"), &[], || crate::scan::open_uri(EDGE_STORE)));
        get.append(&row);
        page.append(&get);

        let point = widgets::card();
        point.append(&widgets::h2(&t!("3. Point it at your server")));
        point.append(&widgets::p(&t!("On the extension's sign-in screen, open the list beside \"Logging in on\" and choose the self-hosted server. Paste the address from step 1 as the Server URL, and save."), &["soft", "small"]));
        page.append(&point);

        let login = widgets::card();
        login.append(&widgets::h2(&t!("4. Sign in with the same account")));
        login.append(&widgets::p(&t!("Use the email and password of the account Sync connected to. The extension then fills what syncs as Plain. Items that sync Sealed, and items that do not sync, stay on this computer and are not there. Sync says which kinds are which."), &["soft", "small"]));
        page.append(&login);

        page.append(&widgets::callout(&t!("If your server's certificate comes from a certificate authority of your own, trust that authority in the browser as well, or the extension cannot reach the server.")));
        page.append(&widgets::p(&t!("A browser installed as a Flatpak works too. Bitwarden is a trademark of its owner, and Vault is not made by or connected with it."), &["soft", "small"]));
        let root = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&page).build();
        let this = Rc::new(Browser { vault: vault.clone(), root, server, me: RefCell::new(Weak::new()) });
        this.me.replace(Rc::downgrade(&this));
        this
    }

    /// Step 1 with no connection: nothing to type yet, and the way to make one
    fn not_connected(&self) {
        ui::clear(&self.server);
        self.server.append(&widgets::h2(&t!("1. Connect Sync")));
        self.server.append(&widgets::p(&t!("Sync is not connected. Connect it to your Vaultwarden or Bitwarden server first: the extension signs in to that server."), &["soft", "small"]));
        let vault = self.vault.clone();
        let button = btn(&t!("Open Sync"), Some("refresh"), &["primary", "small"], move || vault.go(Section::Sync));
        button.set_halign(gtk::Align::Start);
        self.server.append(&button);
    }

    /// Step 1 with one: the address to give the extension, to copy
    fn connected(&self, address: &str) {
        ui::clear(&self.server);
        self.server.append(&widgets::h2(&t!("1. Your server's address")));
        self.server.append(&widgets::p(&t!("Sync is connected. This is the address the extension needs."), &["soft", "small"]));
        let row = ui::hbox(8, &[]);
        let shown = widgets::code(address, &[]);
        shown.set_hexpand(true);
        shown.set_halign(gtk::Align::Start);
        row.append(&shown);
        let (vault, address) = (self.vault.clone(), address.to_string());
        row.append(&btn(&t!("Copy"), Some("copy"), &["small"], move || {
            process::copy(&address);
            vault.say(&t!("Copied."));
        }));
        self.server.append(&row);
    }
}

impl Page for Browser {
    fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    fn reload(&self) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            let status = vault.call(json!({"op": "sync-status"})).await;
            let Some(page) = weak.upgrade() else { return };
            match status {
                // The address is text somebody typed: shown as text, never as markup
                Ok(status) if status["connected"] == json!(true) && status["address"].as_str().is_some_and(|address| address.starts_with("https://")) => {
                    page.connected(&widgets::clean(status["address"].as_str().unwrap_or("")));
                }
                _ => page.not_connected(),
            }
        });
    }
}
