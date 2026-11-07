//! Amethystora Vault's window (plans/done/PLAN-vault.md): passwords, authenticator codes, backup codes, keys, certificates and protected
//! files, in one or several vaults. The keys are the service's: this window holds the list of names, and a secret only while it is
//! shown, in memory that wipes itself, and never on the window's thread does it wait for the socket, a file or a program.

mod fido;
mod icons;
mod layout;
mod list;
mod logos;
mod measure;
mod page;
mod print;
mod qr;
mod rpc;
mod scan;
mod secret;
mod widgets;

use amethystora_ui::app::{self, Spec};
use amethystora_ui::glib;

pub fn main() -> glib::ExitCode {
    // What only the portals' helper does, which has to be readable by them (scan.rs): it holds no secret, and starts nothing else
    if let Some(code) = scan::helper() {
        return code;
    }
    // SAFETY: a plain prctl, before any thread exists. No other program of this account may read what the window shows
    unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) };
    measure::started();
    app::run(
        Spec {
            name: "vault",
            id: "org.amethystora.Vault",
            css: include_str!("vault.css"),
            width: 1120,
            height: 720,
            min_width: 560,
            min_height: 480,
        },
        page::build,
    )
}
