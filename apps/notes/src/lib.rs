//! Amethystora Notes: notes and tasks, kept on this computer and nowhere else, in place of Joplin and
//! Planify. Notes are Markdown in notebooks, with tags, attachments, a history and a trash; tasks sit in the
//! inbox or in projects and their sections, with dates, repeats, reminders, priorities, labels and subtasks.
//! Nothing is synced, and nothing here opens a connection.
//!
//! backend.rs is what main.js was, and keeps the same files in the same formats (vault.rs seals them), so
//! that notes move between this window and the Electron one unchanged; import.rs, export.rs and
//! passkeys.rs are the rest of main.js, session.rs its notifications and powerMonitor. page/ is notes.js.

pub mod backend;
pub mod capture;
pub mod data;
pub mod dates;
pub mod export;
pub mod import;
pub mod passkeys;
pub mod quick;
pub mod search;
pub mod sync;
pub mod today;
pub mod vault;

mod glyphs;
#[allow(dead_code)]
mod icons;
mod layout;
mod page;
mod print;
mod session;

use amethystora_ui::app::{self, Spec};
use amethystora_ui::glib;

pub fn main() -> glib::ExitCode {
    app::run(
        Spec {
            name: "notes",
            id: "org.amethystora.Notes",
            css: include_str!("notes.css"),
            width: 1240,
            height: 820,
            min_width: 780,
            min_height: 540,
        },
        page::build,
    )
}
