//! Words: the program's own sentences through gettext (text domain `amethystora`, the catalogs of the shell programs), the
//! English being the key. What the other computer sends is never run through here: it is only counted, or its name is shown
//! as text with its control characters taken out.

use std::ffi::{CStr, CString, c_char};

unsafe extern "C" {
    fn dgettext(domain: *const c_char, msgid: *const c_char) -> *const c_char;
}

pub fn tr(msgid: &str) -> String {
    let (Ok(domain), Ok(id)) = (CString::new("amethystora"), CString::new(msgid)) else { return msgid.to_string() };
    // SAFETY: both are NUL-terminated; dgettext returns the msgid itself or a string of the catalog, which lives as long as the program
    unsafe { CStr::from_ptr(dgettext(domain.as_ptr(), id.as_ptr())).to_string_lossy().into_owned() }
}

/// `tr` with `{name}` placeholders filled in, each value put in once as it is
pub fn trf(msgid: &str, values: &[(&str, &str)]) -> String {
    let mut out = tr(msgid);
    for (name, value) in values {
        out = out.replacen(&format!("{{{name}}}"), value, 1);
    }
    out
}

/// A name another computer gave itself, as text a terminal can show: no control character, no bidi mark, at most 40 characters
pub fn clean(name: &str) -> String {
    let text: String = name.chars().filter(|c| !c.is_control() && !matches!(*c as u32, 0x200e | 0x200f | 0x202a..=0x202e | 0x2066..=0x2069)).take(40).collect();
    if text.trim().is_empty() { "?".to_string() } else { text }
}

/// A size as people read it: 38 GB, 12 MB
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 || value >= 10.0 { format!("{value:.0} {}", UNITS[unit]) } else { format!("{value:.1} {}", UNITS[unit]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_made_plain() {
        assert_eq!(clean("Sam\u{1b}[31m\u{202e}phone"), "Sam[31mphone");
        assert_eq!(clean("\n"), "?");
        assert_eq!(clean(&"x".repeat(100)).len(), 40);
    }

    #[test]
    fn sizes() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1_500), "1.5 kB");
        assert_eq!(size(38_000_000_000), "38 GB");
    }

    #[test]
    fn placeholders_fill_once() {
        assert_eq!(trf("From {name}", &[("name", "{name}")]), "From {name}");
    }
}
