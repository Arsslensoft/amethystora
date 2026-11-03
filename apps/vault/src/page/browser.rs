//! The browser extension: where to get it, the one setup step it needs (`ame vault browser on`, in a terminal, which writes the native
//! host's file into the person's own folders), and what it cannot reach. Nothing here installs it, and the image's Firefox stays as it is.

use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gtk, process, t, ui};

use super::{Page, Vault};
use crate::widgets::{self, btn};

/// The stores' listings, once they are published (the maintainer's): placeholders until then
const FIREFOX_LISTING: &str = "https://addons.mozilla.org/firefox/addon/amethystora-vault/";
const CHROMIUM_LISTING: &str = "https://chromewebstore.google.com/search/Amethystora%20Vault";

pub struct Browser {
    root: gtk::ScrolledWindow,
}

impl Browser {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let page = ui::vbox(18, &["page"]);
        let head = ui::vbox(4, &["page-head"]);
        head.append(&widgets::h1(&t!("In your browser")));
        head.append(&widgets::p(&t!("Vault's extension fills a password or a sign-in code when you click it, never by itself, and only on the site the item is for."), &["soft"]));
        page.append(&head);

        let get = widgets::card();
        get.append(&widgets::h2(&t!("1. Install the extension")));
        get.append(&widgets::p(&t!("From your browser's own store. Vault never installs it for you."), &["soft", "small"]));
        let row = ui::wrap(8, 8, &[]);
        let window = vault.window();
        row.append(&btn(&t!("For Firefox"), Some("down"), &[], move || process::open_uri(Some(&window), FIREFOX_LISTING)));
        let window = vault.window();
        row.append(&btn(&t!("For Chromium, Chrome, Brave and Edge"), Some("down"), &[], move || process::open_uri(Some(&window), CHROMIUM_LISTING)));
        get.append(&row);
        page.append(&get);

        let connect = widgets::card();
        connect.append(&widgets::h2(&t!("2. Let it reach the vault")));
        connect.append(&widgets::p(&t!("This writes one small file into your own browser folders, naming only Vault's extension, and takes it away again when turned off."), &["soft", "small"]));
        let row = ui::wrap(8, 8, &[]);
        row.append(&btn(&t!("Turn on"), None, &["primary", "small"], || process::in_terminal(&["ame", "vault", "browser", "on"])));
        row.append(&btn(&t!("Turn off"), None, &["small"], || process::in_terminal(&["ame", "vault", "browser", "off"])));
        connect.append(&row);
        page.append(&connect);

        page.append(&widgets::callout(&t!("Browsers installed as Flatpaks cannot reach Vault. Connect Vault to a Vaultwarden or Bitwarden server under Sync, and use Bitwarden's own extension there against the same server.")));
        page.append(&widgets::p(&t!("Before a password is filled, the vault asks as the item says. A closed vault says so, and offers to open this window."), &["soft", "small"]));
        let root = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&page).build();
        Rc::new(Browser { root })
    }
}

impl Page for Browser {
    fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    fn reload(&self) {}
}
