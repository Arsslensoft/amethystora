//! /etc/amethystora/plugins.conf, the administrator's: `ALLOW=off|granted|all` (the most any plugin may do: nothing, what a
//! person granted, and later also drafts) and `BLOCK_NETWORK=yes` (no plugin reaches the network). A file anyone but
//! root could have written, or a value that is not one of these, counts as the strictest.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub const CONF: &str = "/etc/amethystora/plugins.conf";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allow {
    Off,
    Granted,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conf {
    pub allow: Allow,
    pub block_network: bool,
}

impl Conf {
    pub const STRICTEST: Conf = Conf { allow: Allow::Off, block_network: true };
    pub const DEFAULT: Conf = Conf { allow: Allow::Granted, block_network: false };

    pub fn parse(text: &str) -> Conf {
        let mut conf = Conf::DEFAULT;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let Some((key, value)) = line.split_once('=') else { return Conf::STRICTEST };
            let value = value.trim().trim_matches('"');
            match (key.trim(), value) {
                ("ALLOW", "off") => conf.allow = Allow::Off,
                ("ALLOW", "granted") => conf.allow = Allow::Granted,
                ("ALLOW", "all") => conf.allow = Allow::All,
                ("BLOCK_NETWORK", "yes") => conf.block_network = true,
                ("BLOCK_NETWORK", "no") => conf.block_network = false,
                _ => return Conf::STRICTEST,
            }
        }
        conf
    }

    pub fn load(path: &Path) -> Conf {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.uid() != 0 || meta.mode() & 0o022 != 0 || !meta.file_type().is_file() => Conf::STRICTEST,
            Ok(_) => std::fs::read_to_string(path).map(|t| Conf::parse(&t)).unwrap_or(Conf::STRICTEST),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Conf::DEFAULT,
            Err(_) => Conf::STRICTEST,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_two_keys_and_fails_strict() {
        assert_eq!(Conf::parse(""), Conf::DEFAULT);
        assert_eq!(Conf::parse("# c\nALLOW=off\nBLOCK_NETWORK=yes\n"), Conf::STRICTEST);
        assert_eq!(Conf::parse("ALLOW=all"), Conf { allow: Allow::All, block_network: false });
        assert_eq!(Conf::parse("ALLOW=sometimes"), Conf::STRICTEST);
        assert_eq!(Conf::parse("garbage"), Conf::STRICTEST);
        assert_eq!(Conf::parse("OTHER=1"), Conf::STRICTEST);
        assert_eq!(Conf::load(Path::new("/nonexistent/plugins.conf")), Conf::DEFAULT);
    }
}
