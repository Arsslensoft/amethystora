//! Taking the codes out: an encrypted file in BackMyCodes' format, which asks for its password again afterwards to prove that it opens, or
//! the same as a run of QR codes for another device. Always with a password. The service makes either, after asking; the window only
//! writes the file where it was told to and shows the codes.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gio, glib, gtk, store as saved, t, ui};
use amethystora_vault_core::bmc;
use serde_json::json;
use zeroize::Zeroizing;

use super::{Target, Vault};
use crate::print;
use crate::qr;
use crate::rpc::{self, Trouble};
use crate::secret;
use crate::widgets::{self, btn, say};

thread_local! {
    static LAST: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// When the last export was made, for the reminder under the list. Only ever a time: counting is the list's. Read once, as the window
/// opens, and kept.
pub fn last_export() -> i64 {
    LAST.with(std::cell::Cell::get)
}

pub fn remember_last(at: i64) {
    LAST.with(|last| last.set(at));
}

/// Only an export of the first vault is remembered: one of another, a hidden one included, says nothing of what the first holds
fn remember_export(vault: &Vault, from: Option<&str>) {
    if from.is_none() || from != vault.first_open_vault().as_deref() {
        return;
    }
    let at = (amethystora_ui::i18n::now() * 1000.0) as i64;
    remember_last(at);
    // Written off the window's thread
    glib::spawn_future_local(async move {
        let _ = gio::spawn_blocking(move || {
            let mut state = saved::load("vault");
            if !state.is_object() {
                state = json!({});
            }
            state["last_export"] = json!(at);
            saved::save("vault", &state);
        })
        .await;
    });
}

pub fn show(vault: &Rc<Vault>) {
    let sheet = widgets::sheet(&t!("Move or save your codes"), 540);
    sheet.body.append(&widgets::p(&t!("Both are locked with a password you choose, and both hold the secrets of every account and all your backup codes in the vault you choose."), &["soft"]));
    // Which vault: it is named in the request, and said when asked for
    let target = vault.target();
    if let Some(field) = target.field() {
        sheet.body.append(&field);
    }
    let row = ui::vbox(8, &[]);
    let (vault_file, dialog, chosen) = (vault.clone(), sheet.dialog.clone(), target.clone());
    row.append(&btn(&t!("Save an encrypted file"), Some("down"), &[], move || {
        dialog.close();
        ask_password(&vault_file, "file", chosen.clone());
    }));
    let (vault_qr, dialog, chosen) = (vault.clone(), sheet.dialog.clone(), target);
    row.append(&btn(&t!("Show QR codes for another device"), Some("qr"), &[], move || {
        dialog.close();
        ask_password(&vault_qr, "qr", chosen.clone());
    }));
    sheet.body.append(&row);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    sheet.pin(&widgets::actions(&[cancel.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

fn ask_password(vault: &Rc<Vault>, format: &'static str, target: Rc<Target>) {
    let this = vault.clone();
    vault.new_password(&t!("Choose its password"), Some(&t!("You will need this password to open it. It is not your vault's.")), move |_, password| {
        let (this, target) = (this.clone(), target.clone());
        glib::spawn_future_local(async move {
            let mut request = json!({"op": "auth-export", "format": format});
            request["password"] = rpc::encode(password.as_bytes());
            target.name(&mut request);
            let from = target.id();
            let answer = this.call(request).await;
            let answer = match answer {
                Ok(answer) => answer,
                Err(Trouble::Refused { wait: 0, problem: None }) => return,
                Err(trouble) => {
                    this.say(&trouble.words());
                    return;
                }
            };
            if format == "file" {
                let file = Zeroizing::new(answer["file"].as_str().unwrap_or("").to_string());
                save_file(&this, file, from).await;
            } else {
                let chunks: Vec<Zeroizing<String>> = answer["chunks"].as_array().into_iter().flatten().filter_map(|chunk| chunk.as_str().map(|chunk| Zeroizing::new(chunk.to_string()))).collect();
                drop(answer);
                if this.active_soon().await {
                    show_codes(&this, chunks, from);
                }
            }
        });
    });
}

/// Blocks: readable only by this account, and a crash leaves no half of it. The file is written beside its place under a name of its
/// own, which never meets one that is there, synced, put in place and the folder synced; a write that fails leaves nothing behind.
fn write_private(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.subsec_nanos());
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.{}.{nanos}.part", std::process::id()));
    let written = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return written;
    }
    if let Some(folder) = path.parent() {
        std::fs::File::open(folder)?.sync_all()?;
    }
    Ok(())
}

/// Save it where the person says, off the window's thread
pub async fn save_to(vault: &Vault, name: &str, bytes: Zeroizing<Vec<u8>>) -> Option<PathBuf> {
    let dialog = gtk::FileDialog::builder().title(t!("Save")).initial_name(name).modal(true).build();
    let file = dialog.save_future(Some(&vault.window())).await.ok()?;
    let path = file.path()?;
    let target = path.clone();
    let written = gio::spawn_blocking(move || write_private(&target, &bytes)).await;
    match written {
        Ok(Ok(())) => Some(path),
        _ => {
            vault.say(&t!("That could not be saved there."));
            None
        }
    }
}

async fn save_file(vault: &Rc<Vault>, file: Zeroizing<String>, from: Option<String>) {
    let name = bmc::export_file_name((amethystora_ui::i18n::now() * 1000.0) as i64);
    if save_to(vault, &name, Zeroizing::new(file.as_bytes().to_vec())).await.is_none() {
        return;
    }
    prove(vault, file, from);
}

/// The password again, on what was written: a file nobody can open is worse than none
fn prove(vault: &Rc<Vault>, file: Zeroizing<String>, from: Option<String>) {
    let sheet = widgets::sheet(&t!("Check that it opens"), 480);
    sheet.body.append(&widgets::p(&t!("Type its password once more. If it opens here, it will open when you need it."), &["soft"]));
    let input = widgets::password_entry();
    sheet.body.append(&widgets::field(&t!("Its password"), &input, &[]));
    let error = widgets::problem();
    sheet.body.append(&error);
    let held = Rc::new(file);
    let (vault_prove, dialog, reading, failing) = (vault.clone(), sheet.dialog.clone(), input.clone(), error.clone());
    let go = Rc::new(move || {
        let typed = secret::read_text(&reading);
        let (vault, dialog, failing, held, from) = (vault_prove.clone(), dialog.clone(), failing.clone(), held.clone(), from.clone());
        say(&failing, &t!("Checking…"));
        glib::spawn_future_local(async move {
            let text = held.as_ref().clone();
            let opened = gio::spawn_blocking(move || bmc::read_export(text.as_bytes(), &typed).map(|contents| contents.accounts.len())).await;
            match opened {
                Ok(Ok(count)) => {
                    remember_export(&vault, from.as_deref());
                    dialog.close();
                    vault.say(&t!("Saved, and it opens: {count, plural, one {# account} other {# accounts}}.", count = count));
                }
                _ => say(&failing, &t!("That did not open it. Try again, or make a new file if you have lost the password.")),
            }
        });
    });
    let run = go.clone();
    let check = btn(&t!("Check"), None, &["primary"], move || run());
    input.connect_activate(move |_| go());
    let dialog = sheet.dialog.clone();
    let later = btn(&t!("Not now"), None, &[], move || {
        dialog.close();
    });
    sheet.pin(&widgets::actions(&[later.upcast(), check.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

/// The transfer's codes one at a time, for as long as they are looked at. Each step shows the next for another 30 seconds.
fn show_codes(vault: &Rc<Vault>, chunks: Vec<Zeroizing<String>>, from: Option<String>) {
    if chunks.is_empty() {
        vault.say(&t!("There is nothing to move."));
        return;
    }
    let sheet = widgets::sheet(&t!("Show QR codes for another device"), 560);
    sheet.body.append(&widgets::callout(&t!("Scan them all, in any order, then type the password on the other device. Hides in 30 seconds, and if you leave this window.")));
    let frame = ui::hbox(0, &["qr-frame"]);
    frame.set_halign(gtk::Align::Center);
    let code = qr::Code::new();
    code.update_property(&[gtk::accessible::Property::Label(&t!("QR code to scan with another app"))]);
    frame.append(&code);
    sheet.body.append(&frame);
    let place = ui::line("", &["soft"]);
    place.set_xalign(0.5);
    sheet.body.append(&place);
    let chunks = Rc::new(RefCell::new(chunks));
    let at = Rc::new(Cell::new(0usize));
    let token = Rc::new(Cell::new(0u64));
    let draw: Rc<dyn Fn()> = {
        let (vault, code, place, chunks, at, token) = (vault.clone(), code.clone(), place.clone(), chunks.clone(), at.clone(), token.clone());
        Rc::new(move || {
            let held = chunks.borrow();
            let Some(text) = held.get(at.get()) else { return };
            place.set_label(&t!("Code {number} of {total}", number = at.get() + 1, total = held.len()));
            let bytes = Zeroizing::new(text.as_bytes().to_vec());
            let (vault, code, token, place) = (vault.clone(), code.clone(), token.clone(), place.clone());
            glib::spawn_future_local(async move {
                let Some(modules) = qr::encode(bytes).await else { return };
                code.set(Some(modules));
                vault.secrets.release(token.get());
                let (hide_code, hide_place) = (code.clone(), place.clone());
                token.set(vault.secrets.hold(move || {
                    hide_code.set(None);
                    hide_place.set_label(&t!("Hidden. Go on to see the next."));
                }));
            });
        })
    };
    let (step_at, step_draw, total) = (at.clone(), draw.clone(), chunks.borrow().len());
    let previous = btn(&t!("Back"), None, &[], move || {
        step_at.set(step_at.get().saturating_sub(1));
        step_draw();
    });
    let (step_at, step_draw) = (at.clone(), draw.clone());
    let next = btn(&t!("Next"), None, &["primary"], move || {
        step_at.set((step_at.get() + 1).min(total - 1));
        step_draw();
    });
    let reading = chunks.clone();
    let printing = btn(&t!("Print them"), Some("print"), &[], move || {
        let held: Vec<Zeroizing<String>> = reading.borrow().clone();
        glib::spawn_future_local(async move {
            let mut pages = Vec::new();
            let total = held.len();
            for (at, chunk) in held.into_iter().enumerate() {
                if let Some(modules) = qr::encode(Zeroizing::new(chunk.as_bytes().to_vec())).await {
                    pages.push((t!("Code {number} of {total}", number = at + 1, total = total), modules));
                }
            }
            print::codes(pages);
        });
    });
    let dialog = sheet.dialog.clone();
    let done = btn(&t!("Done"), None, &[], move || {
        dialog.close();
    });
    let (vault_end, token_end, code_end, chunks_end) = (vault.clone(), token.clone(), code.clone(), chunks.clone());
    sheet.dialog.connect_closed(move |_| {
        vault_end.secrets.release(token_end.replace(0));
        code_end.set(None);
        chunks_end.borrow_mut().clear();
    });
    sheet.pin(&widgets::actions(&[printing.upcast(), done.upcast(), previous.upcast(), next.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
    remember_export(vault, from.as_deref());
    draw();
}
