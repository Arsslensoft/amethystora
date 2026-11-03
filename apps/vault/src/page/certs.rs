//! Certificates: all five kinds, each issued by this machine's own authority, and what the window always says beside one, since only a
//! computer that trusts it accepts it. Signing a PDF and trusting a CA are `ame vault` commands in a terminal, as the plan has them.

use crate::scan;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{glib, gtk, i18n, process, t, ui};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::{export, Page, Vault, Width};
use crate::icons::icon;
use crate::list::{self, ListPage, Row};
use crate::rpc::{self, Trouble};
use crate::secret;
use crate::widgets::{self, btn, say};

const DAY_MS: i64 = 86_400_000;

fn profile_words(profile: &str) -> String {
    match profile {
        "pdf" => t!("PDF signature"),
        "code" => t!("Code signing"),
        "smime" => t!("Email"),
        "local-tls" => t!("Local TLS"),
        "device-identity" => t!("This computer's identity"),
        "service-login" => t!("Logging in to a service"),
        other => other.to_string(),
    }
}

fn purpose_words(profile: &str) -> String {
    match profile {
        "pdf" => t!("Document signing, email protection"),
        "code" => t!("Code signing"),
        "smime" => t!("Email protection"),
        "local-tls" => t!("Server authentication"),
        "device-identity" => t!("Client and server authentication"),
        "service-login" => t!("Client authentication"),
        other => other.to_string(),
    }
}

fn now_ms() -> i64 {
    (i18n::now() * 1000.0) as i64
}

fn row_of(entry: &Value) -> Row {
    let meta = &entry["meta"];
    let until = meta["not_after"].as_i64().unwrap_or(0) * 1000;
    let sub = t!("{kind}, until {date}", kind = profile_words(meta["profile"].as_str().unwrap_or("")), date = list::long_date(until));
    let tag = if until < now_ms() {
        Some((t!("Expired"), "bad"))
    } else if until < now_ms() + 30 * DAY_MS {
        Some((t!("Expires soon"), "warn"))
    } else if meta["renewed_by"].is_string() {
        Some((t!("Renewed"), ""))
    } else {
        None
    };
    Row::new(entry, list::text(entry, "name"), sub, tag)
}

pub struct Certificates {
    vault: Rc<Vault>,
    list: Rc<ListPage>,
    machine: RefCell<Value>,
    me: RefCell<std::rc::Weak<Certificates>>,
}

impl Certificates {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let weak = Rc::downgrade(vault);
        let list = ListPage::new(
            &t!("Search certificates"),
            &t!("Make a certificate"),
            move || {
                if let Some(vault) = weak.upgrade() {
                    new_sheet(&vault);
                }
            },
            &t!("No certificates yet. Make one with the plus above."),
        );
        let page = Rc::new(Certificates { vault: vault.clone(), list: list.clone(), machine: RefCell::new(Value::Null), me: RefCell::new(std::rc::Weak::new()) });
        page.me.replace(Rc::downgrade(&page));
        let weak = Rc::downgrade(&page);
        list.on_select(move |row| {
            if let Some(page) = weak.upgrade() {
                page.render(row);
            }
        });
        page
    }

    fn render(&self, row: Option<&Row>) {
        self.vault.secrets.hide_all();
        let Some(row) = row else {
            self.list.clear();
            return;
        };
        let meta = &row.meta;
        let profile = meta["profile"].as_str().unwrap_or("");
        let until = meta["not_after"].as_i64().unwrap_or(0) * 1000;
        let validity = if until < now_ms() {
            widgets::chip(&t!("Expired on {date}", date = list::long_date(until)), "bad")
        } else if until < now_ms() + 30 * DAY_MS {
            widgets::chip(&t!("Expires on {date}", date = list::long_date(until)), "warn")
        } else {
            widgets::chip(&t!("Valid until {date}", date = list::long_date(until)), "ok")
        };
        let body = Vault::detail_body();
        body.append(&self.vault.detail_head(&self.list, row, "", &[]));
        let chips = ui::wrap(8, 8, &[]);
        chips.append(&widgets::chip(&profile_words(profile), "accent"));
        chips.append(&validity);
        if meta["renewed_by"].is_string() {
            chips.append(&widgets::chip(&t!("Renewed: a newer one takes its place"), ""));
        }
        body.append(&chips);
        body.append(&widgets::callout(&t!("Only computers that trust this machine's root, or your CA, accept this. Other computers call the signer unknown until then.")));

        let says = widgets::card();
        says.append(&widgets::h2(&t!("What it says")));
        let pairs = widgets::Pairs::new();
        pairs.add(&t!("Subject"), meta["subject"].as_str().unwrap_or(""), true);
        pairs.add(&t!("Issued by"), meta["issuer"].as_str().unwrap_or(""), true);
        pairs.add(&t!("Purpose"), &purpose_words(profile), false);
        pairs.add(&t!("Key usage"), &t!("Digital signature, critical"), false);
        pairs.add(&t!("Key"), &key_words(meta["key_type"].as_str().unwrap_or("")), true);
        pairs.add(&t!("Fingerprint"), &meta["sha256"].as_str().map(short_fingerprint).unwrap_or_default(), true);
        says.append(&pairs.grid);
        body.append(&says);

        let chain = widgets::card();
        chain.append(&widgets::h2(&t!("The chain")));
        let machine = self.machine.borrow();
        let number = machine["number"].as_u64().map(|number| number.to_string()).or_else(|| machine["number"].as_str().map(str::to_string));
        let root_line = match number {
            Some(number) => t!("This machine, {number}", number = number),
            None => t!("This machine"),
        };
        chain.append(&chain_step("lock", &t!("Amethystora OS Root CA"), &root_line, false, 0));
        let issuer = meta["issuer"].as_str().unwrap_or("");
        let issuer_name = issuer.split(", ").next().unwrap_or(issuer).trim_start_matches("CN=");
        chain.append(&chain_step("cert", issuer_name, &if matches!(profile, "pdf" | "code" | "smime") { t!("Your CA") } else { t!("This machine's") }, false, 1));
        chain.append(&chain_step("pass", &row.title, &t!("This certificate"), true, 2));
        body.append(&chain);
        body.append(&self.use_card(row, profile));
        body.append(&self.vault.approval_card("certificate", row));
        body.append(&self.vault.sync_card("certificate", &row.id, &row.approval, row.meta["profile"] == "device-identity"));
        let (weak, id, title) = (self.me.borrow().clone(), row.id.clone(), row.title.clone());
        let remove = btn(&t!("Remove"), Some("trash"), &["small", "danger"], move || {
            let Some(page) = weak.upgrade() else { return };
            let reload = page.me.borrow().clone();
            page.vault.remove_with_undo(&page.list, "certificate", &id, &title, move || {
                if let Some(page) = reload.upgrade() {
                    page.reload();
                }
            });
        });
        remove.set_halign(gtk::Align::Start);
        body.append(&remove);
        self.list.show(&body);
    }

    /// What this kind of certificate is used for, first
    fn use_card(&self, row: &Row, profile: &str) -> gtk::Box {
        let card = widgets::card();
        card.append(&widgets::h2(&t!("Use it")));
        let buttons = ui::vbox(8, &[]);
        let vault = self.vault.clone();
        let (id, title) = (row.id.clone(), row.title.clone());
        match profile {
            "pdf" => {
                let (vault_sign, which) = (vault.clone(), id.clone());
                buttons.append(&btn(&t!("Sign a PDF"), Some("check"), &["primary"], move || sign_pdf(&vault_sign, which.clone())));
                buttons.append(&give_ca(&vault, &t!("Give out your CA"), "account", "ca"));
                card.append(&buttons);
                card.append(&widgets::p(&t!("Signing asks you first. The key stays in the vault. A signed PDF tells its readers the signer's name, your name in the CA, and a number that identifies this computer."), &["soft", "small"]));
                return card;
            }
            "local-tls" => {
                let (vault_key, key_id, name) = (vault.clone(), id.clone(), title.clone());
                buttons.append(&btn(&t!("Give a server its key and chain"), Some("down"), &["primary"], move || server_files(&vault_key, key_id.clone(), name.clone())));
                buttons.append(&give_ca(&vault, &t!("Give out the Local Services CA"), "local", "local-services-ca"));
                let on = t!("Trust it in browsers from your distribution");
                buttons.append(&btn(&on, Some("shield"), &[], || process::in_terminal(&["ame", "vault", "tls", "trust", "browsers", "on"])));
                card.append(&buttons);
                card.append(&widgets::p(&t!("Browsers warn until they trust your Local Services CA. Firefox and Flatpak browsers take it by hand."), &["soft", "small"]));
            }
            _ => {
                let (vault_pfx, pfx_id, name) = (vault.clone(), id.clone(), title.clone());
                buttons.append(&btn(&t!("Export for signing (PFX)"), Some("down"), &["primary"], move || export_pfx(&vault_pfx, pfx_id.clone(), name.clone())));
                let (vault_chain, chain_id) = (vault.clone(), id.clone());
                buttons.append(&btn(&t!("Copy the chain (PEM)"), Some("copy"), &[], move || {
                    let (vault, id) = (vault_chain.clone(), chain_id.clone());
                    glib::spawn_future_local(async move {
                        match vault.call(json!({"op": "cert-chain", "id": id})).await {
                            Ok(answer) => {
                                process::copy(answer["pem"].as_str().unwrap_or(""));
                                vault.say(&t!("Copied."));
                            }
                            Err(trouble) => vault.say(&trouble.words()),
                        }
                    });
                }));
                if profile == "device-identity" || profile == "service-login" {
                    buttons.append(&give_ca(&vault, &t!("Give out the Device Identity CA"), "device", "device-identity-ca"));
                } else {
                    buttons.append(&give_ca(&vault, &t!("Give out your CA"), "account", "ca"));
                }
                card.append(&buttons);
                card.append(&widgets::p(&t!("A PFX holds the private key. You will be asked first, and it then lives outside the vault."), &["soft", "small"]));
            }
        }
        let (vault_renew, renew_id, weak) = (vault.clone(), id, self.me.borrow().clone());
        let renew = btn(&t!("Renew"), Some("refresh"), &[], move || {
            let (vault, id, weak) = (vault_renew.clone(), renew_id.clone(), weak.clone());
            glib::spawn_future_local(async move {
                match vault.call(json!({"op": "cert-renew", "id": id})).await {
                    Ok(answer) => {
                        vault.count_items();
                        if let Some(page) = weak.upgrade() {
                            page.reload_selecting(answer["id"].as_str().map(str::to_string));
                        }
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        card.append(&renew);
        card
    }

    fn reload_selecting(&self, select: Option<String>) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            let machine = vault.call(json!({"op": "machine"})).await;
            let Ok(answer) = vault.call(json!({"op": "list", "kind": "certificate"})).await else { return };
            let Some(page) = weak.upgrade() else { return };
            if let Ok(machine) = machine {
                page.machine.replace(machine.0.clone());
            }
            let mut rows: Vec<Row> = answer["items"].as_array().into_iter().flatten().map(row_of).collect();
            super::sort_rows(&mut rows);
            page.list.set_rows(rows);
            match &select {
                Some(id) => page.list.select_id(id),
                None if vault.width.get() == Width::Full => page.list.select_first(),
                None => {}
            }
        });
    }
}

fn short_fingerprint(hex: &str) -> String {
    if hex.len() > 16 {
        format!("SHA256:{}…{}", &hex[..8], &hex[hex.len() - 8..])
    } else {
        hex.to_string()
    }
}

fn key_words(key_type: &str) -> String {
    match key_type {
        "rsa-3072" => "RSA 3072".into(),
        "rsa-4096" => "RSA 4096".into(),
        "ecdsa-p256" => "ECDSA P-256".into(),
        "ecdsa-p384" => "ECDSA P-384".into(),
        other => other.to_string(),
    }
}

/// One step of the chain, stepped in by how far down it is
fn chain_step(glyph: &str, title: &str, sub: &str, this: bool, depth: i32) -> gtk::Box {
    let step = ui::hbox(10, &["chain-step"]);
    if this {
        step.add_css_class("this");
    }
    step.add_css_class(&format!("depth-{depth}"));
    step.append(&icon(glyph, 16));
    let words = ui::vbox(0, &[]);
    words.append(&widgets::data(title, &["step-title"]));
    words.append(&ui::line(sub, &["soft", "tiny"]));
    step.append(&words);
    step
}

/// A CA's certificate, for whoever should trust it: only a file, which holds nothing secret
fn give_ca(vault: &Rc<Vault>, words: &str, which: &'static str, file: &'static str) -> gtk::Button {
    let vault = vault.clone();
    btn(words, Some("down"), &[], move || {
        let vault = vault.clone();
        glib::spawn_future_local(async move {
            match vault.call(json!({"op": "ca-cert", "which": which})).await {
                Ok(answer) => {
                    let pem = answer["pem"].as_str().unwrap_or("").as_bytes().to_vec();
                    if export::save_to(&vault, &format!("{file}.pem"), Zeroizing::new(pem)).await.is_some() {
                        vault.say(&t!("Saved. Whoever should accept your signatures can import this file."));
                    }
                }
                Err(Trouble::Refused { .. }) => vault.say(&t!("It is made with the first certificate that needs it.")),
                Err(trouble) => vault.say(&trouble.words()),
            }
        });
    })
}

/// `ame vault pdf sign`: said first, and then in a terminal, which also shows the prompt's outcome. The certificate is named by its id,
/// which the service made and which cannot read as an option, and what the person picked comes after `--`: a title that began with
/// dashes would otherwise be taken for one.
fn sign_pdf(vault: &Rc<Vault>, id: String) {
    let vault = vault.clone();
    glib::spawn_future_local(async move {
        let Some(path) = scan::choose_file(&t!("Sign a PDF"), None).await else { return };
        let sure = vault.confirm(&t!("Sign this PDF?"), &t!("Only computers that trust this machine's root, or your CA, accept this. Other computers call the signer unknown until then."), &t!("Sign it"), false).await;
        if sure {
            process::in_terminal(&["ame", "vault", "pdf", "sign", "--", &path.to_string_lossy(), &id]);
        }
    });
}

fn export_pfx(vault: &Rc<Vault>, id: String, name: String) {
    let this = vault.clone();
    vault.new_password(&t!("A password for the PFX file"), Some(&t!("A PFX holds the private key. You will be asked first, and it then lives outside the vault.")), move |_, password| {
        let (vault, id, name) = (this.clone(), id.clone(), name.clone());
        glib::spawn_future_local(async move {
            let mut request = json!({"op": "cert-pfx", "id": id});
            request["password"] = rpc::encode(password.as_bytes());
            match vault.call(request).await {
                Ok(answer) => {
                    use base64::Engine;
                    let Ok(data) = base64::engine::general_purpose::STANDARD.decode(answer["data"].as_str().unwrap_or("")) else { return };
                    if export::save_to(&vault, &format!("{name}.pfx"), Zeroizing::new(data)).await.is_some() {
                        vault.say(&t!("Saved."));
                    }
                }
                Err(Trouble::Refused { wait: 0, problem: None }) => {}
                Err(trouble) => vault.say(&trouble.words()),
            }
        });
    });
}

/// A local server cannot ask a person, so it gets its key as a plain file, after an approval
fn server_files(vault: &Rc<Vault>, id: String, name: String) {
    let vault = vault.clone();
    glib::spawn_future_local(async move {
        match vault.call(json!({"op": "cert-key", "id": id, "plain": true})).await {
            Ok(answer) => {
                let Some(key) = rpc::secret_of(&answer, "key") else { return };
                let mut both = Zeroizing::new(key.as_bytes().to_vec());
                both.extend_from_slice(answer["pem"].as_str().unwrap_or("").as_bytes());
                if export::save_to(&vault, &format!("{name}.pem"), both).await.is_some() {
                    vault.say(&t!("Saved: the key first, then the chain. It now lives outside the vault."));
                }
            }
            Err(Trouble::Refused { wait: 0, problem: None }) => {}
            Err(trouble) => vault.say(&trouble.words()),
        }
    });
}

/// How a form differs, for each of the five kinds on the board and identity's two
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Pdf,
    Code,
    Mail,
    Tls,
    Identity,
}

/// The sheet that makes one: the same frame, and the fields each kind takes
fn new_sheet(vault: &Rc<Vault>) {
    let sheet = widgets::sheet(&t!("Make a certificate"), 600);
    let kind = Rc::new(Cell::new(Kind::Pdf));
    let holder = ui::vbox(16, &[]);
    let draw: Rc<dyn Fn()> = {
        let (kind, holder, vault, dialog, foot) = (kind.clone(), holder.clone(), vault.clone(), sheet.dialog.clone(), sheet.foot.clone());
        Rc::new(move || {
            ui::clear(&holder);
            ui::clear(&foot);
            fill(&vault, kind.get(), &holder, &dialog, &foot);
        })
    };
    let (chosen, redraw) = (kind.clone(), draw.clone());
    let choices = [("pdf", t!("PDF")), ("code", t!("Code")), ("mail", t!("Email")), ("tls", t!("Local TLS")), ("identity", t!("Identity"))];
    sheet.body.append(&widgets::seg(&choices, "pdf", move |value| {
        chosen.set(match value {
            "code" => Kind::Code,
            "mail" => Kind::Mail,
            "tls" => Kind::Tls,
            "identity" => Kind::Identity,
            _ => Kind::Pdf,
        });
        redraw();
    }));
    sheet.body.append(&holder);
    draw();
    sheet.dialog.present(Some(&vault.shell.window));
}

fn key_choices(types: &[&str], words: &[String], chosen: &Rc<RefCell<String>>) -> gtk::Grid {
    let group = RefCell::new(None);
    let mut tiles: Vec<gtk::Widget> = Vec::new();
    for (at, (key_type, sub)) in types.iter().zip(words).enumerate() {
        let (keep, value) = (chosen.clone(), key_type.to_string());
        let tile = widgets::tile(&key_words(key_type), sub, &group, move || {
            keep.replace(value.clone());
        });
        if at == 0 {
            tile.set_active(true);
            chosen.replace(key_type.to_string());
        }
        tiles.push(tile.upcast());
    }
    widgets::columns(tiles.len().max(1) as i32, 8, &tiles)
}

fn fill(vault: &Rc<Vault>, kind: Kind, holder: &gtk::Box, dialog: &amethystora_ui::adw::Dialog, foot: &gtk::Box) {
    let full_name = glib::real_name().to_string_lossy().into_owned();
    let title = widgets::entry("", "");
    // A name for the list, beside the signer's where there is one: a sheet over 720 pixels hides its buttons
    let named = |holder: &gtk::Box, signer: &gtk::Entry| {
        holder.append(&widgets::columns(2, 12, &[widgets::field(&t!("Name"), &title, &[]).upcast(), widgets::field(&t!("Signer's name"), signer, &[]).upcast()]));
    };
    if matches!(kind, Kind::Tls | Kind::Identity) {
        holder.append(&widgets::field(&t!("Name"), &title, &[]));
    }
    let chosen_key = Rc::new(RefCell::new(String::new()));
    let subject = widgets::code("", &[]);
    let valid = ui::label("", &[]);
    let issuer = widgets::code("", &[]);
    // What each kind holds, read when it is made
    let signer = widgets::entry(&full_name, "");
    let organization = widgets::entry("", "");
    let country = widgets::entry("", "");
    let email = widgets::entry("", "");
    let hosts = widgets::entry("", "nas.home.arpa, 192.168.1.5");
    hosts.set_direction(gtk::TextDirection::Ltr);
    let service_login = Rc::new(Cell::new(true));
    let days;
    let ca_name = t!("{name} Code Signing CA, under this machine's root", name = full_name);
    match kind {
        Kind::Pdf | Kind::Code => {
            named(holder, &signer);
            holder.append(&widgets::columns(2, 12, &[widgets::field(&t!("Organization, if any"), &organization, &[]).upcast(), widgets::field(&t!("Country code, if any"), &country, &[]).upcast()]));
            holder.append(&ui::line(&t!("Key"), &["field-label"]));
            let words = [t!("Works most widely"), t!("Larger"), t!("Smaller")];
            holder.append(&key_choices(&["rsa-3072", "rsa-4096", "ecdsa-p256"], &words, &chosen_key));
            issuer.set_label(&ca_name);
            days = 460;
        }
        Kind::Mail => {
            named(holder, &signer);
            email.set_direction(gtk::TextDirection::Ltr);
            holder.append(&widgets::field(&t!("Email address"), &email, &[]));
            holder.append(&widgets::p(&t!("Nothing checks this address. It is exported as a PFX for your mail program, which keeps its own copy."), &["soft", "small"]));
            holder.append(&ui::line(&t!("Key"), &["field-label"]));
            let words = [t!("Works most widely"), t!("Larger")];
            holder.append(&key_choices(&["rsa-3072", "rsa-4096"], &words, &chosen_key));
            issuer.set_label(&ca_name);
            days = 460;
        }
        Kind::Tls => {
            holder.append(&widgets::field(&t!("Local names or private addresses"), &hosts, &[]));
            holder.append(&widgets::p(&t!("Only names under .local, .home.arpa, .internal and localhost, and private addresses. A public website's name is refused."), &["soft", "small"]));
            holder.append(&ui::line(&t!("Key"), &["field-label"]));
            let words = [t!("The default"), t!("For older clients")];
            holder.append(&key_choices(&["ecdsa-p256", "rsa-3072"], &words, &chosen_key));
            issuer.set_label(&t!("Local Services CA, under this machine's root"));
            subject.set_label("CN=Local service, OU=Amethystora OS");
            days = 397;
        }
        Kind::Identity => {
            let keep = service_login.clone();
            holder.append(&widgets::seg(&[("login", t!("Logging in to a service")), ("device", t!("This computer"))], "login", move |value| keep.set(value == "login")));
            holder.append(&widgets::p(&t!("For a VPN, Wi-Fi or a server of your own that asks for a certificate. The name is what you call it there. They prove which machine, not who. This computer's own needs an administrator."), &["soft", "small"]));
            holder.append(&ui::line(&t!("Key"), &["field-label"]));
            let words = [t!("The default"), t!("For older clients")];
            holder.append(&key_choices(&["ecdsa-p256", "rsa-3072"], &words, &chosen_key));
            issuer.set_label(&t!("Device Identity CA, under this machine's root"));
            days = 397;
        }
    }
    // What it will read, as it is typed
    let preview_subject = |signer: &gtk::Entry, organization: &gtk::Entry, country: &gtk::Entry| {
        let mut words = format!("CN={}, OU=Amethystora Vault", signer.text());
        if !organization.text().is_empty() {
            words.push_str(&format!(", O={}", organization.text()));
        }
        if !country.text().is_empty() {
            words.push_str(&format!(", C={}", country.text()));
        }
        words
    };
    if matches!(kind, Kind::Pdf | Kind::Code | Kind::Mail) {
        subject.set_label(&preview_subject(&signer, &organization, &country));
        for entry in [&signer, &organization, &country] {
            let (subject, signer, organization, country) = (subject.clone(), signer.clone(), organization.clone(), country.clone());
            entry.connect_changed(move |_| subject.set_label(&preview_subject(&signer, &organization, &country)));
        }
    } else if kind == Kind::Identity {
        subject.set_label(&t!("Your name for it, your login, and this machine's number"));
    }
    let reads = ui::vbox(6, &["reads"]);
    reads.append(&ui::line(&t!("The subject will read"), &["field-label"]));
    reads.append(&subject);
    reads.append(&ui::line(&t!("Issued by"), &["field-label"]));
    reads.append(&issuer);
    reads.append(&ui::line(&t!("Valid for"), &["field-label"]));
    valid.set_label(&t!("{days, plural, one {# day} other {# days}}, until {date}", days = days, date = list::long_date(now_ms() + days * DAY_MS)));
    reads.append(&valid);
    holder.append(&reads);
    let error = widgets::problem();
    holder.append(&error);
    let dialog_cancel = dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog_cancel.close();
    });
    let (vault_make, dialog_make) = (vault.clone(), dialog.clone());
    let make = btn(&t!("Make the certificate"), Some("cert"), &["primary"], || {});
    make.connect_clicked(move |make| {
        let name = title.text().trim().to_string();
        if name.is_empty() {
            say(&error, &t!("Give it a name."));
            return;
        }
        let mut request = json!({"op": "cert-new", "title": name, "key_type": *chosen_key.borrow()});
        match kind {
            Kind::Pdf | Kind::Code => {
                request["profile"] = json!(if kind == Kind::Pdf { "pdf" } else { "code" });
                request["name"] = json!(signer.text().as_str());
                if !organization.text().is_empty() {
                    request["organization"] = json!(organization.text().as_str());
                }
                if !country.text().is_empty() {
                    request["country"] = json!(country.text().as_str());
                }
            }
            Kind::Mail => {
                request["profile"] = json!("smime");
                request["name"] = json!(signer.text().as_str());
                request["email"] = json!(email.text().as_str());
                if email.text().is_empty() {
                    say(&error, &t!("It needs an email address."));
                    return;
                }
            }
            Kind::Tls => {
                request["profile"] = json!("local-tls");
                let names: Vec<String> = hosts.text().split(',').map(|host| host.trim().to_string()).filter(|host| !host.is_empty()).collect();
                if names.is_empty() {
                    say(&error, &t!("Name at least one local name or private address."));
                    return;
                }
                request["hosts"] = json!(names);
            }
            Kind::Identity => {
                if service_login.get() {
                    request["profile"] = json!("service-login");
                    request["name"] = json!(name);
                } else {
                    request["profile"] = json!("device-identity");
                }
            }
        }
        let (vault, dialog, error, make) = (vault_make.clone(), dialog_make.clone(), error.clone(), make.clone());
        make.set_sensitive(false);
        glib::spawn_future_local(async move {
            let answer = vault.call(request).await;
            make.set_sensitive(true);
            match answer {
                Ok(answer) => {
                    dialog.close();
                    if answer["new_ca"] == json!(true) {
                        vault.say(&t!("Made, with a new CA under this machine's root."));
                    }
                    vault.count_items();
                    vault.reload_and_select(answer["id"].as_str().map(str::to_string));
                }
                Err(Trouble::Refused { wait: 0, problem: None }) => {}
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    });
    foot.append(&widgets::actions(&[cancel.upcast(), make.upcast()]));
}

impl Page for Certificates {
    fn widget(&self) -> gtk::Widget {
        self.list.root.clone().upcast()
    }

    fn reload(&self) {
        self.reload_selecting(self.list.current.borrow().clone());
    }

    fn width(&self, width: Width) {
        self.list.set_narrow(width != Width::Full);
    }

    fn add(&self) {
        new_sheet(&self.vault);
    }

    fn find(&self) {
        self.list.search.grab_focus();
    }

    fn select(&self, id: &str) {
        self.list.select_id(id);
    }
}

#[allow(unused_imports)]
use secret as _;
