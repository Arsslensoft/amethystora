//! amethystora-plugin-root: the one writer of /var/lib/amethystora/plugins, started only by pkexec with one polkit
//! action for each verb (org.amethystora.plugins.policy). See rootcmd.rs for the verbs and what each trusts.
//!
//!   pkexec /usr/libexec/amethystora-plugin-root VERB [ARGUMENT...]
//!
//! The account is pkexec's PKEXEC_UID, never an argument. A file or an archive is read from standard input and never
//! from a path the account could swap underneath it.

use std::process::ExitCode;

use amethystora_plugin_host::rootcmd::Ctx;
use amethystora_plugin_host::store::Store;

fn main() -> ExitCode {
    // SAFETY: geteuid has no arguments and cannot fail
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("amethystora-plugin-root: it runs as root, through pkexec");
        return ExitCode::from(1);
    }
    // A plugin's own files are created by this program with its own modes; nothing it writes is group or world writable
    // SAFETY: umask has no failure mode
    unsafe { libc::umask(0o022) };
    let uid = std::env::var("PKEXEC_UID").ok().and_then(|u| u.parse::<u32>().ok());
    let ctx = Ctx { store: Store::production(), uid };
    let args: Vec<String> = std::env::args().skip(1).collect();
    match ctx.run(&args, std::io::stdin().lock(), &mut std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("amethystora-plugin-root: {why}");
            ExitCode::from(1)
        }
    }
}
