//! Protected files and the SSH agent. Taking a file in, giving it back and turning the agent on or off are `ame vault` commands in a
//! terminal, where the person sees what they do. What the window adds is what is said first: old copies survive, in the snapshots and
//! the backups, so a key that was protected should be replaced.

use crate::scan;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gio, glib, gtk, palette, process, t, ui};
use serde_json::{json, Value};

use super::{Page, Vault};
use crate::rpc::Trouble;
use crate::widgets::{self, btn};

/// The usual places, from the watcher's own list (`CREDENTIAL_FILES` in `amethystora-audit-home-rules`): whatever is a file of its
/// own and holds a login, not the browsers' profiles, which a browser has to keep open itself
const USUAL: &[&str] = &[
    ".npmrc",
    ".pypirc",
    ".git-credentials",
    ".config/gh/hosts.yml",
    ".aws/credentials",
    ".docker/config.json",
    ".kube/config",
    ".claude/.credentials.json",
    ".codex/auth.json",
    ".codex/.credentials.json",
    ".gemini/oauth_creds.json",
    ".gemini/gemini-credentials.json",
    ".gemini/mcp-oauth-tokens.json",
    ".local/share/opencode/auth.json",
    ".local/share/opencode/mcp-auth.json",
    ".config/cursor/auth.json",
    ".gemini/antigravity-cli/antigravity-oauth-token",
];

/// Blocks: what is there now, from a home. `~/.ssh/id_*` but for the public halves.
fn found_files(home: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = USUAL.iter().map(|path| home.join(path)).filter(|path| path.is_file()).collect();
    if let Ok(entries) = std::fs::read_dir(home.join(".ssh")) {
        let mut keys: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                name.starts_with("id_") && !name.ends_with(".pub") && path.is_file()
            })
            .collect();
        keys.sort();
        found.extend(keys);
    }
    found
}

fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

pub struct Protected {
    vault: Rc<Vault>,
    root: gtk::ScrolledWindow,
    rows: gtk::Box,
    agent: gtk::Label,
    agent_button: gtk::Button,
    me: std::cell::RefCell<std::rc::Weak<Protected>>,
}

impl Protected {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let page = ui::vbox(18, &["page"]);
        let head = ui::vbox(4, &["page-head"]);
        head.append(&widgets::h1(&t!("Protected files")));
        head.append(&widgets::p(&t!("Sealed in the vault, and out of reach of other programs. A tool gets one only when you run it through the vault."), &["soft"]));
        page.append(&head);
        let rows = ui::vbox(8, &[]);
        page.append(&rows);
        let other_row = ui::hbox(8, &[]);
        let vault_pick = vault.clone();
        other_row.append(&btn(&t!("Protect another file"), Some("plus"), &["small"], move || {
            let vault = vault_pick.clone();
            glib::spawn_future_local(async move {
                if let Some(path) = scan::choose_file(&t!("Protect another file"), None).await {
                    ask_to_protect(&vault, path);
                }
            });
        }));
        page.append(&other_row);
        let warning = widgets::card();
        let warning_row = ui::hbox(12, &[]);
        let mark = crate::icons::icon("warn", 18);
        mark.add_css_class("warn");
        mark.set_valign(gtk::Align::Start);
        warning_row.append(&mark);
        let words = ui::vbox(6, &[]);
        words.set_hexpand(true);
        words.append(&widgets::h2(&t!("Replace keys you protect")));
        words.append(&widgets::p(&t!("Your hourly snapshots keep the old file for up to 14 days, and your backups for as long as they last. Treat it as already copied: make a new key, and retire this one."), &["soft", "small"]));
        warning_row.append(&words);
        warning.append(&warning_row);
        page.append(&warning);
        let agent_card = widgets::card();
        let agent_row = ui::hbox(12, &[]);
        let agent_words = ui::vbox(0, &[]);
        agent_words.set_hexpand(true);
        agent_words.append(&widgets::h2(&t!("SSH agent")));
        let agent = ui::label("", &["soft", "small"]);
        agent_words.append(&agent);
        agent_row.append(&agent_words);
        let agent_button = btn(&t!("Turn on"), None, &["small"], || {});
        agent_row.append(&agent_button);
        agent_card.append(&agent_row);
        page.append(&agent_card);
        let root = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&page).build();
        let this = Rc::new(Protected { vault: vault.clone(), root, rows, agent, agent_button, me: std::cell::RefCell::new(std::rc::Weak::new()) });
        this.me.replace(Rc::downgrade(&this));
        this
    }

    fn file_row(&self, path: &str, what: &str, chip: gtk::Label, action: gtk::Button) -> gtk::Box {
        let row = ui::hbox(12, &["file-row"]);
        let words = ui::vbox(0, &[]);
        words.set_hexpand(true);
        let shown = widgets::data(path, &["mono"]);
        shown.set_direction(gtk::TextDirection::Ltr);
        shown.set_xalign(0.0);
        words.append(&shown);
        if !what.is_empty() {
            words.append(&ui::label(what, &["soft", "small"]));
        }
        row.append(&words);
        row.append(&chip);
        row.append(&action);
        row
    }

    fn draw(&self, protected: &[Value], found: &[PathBuf]) {
        ui::clear(&self.rows);
        let home = palette::home();
        for item in protected {
            let meta = &item["meta"];
            let name = item["name"].as_str().unwrap_or("").to_string();
            let path = meta["what"].as_str().or(meta["path"].as_str()).unwrap_or(&name).to_string();
            let what = t!("Used through the vault. Run a tool with it by `ame vault run {name} -- COMMAND`.", name = name.clone());
            let vault = self.vault.clone();
            let shown_name = name.clone();
            let (id, approval) = (item["id"].as_str().unwrap_or("").to_string(), item["approval"].as_str().unwrap_or("while-open").to_string());
            let details = btn(&t!("Details"), None, &["small"], move || details_sheet(&vault, shown_name.clone(), &id, &approval));
            self.rows.append(&self.file_row(&path, &what, widgets::chip(&t!("Protected"), "ok"), details));
        }
        for path in found {
            let shown = tilde(path, &home);
            let vault = self.vault.clone();
            let target = path.clone();
            let protect = btn(&t!("Protect"), None, &["small", "primary"], move || ask_to_protect(&vault, target.clone()));
            self.rows.append(&self.file_row(&shown, &t!("Found. Any program of yours can read it."), widgets::chip(&t!("Not protected"), "warn"), protect));
        }
        if protected.is_empty() && found.is_empty() {
            self.rows.append(&widgets::p(&t!("Nothing to protect was found in the usual places."), &["soft"]));
        }
    }

    /// `ame vault ssh status` says whether it is on, and a press opens a terminal for the other
    async fn draw_agent(&self) {
        let said = process::run("ame", &["vault", "ssh", "status"]).await;
        let on = said.ok() && said.stdout.trim_start().starts_with("on");
        self.agent.set_label(&if on { t!("On. Keys stay in the vault, and you approve for 15 minutes at a time.") } else { t!("Off. Turned on, `ssh` asks the vault for its keys, and none of them leaves it.") });
        if let Some(words) = ui::button_words(&self.agent_button) {
            words.set_label(&if on { t!("Turn off") } else { t!("Turn on") });
        }
        self.agent_button.update_property(&[gtk::accessible::Property::Label(&if on { t!("Turn the SSH agent off") } else { t!("Turn the SSH agent on") })]);
        // One handler, whichever it says now: a terminal for the opposite
        let state = if on { "off" } else { "on" };
        self.agent_button.set_action_target_value(None);
        unsafe {
            if let Some(old) = self.agent_button.steal_data::<glib::SignalHandlerId>("press") {
                self.agent_button.disconnect(old);
            }
            let handler = self.agent_button.connect_clicked(move |_| process::in_terminal(&["ame", "vault", "ssh", state]));
            self.agent_button.set_data("press", handler);
        }
    }
}

/// What has to be said first: the old copies survive
fn ask_to_protect(vault: &Rc<Vault>, path: PathBuf) {
    let vault = vault.clone();
    glib::spawn_future_local(async move {
        let shown = path.display().to_string();
        let sure = vault
            .confirm(
                &t!("Protect this file?"),
                &t!("Your hourly snapshots keep the old file for up to 14 days, and your backups for as long as they last, and nothing here can delete those. If it holds a key or a token, make a new one afterwards."),
                &t!("Protect"),
                false,
            )
            .await;
        if sure {
            process::in_terminal(&["ame", "vault", "protect", &shown]);
        }
    });
}

fn details_sheet(vault: &Rc<Vault>, name: String, id: &str, approval: &str) {
    let sheet = widgets::sheet(&name, 520);
    sheet.body.append(&vault.sync_card("protected", id, approval, false));
    sheet.body.append(&widgets::p(&t!("Sealed in the vault. A tool reaches it through the command below, after you approve, and it is sealed again when the command ends."), &[]));
    sheet.body.append(&widgets::code(&format!("ame vault run {name} -- COMMAND"), &["small"]));
    sheet.body.append(&widgets::p(&t!("While that command runs, other programs of yours can reach what it holds. It is safe at rest and while nothing uses it."), &["soft", "small"]));
    let target = name.clone();
    let back = btn(&t!("Give it back"), None, &["danger"], move || process::in_terminal(&["ame", "vault", "unprotect", &target]));
    let dialog = sheet.dialog.clone();
    let done = btn(&t!("Done"), None, &[], move || {
        dialog.close();
    });
    sheet.pin(&widgets::actions(&[back.upcast(), done.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

impl Page for Protected {
    fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    fn reload(&self) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            let listed = match vault.call(json!({"op": "list", "kind": "protected"})).await {
                Ok(answer) => answer["items"].as_array().cloned().unwrap_or_default(),
                Err(Trouble::Closed | Trouble::NotSetUp) => return,
                Err(_) => Vec::new(),
            };
            let home = palette::home();
            let found = gio::spawn_blocking(move || found_files(&home)).await.unwrap_or_default();
            let Some(page) = weak.upgrade() else { return };
            page.draw(&listed, &found);
            page.draw_agent().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_in_the_home_is_written_short() {
        assert_eq!(tilde(Path::new("/home/a/.npmrc"), Path::new("/home/a")), "~/.npmrc");
        assert_eq!(tilde(Path::new("/etc/x"), Path::new("/home/a")), "/etc/x");
    }

    #[test]
    fn finds_keys_but_not_their_public_halves() {
        let home = std::env::temp_dir().join(format!("amethystora-vault-protect-{}", std::process::id()));
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        for name in [".ssh/id_ed25519", ".ssh/id_ed25519.pub", ".ssh/config", ".npmrc"] {
            std::fs::write(home.join(name), "x").unwrap();
        }
        let found = found_files(&home);
        std::fs::remove_dir_all(&home).unwrap();
        assert_eq!(found, vec![home.join(".npmrc"), home.join(".ssh/id_ed25519")]);
    }
}
