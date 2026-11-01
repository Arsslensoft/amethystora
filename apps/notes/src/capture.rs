//! The quick note (`amethystora-notes capture`, Super+Alt+N): what is typed becomes a task in the inbox. The first
//! line is read as the task box reads it (quick.rs: its title, and the dates and the rest it understands); the rest of
//! the text, trimmed, is the task's notes. The window is page/capture.rs; the text lives in its buffer only until it
//! is saved, and nowhere else.

/// The first line and the rest, each trimmed: ("first", "second") for "first\n\n second \n"
pub fn split(text: &str) -> (String, String) {
    let text = text.trim_start();
    match text.split_once('\n') {
        Some((first, rest)) => (first.trim().to_string(), rest.trim().to_string()),
        None => (text.trim().to_string(), String::new()),
    }
}

/// The most `amethystora-notes remind` reads of its standard input, and the longest line it keeps of it
pub const REMIND_MOST: usize = 4096;
pub const REMIND_CHARS: usize = 500;

/// What `amethystora-notes remind` was handed on its standard input (the search's reminder, Amethystora UI's
/// search/remindProvider.js: "2026-10-31 17:00 Call the garage"): its first line, control and format characters taken
/// out, trimmed, at most REMIND_CHARS characters; None when nothing is left or there was more than REMIND_MOST
pub fn remind_line(bytes: &[u8]) -> Option<String> {
    if bytes.len() > REMIND_MOST {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    let first = text.split(['\n', '\r']).next().unwrap_or("");
    let kept: String = first
        .chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control() && !matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'))
        .take(REMIND_CHARS)
        .collect();
    let kept = kept.trim().to_string();
    (!kept.is_empty()).then_some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reminders_line() {
        assert_eq!(remind_line(b"2026-10-31 17:00 Call the garage\n").as_deref(), Some("2026-10-31 17:00 Call the garage"));
        assert_eq!(remind_line(b"  first\tline \nsecond").as_deref(), Some("first line"));
        assert_eq!(remind_line("a\u{202e}b\u{7}c".as_bytes()).as_deref(), Some("abc"));
        assert_eq!(remind_line(b"\n\nlater"), None);
        assert_eq!(remind_line(b"   "), None);
        assert_eq!(remind_line(&vec![b'a'; REMIND_MOST + 1]), None);
        assert_eq!(remind_line(&vec![b'a'; 2000]).map(|s| s.chars().count()), Some(REMIND_CHARS));
    }

    #[test]
    fn first_line_and_the_rest() {
        assert_eq!(split("first\n\n second \n"), ("first".to_string(), "second".to_string()));
        assert_eq!(split("only"), ("only".to_string(), String::new()));
        assert_eq!(split("\n\n  late start\nmore\nlines "), ("late start".to_string(), "more\nlines".to_string()));
        assert_eq!(split("   \n  "), (String::new(), String::new()));
    }
}
