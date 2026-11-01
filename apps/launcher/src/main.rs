//! amethystora-apps: every Amethystora app in one program, which the image hard-links to each app's own
//! path (/usr/lib/amethystora-<app>/amethystora-<app>), so that the app is the name it was started as.

use std::path::Path;

use amethystora_ui::glib::ExitCode;

fn main() -> ExitCode {
    let name = std::env::args_os()
        .next()
        .and_then(|arg0| Path::new(&arg0).file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_default();
    match name.as_str() {
        "amethystora-update" => amethystora_update::main(),
        "amethystora-security" => amethystora_security::main(),
        "amethystora-control" => amethystora_control::main(),
        "amethystora-backups" => amethystora_backups::main(),
        "amethystora-logs" => amethystora_logs::main(),
        "amethystora-manual" => amethystora_manual::main(),
        "amethystora-notes" => amethystora_notes::main(),
        "amethystora-phone" => amethystora_phone::main(),
        "amethystora-shelf" => amethystora_shelf::main(),
        "amethystora-trim" => amethystora_trim::main(),
        _ => {
            eprintln!("amethystora-apps: started as {name:?}, which is none of the apps; start it as one of /usr/lib/amethystora-*/amethystora-*");
            ExitCode::FAILURE
        }
    }
}
