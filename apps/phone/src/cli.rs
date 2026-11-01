//! What the window is asked to open, from its command line. The program amethystora-connect starts it with an
//! argument vector (never through a shell), and so do the module and Files' Open With, so every word is checked
//! here for its shape before anything is done with it: a device is an id the protocol allows, a notification an
//! opaque id without control characters, a file a local path or a file: address, a link an address with a
//! scheme. Nothing here talks to the bus.
//!
//! A phone chooses its own id, and its notifications' ids, so one may look like an option (start with a dash). The
//! program puts `--` before the operands of `messages`, `reply`, `pair` and `text`, which GLib's own option parser
//! would otherwise read first; GLib passes the `--` on, and parse() takes it for no operand. `send` takes its phone
//! as `--device=ID`, which an id cannot turn into another option. (The top bar's menu runs `send --device ID` and
//! `messages ID` without, and leaves out the phones whose ids start with a dash.)
//!
//!   amethystora-phone                           the phones
//!   amethystora-phone messages [--] [DEVICE]    the texts of a phone
//!   amethystora-phone send [--now] [--device=ID] [--] FILE_OR_URI...
//!                                               the send dialog, with these files, or one link; with
//!                                               --now, files to the phone named are sent at once without
//!                                               the window when nothing needs the person (the dock's drop)
//!   amethystora-phone reply [--] DEVICE NOTIFICATION
//!                                               the reply to a notification of a phone
//!   amethystora-phone pair [--] DEVICE          the pairing of a phone
//!   amethystora-phone text [--] DEVICE -- TEXT  text a phone shared, shown as text
//!   amethystora-phone ring [--device=ID]        rings the phone named, or every phone that can ring, without the
//!                                               window unless none can (`ame desktop phone ring`, search's "ring")

use std::path::Path;

use amethystora_ui::gio;
use gio::prelude::*;

/// The longest id of a device or notification this window takes on its command line
const MOST: usize = 512;

/// What is asked for
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Phones,
    Messages { device: Option<String> },
    Send { device: Option<String>, what: Sending, now: bool },
    Reply { device: String, notification: String },
    Pair { device: String },
    Text { device: String, text: String },
    Ring { device: Option<String> },
}

/// What the send dialog starts with
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sending {
    /// Nothing yet: the person chooses
    Nothing,
    /// file: addresses
    Files(Vec<String>),
    /// An address with a scheme, as it was given
    Link(String),
}

/// Why a command line was not taken
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The first word is not a command of this window
    Unknown(String),
    /// A command with what it does not take, or without what it needs
    Usage(&'static str),
    /// A device or notification id of a shape no phone has
    Id,
    /// Files and a link in one `send`, or several links
    Mixed,
}

/// A device id as the protocol has them: letters, digits, underscore and hyphen
pub fn valid_device(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A notification id is the program's own and opaque; it only has to be text a person could not mistake
pub fn valid_notification(id: &str) -> bool {
    !id.is_empty() && id.len() <= MOST && !id.chars().any(char::is_control)
}

/// An address that is a link and not a file: a scheme and `//` after it, or a mail or telephone address
pub fn is_link(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if lower.starts_with("file:") {
        return false;
    }
    if lower.starts_with("mailto:") || lower.starts_with("tel:") {
        return true;
    }
    match lower.split_once("://") {
        Some((scheme, rest)) => {
            !rest.is_empty()
                && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        }
        None => false,
    }
}

/// A file argument as the address of a local file: a file: address is kept, a path is taken from `cwd`
fn file_uri(word: &str, cwd: &Path) -> Option<String> {
    if word.is_empty() || word.contains('\0') {
        return None;
    }
    let lower = word.to_ascii_lowercase();
    if lower.starts_with("file:") {
        // GIO reads the name of another host out of an address and keeps the path, which would name a file
        // here that the address did not: only an address with no host, or this one, is a file
        if !(lower.starts_with("file:///") || lower.starts_with("file://localhost/")) {
            return None;
        }
        let file = gio::File::for_uri(word);
        return file.is_native().then(|| file.uri().to_string());
    }
    Some(gio::File::for_commandline_arg_and_cwd(word, cwd).uri().to_string())
}

/// What follows a command's name, less the `--` the program and the top bar's menu put before what a phone chose
/// (its id, a notification's id: either may start with a dash, which GLib's own option parser would take for an
/// option). GLib passes that `--` on, and it is no operand; one that was taken off already is not missed.
fn operands(rest: &[String]) -> &[String] {
    rest.strip_prefix(&["--".to_string()]).unwrap_or(rest)
}

/// The command a command line is, the program's own name left out
pub fn parse(args: &[String], cwd: &Path) -> Result<Command, Problem> {
    let Some((first, rest)) = args.split_first() else { return Ok(Command::Phones) };
    match first.as_str() {
        "phones" if rest.is_empty() => Ok(Command::Phones),
        "phones" => Err(Problem::Usage("phones")),
        "messages" => match operands(rest) {
            [] => Ok(Command::Messages { device: None }),
            [device] if valid_device(device) => Ok(Command::Messages { device: Some(device.clone()) }),
            [_] => Err(Problem::Id),
            _ => Err(Problem::Usage("messages")),
        },
        "reply" => match operands(rest) {
            [device, notification] if valid_device(device) && valid_notification(notification) => {
                Ok(Command::Reply { device: device.clone(), notification: notification.clone() })
            }
            [_, _] => Err(Problem::Id),
            _ => Err(Problem::Usage("reply")),
        },
        "pair" => match operands(rest) {
            [device] if valid_device(device) => Ok(Command::Pair { device: device.clone() }),
            [_] => Err(Problem::Id),
            _ => Err(Problem::Usage("pair")),
        },
        "text" => {
            // `text DEVICE -- TEXT`: the `--` keeps a text that starts with a dash from being taken for an option, and
            // is no part of it
            let [device, tail @ ..] = operands(rest) else { return Err(Problem::Usage("text")) };
            if !valid_device(device) {
                return Err(Problem::Id);
            }
            match tail {
                [text] | [_, text] if !text.is_empty() && (tail.len() == 1 || tail[0] == "--") => Ok(Command::Text { device: device.clone(), text: text.clone() }),
                _ => Err(Problem::Usage("text")),
            }
        }
        "send" => send(rest, cwd),
        "ring" => match rest {
            [] => Ok(Command::Ring { device: None }),
            [option, id] if option == "--device" || option == "-d" => ring_device(id),
            [word] => match word.strip_prefix("--device=") {
                Some(id) => ring_device(id),
                None => Err(Problem::Usage("ring")),
            },
            _ => Err(Problem::Usage("ring")),
        },
        other => Err(Problem::Unknown(other.chars().take(40).collect())),
    }
}

fn ring_device(id: &str) -> Result<Command, Problem> {
    if valid_device(id) { Ok(Command::Ring { device: Some(id.to_string()) }) } else { Err(Problem::Id) }
}

fn send(rest: &[String], cwd: &Path) -> Result<Command, Problem> {
    let mut device = None;
    let mut now = false;
    let mut words: Vec<&String> = Vec::new();
    let mut options = true;
    let mut at = 0;
    while at < rest.len() {
        let word = &rest[at];
        at += 1;
        if options && word == "--" {
            options = false;
        } else if options && word == "--now" {
            now = true;
        } else if options && (word == "--device" || word == "-d") {
            let Some(id) = rest.get(at) else { return Err(Problem::Usage("send")) };
            at += 1;
            device = Some(id.clone());
        } else if let Some(id) = options.then(|| word.strip_prefix("--device=")).flatten() {
            device = Some(id.to_string());
        } else if options && word.starts_with('-') && word.len() > 1 {
            return Err(Problem::Usage("send"));
        } else {
            words.push(word);
        }
    }
    if device.as_deref().is_some_and(|id| !valid_device(id)) {
        return Err(Problem::Id);
    }
    let links: Vec<&&String> = words.iter().filter(|word| is_link(word)).collect();
    let what = if words.is_empty() {
        Sending::Nothing
    } else if links.is_empty() {
        let mut files = Vec::new();
        for word in &words {
            match file_uri(word, cwd) {
                Some(uri) if !files.contains(&uri) => files.push(uri),
                Some(_) => {}
                None => return Err(Problem::Usage("send")),
            }
        }
        Sending::Files(files)
    } else if links.len() == 1 && words.len() == 1 {
        Sending::Link(words[0].clone())
    } else {
        return Err(Problem::Mixed);
    };
    Ok(Command::Send { device, what, now })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_string).collect()
    }

    fn cwd() -> &'static Path {
        Path::new("/home/ada/Pictures")
    }

    #[test]
    fn nothing_opens_the_phones() {
        assert_eq!(parse(&[], cwd()), Ok(Command::Phones));
        assert_eq!(parse(&words("phones"), cwd()), Ok(Command::Phones));
        assert_eq!(parse(&words("phones now"), cwd()), Err(Problem::Usage("phones")));
    }

    #[test]
    fn messages_take_a_device_or_none() {
        assert_eq!(parse(&words("messages"), cwd()), Ok(Command::Messages { device: None }));
        assert_eq!(parse(&words("messages a1_b2-c3"), cwd()), Ok(Command::Messages { device: Some("a1_b2-c3".into()) }));
        assert_eq!(parse(&words("messages a/b"), cwd()), Err(Problem::Id));
        assert_eq!(parse(&words("messages a b"), cwd()), Err(Problem::Usage("messages")));
    }

    #[test]
    fn reply_and_pair_need_their_ids() {
        assert_eq!(
            parse(&["reply".into(), "dev1".into(), "dev1|0|com.chat|42".into()], cwd()),
            Ok(Command::Reply { device: "dev1".into(), notification: "dev1|0|com.chat|42".into() })
        );
        assert_eq!(parse(&words("reply dev1"), cwd()), Err(Problem::Usage("reply")));
        assert_eq!(parse(&["reply".into(), "dev1".into(), "a\nb".into()], cwd()), Err(Problem::Id));
        assert_eq!(parse(&["reply".into(), "../x".into(), "n".into()], cwd()), Err(Problem::Id));
        assert_eq!(parse(&words("pair dev1"), cwd()), Ok(Command::Pair { device: "dev1".into() }));
        assert_eq!(parse(&words("pair"), cwd()), Err(Problem::Usage("pair")));
        assert_eq!(parse(&words("pair a;b"), cwd()), Err(Problem::Id));
    }

    #[test]
    fn ring_takes_a_phone_or_none() {
        assert_eq!(parse(&words("ring"), cwd()), Ok(Command::Ring { device: None }));
        assert_eq!(parse(&words("ring --device=dev1"), cwd()), Ok(Command::Ring { device: Some("dev1".into()) }));
        assert_eq!(parse(&words("ring --device dev1"), cwd()), Ok(Command::Ring { device: Some("dev1".into()) }));
        assert_eq!(parse(&words("ring --device=a/b"), cwd()), Err(Problem::Id));
        assert_eq!(parse(&words("ring dev1"), cwd()), Err(Problem::Usage("ring")));
        assert_eq!(parse(&words("ring --device"), cwd()), Err(Problem::Usage("ring")));
    }

    #[test]
    fn a_text_is_the_word_after_the_phone() {
        // The `--` the program puts before it is no part of it, and one that GLib took off is not missed
        assert_eq!(parse(&["text".into(), "dev1".into(), "--version".into()], cwd()), Ok(Command::Text { device: "dev1".into(), text: "--version".into() }));
        assert_eq!(parse(&["text".into(), "dev1".into(), "--".into(), "--version".into()], cwd()), Ok(Command::Text { device: "dev1".into(), text: "--version".into() }));
        assert_eq!(parse(&["text".into(), "--".into(), "dev1".into(), "--".into(), "--version".into()], cwd()), Ok(Command::Text { device: "dev1".into(), text: "--version".into() }));
        assert_eq!(parse(&["text".into(), "dev1".into(), "Dinner at 8?".into()], cwd()), Ok(Command::Text { device: "dev1".into(), text: "Dinner at 8?".into() }));
        assert_eq!(parse(&words("text dev1 a b"), cwd()), Err(Problem::Usage("text")));
        assert_eq!(parse(&["text".into(), "dev1".into(), String::new()], cwd()), Err(Problem::Usage("text")));
        assert_eq!(parse(&words("text dev1"), cwd()), Err(Problem::Usage("text")));
        assert_eq!(parse(&words("text"), cwd()), Err(Problem::Usage("text")));
        assert_eq!(parse(&words("text ../d x"), cwd()), Err(Problem::Id));
    }

    #[test]
    fn send_takes_files_from_where_the_launch_was_made() {
        let Ok(Command::Send { device, what: Sending::Files(files), .. }) = parse(&words("send --device dev1 a.jpg /tmp/b.txt file:///tmp/c.txt a.jpg"), cwd()) else {
            panic!("files")
        };
        assert_eq!(device.as_deref(), Some("dev1"));
        assert_eq!(files, ["file:///home/ada/Pictures/a.jpg", "file:///tmp/b.txt", "file:///tmp/c.txt"]);
    }

    #[test]
    fn send_without_words_asks_the_person() {
        assert_eq!(parse(&words("send"), cwd()), Ok(Command::Send { device: None, what: Sending::Nothing, now: false }));
        assert_eq!(parse(&words("send -d dev1"), cwd()), Ok(Command::Send { device: Some("dev1".into()), what: Sending::Nothing, now: false }));
        assert_eq!(parse(&words("send --device=dev1"), cwd()), Ok(Command::Send { device: Some("dev1".into()), what: Sending::Nothing, now: false }));
    }

    #[test]
    fn send_now_is_an_option_before_the_files() {
        // As the dock runs it, with the phone as --device=ID or --device ID, before or after it
        let file = |line: &str| match parse(&words(line), cwd()) {
            Ok(Command::Send { device, what: Sending::Files(files), now }) => (device, files, now),
            other => panic!("{line}: {other:?}"),
        };
        let sent = (Some("dev1".to_string()), vec!["file:///tmp/a.txt".to_string()], true);
        assert_eq!(file("send --now --device=dev1 -- /tmp/a.txt"), sent);
        assert_eq!(file("send --now --device dev1 -- /tmp/a.txt"), sent);
        assert_eq!(file("send --device=dev1 --now -- /tmp/a.txt"), sent);
        assert_eq!(file("send --device dev1 --now /tmp/a.txt"), sent);
        // After the dashes it is a file
        assert_eq!(file("send --device=dev1 -- --now"), (Some("dev1".to_string()), vec!["file:///home/ada/Pictures/--now".to_string()], false));
        // Without it, the dialog waits for the person as before
        assert_eq!(file("send --device=dev1 -- /tmp/a.txt").2, false);
        assert_eq!(parse(&words("send --now"), cwd()), Ok(Command::Send { device: None, what: Sending::Nothing, now: true }));
    }

    #[test]
    fn send_takes_one_link_or_files_never_both() {
        assert_eq!(
            parse(&words("send https://example.org/a?b=1"), cwd()),
            Ok(Command::Send { device: None, what: Sending::Link("https://example.org/a?b=1".into()), now: false })
        );
        assert_eq!(parse(&words("send https://a.example https://b.example"), cwd()), Err(Problem::Mixed));
        assert_eq!(parse(&words("send a.jpg https://a.example"), cwd()), Err(Problem::Mixed));
    }

    #[test]
    fn send_refuses_what_it_does_not_know() {
        assert_eq!(parse(&words("send --frobnicate a.jpg"), cwd()), Err(Problem::Usage("send")));
        assert_eq!(parse(&words("send --device"), cwd()), Err(Problem::Usage("send")));
        assert_eq!(parse(&words("send --device ../x a.jpg"), cwd()), Err(Problem::Id));
        // After the dashes a name that starts with a dash is a file
        let Ok(Command::Send { what: Sending::Files(files), .. }) = parse(&words("send -- -odd.txt"), cwd()) else { panic!("a file") };
        assert_eq!(files, ["file:///home/ada/Pictures/-odd.txt"]);
        // A remote address is no file
        assert_eq!(parse(&words("send file://example.org/etc/passwd"), cwd()), Err(Problem::Usage("send")));
    }

    #[test]
    fn an_id_that_starts_with_a_dash_is_an_id() {
        // As the program starts it (`messages -- -ID`, which GLib passes on with the `--`), and as GLib leaves it if it takes the
        // `--` off, and the forms with the id in an option
        let dashed = "-0a1b2c3d_4e5f-6a7b8c9d0e1f2a3b4c5d";
        for with in [vec![], vec!["--".to_string()]] {
            let said = |command: &str, operands: &[&str]| -> Vec<String> {
                [vec![command.to_string()], with.clone(), operands.iter().map(|o| o.to_string()).collect()].concat()
            };
            assert_eq!(parse(&said("messages", &[dashed]), cwd()), Ok(Command::Messages { device: Some(dashed.into()) }));
            assert_eq!(parse(&said("messages", &[]), cwd()), Ok(Command::Messages { device: None }));
            assert_eq!(parse(&said("pair", &[dashed]), cwd()), Ok(Command::Pair { device: dashed.into() }));
            assert_eq!(
                parse(&said("reply", &[dashed, "-1|0|-pkg|-3"]), cwd()),
                Ok(Command::Reply { device: dashed.into(), notification: "-1|0|-pkg|-3".into() })
            );
        }
        assert_eq!(parse(&["messages".into(), dashed.into()], cwd()), Ok(Command::Messages { device: Some(dashed.into()) }));
        assert_eq!(parse(&["pair".into(), dashed.into()], cwd()), Ok(Command::Pair { device: dashed.into() }));
        assert_eq!(
            parse(&["reply".into(), dashed.into(), "--gapplication-service".into()], cwd()),
            Ok(Command::Reply { device: dashed.into(), notification: "--gapplication-service".into() })
        );
        assert_eq!(
            parse(&["reply".into(), dashed.into(), "-1|0|-pkg|-3".into()], cwd()),
            Ok(Command::Reply { device: dashed.into(), notification: "-1|0|-pkg|-3".into() })
        );
        let Ok(Command::Send { device, what: Sending::Files(files), .. }) = parse(&["send".into(), format!("--device={dashed}"), "/tmp/a.txt".into()], cwd()) else {
            panic!("files")
        };
        assert_eq!((device.as_deref(), files.as_slice()), (Some(dashed), ["file:///tmp/a.txt".to_string()].as_slice()));
        let Ok(Command::Send { device, .. }) = parse(&["send".into(), "--device".into(), dashed.into()], cwd()) else { panic!("a device") };
        assert_eq!(device.as_deref(), Some(dashed));
        // An id is still only letters, digits, underscores and hyphens
        assert_eq!(parse(&["messages".into(), "-a b".into()], cwd()), Err(Problem::Id));
        assert_eq!(parse(&["pair".into(), "-a;b".into()], cwd()), Err(Problem::Id));
    }

    #[test]
    fn unknown_commands_are_said() {
        assert_eq!(parse(&words("rm"), cwd()), Err(Problem::Unknown("rm".into())));
        assert_eq!(parse(&["x".repeat(100)], cwd()), Err(Problem::Unknown("x".repeat(40))));
    }

    #[test]
    fn links_have_a_scheme_and_files_do_not() {
        for link in ["https://example.org", "HTTP://EXAMPLE.ORG", "mailto:a@example.org", "tel:+15550100", "ssh://host"] {
            assert!(is_link(link), "{link}");
        }
        for file in ["a.jpg", "/tmp/a:b", "file:///tmp/a", "C:\\x", "http:", "://x", "1http://x", "a b"] {
            assert!(!is_link(file), "{file}");
        }
    }
}
