//! A plugin's manifest.json: what it is, what it contributes and what it asks for. Everything here is a refusal or a
//! normal form: a permission is one short string (`network=api.github.com`, `notify`, `files=~/Documents:ro`), the same
//! text the helper is given at consent, the grants keep and the dialog shows, so that one definition reads them all.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

pub const MAX_MANIFEST: usize = 64 * 1024;
/// What a host may be sent in an hour when the manifest asks for no more, and the most it may ask for
pub const DEFAULT_LIMIT: u64 = 1 << 20;
pub const MAX_LIMIT: u64 = 64 << 20;
const SURFACES: &[&str] = &["pill", "bar", "desktop", "search", "tile", "dock", "today", "menu", "files-menu"];
const TOP: &[&str] = &["schema", "id", "name", "version", "license", "author", "command", "interval", "contributes", "needs", "actions", "settings"];
/// Names `state`, `control` and `events` may list (the broker's, a later phase; the names are the plan's table)
const STATES: &[&str] = &["clock", "disk", "media", "do-not-disturb", "network", "power", "screen", "theme", "battery", "volume"];
const CONTROLS: &[&str] = &["media", "do-not-disturb"];
const EVENTS: &[&str] = &["network", "power", "screen", "theme", "focused-app"];

#[derive(Debug, Clone, PartialEq)]
pub struct Need {
    /// The normal form, which is what is granted
    pub perm: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    pub name: String,
    pub label: String,
    pub secret: bool,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub license: String,
    pub command: Vec<String>,
    pub interval: Option<u64>,
    pub contributes: Vec<String>,
    /// Where each surface is drawn, with its own options, as the shell reads them: `{"bar": {"position": "right"}, "pill": {}}`
    pub places: Map<String, Value>,
    /// What the maker says of itself (shown as the plugin's own words, never trusted): the agent's drafts say "Agent"
    pub author: String,
    /// Settings a person gives it: a name, and whether it is a secret, which is kept under a data key in the Vault
    pub settings: Vec<Setting>,
    pub actions: Vec<String>,
    pub needs: Vec<Need>,
    /// Bytes an hour a host may be sent, for the hosts that asked for more than the default
    pub limits: Vec<(String, u64)>,
}

fn printable(text: &str, max: usize) -> bool {
    !text.is_empty() && text.chars().count() <= max && !text.chars().any(|c| c.is_control() || is_bidi(c))
}

/// Bidirectional controls and zero-width characters, which make one text read as another
pub fn is_bidi(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}' | '\u{061C}')
}

/// A reverse-domain id: two or more labels of lower-case letters, digits and hyphens. `amethystora.` is the system's
pub fn valid_id(id: &str, system: bool) -> Result<(), String> {
    let labels: Vec<&str> = id.split('.').collect();
    let ok = id.len() <= 64
        && labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty() && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
    if !ok {
        return Err("\"id\" has to be a reverse-domain name such as dev.example.ci-build (lower case, at most 64 characters)".into());
    }
    if !system && id.starts_with("amethystora.") {
        return Err("ids starting with amethystora. are the system's".into());
    }
    Ok(())
}

pub fn valid_action(action: &str) -> bool {
    (1..=32).contains(&action.len()) && action.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A named host: letters, digits, hyphens and dots, one `*.` in front at most, never an address
pub fn valid_host(host: &str) -> bool {
    let name = host.strip_prefix("*.").unwrap_or(host);
    let labels: Vec<&str> = name.split('.').collect();
    host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        // 1.2.3.4 is an address, not a name; the last label of a name is never all digits
        && !labels.last().is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `host` (lower case) is covered by an allowed entry: the name itself, or a subdomain of `*.name`
pub fn host_matches(entry: &str, host: &str) -> bool {
    match entry.strip_prefix("*.") {
        Some(base) => host.len() > base.len() + 1 && host.ends_with(base) && host.as_bytes()[host.len() - base.len() - 1] == b'.',
        None => entry == host,
    }
}

/// Folders of the home that start programs: a plugin that could write there would plant what the account then runs. Never given read-write
pub fn starts_programs(first: &str) -> bool {
    matches!(first, "bin" | "sbin" | "Applications" | "AppImages")
}

/// A path in the home a plugin may be given: `~/Documents/notes`, with no dot-folder, no `..` and no `Private`
pub fn valid_home_path(path: &str) -> Result<(), String> {
    let Some(rest) = path.strip_prefix("~/") else {
        return Err("a path has to start with ~/ (a place under the home folder)".into());
    };
    let parts: Vec<&str> = rest.split('/').collect();
    if rest.is_empty() || path.len() > 200 || parts.iter().any(|p| p.is_empty()) {
        return Err("that is the home folder itself, or an empty name".into());
    }
    if parts.iter().any(|p| p.starts_with('.')) {
        return Err("hidden folders and files, and .., are never given to a plugin".into());
    }
    if parts[0] == "Private" {
        return Err("the private folder is never given to a plugin".into());
    }
    // What a permission's words can say without a space or a quote: the dialog has to read what runs
    if !path.bytes().all(|b| b.is_ascii_alphanumeric() || b"~/_-+@".contains(&b)) {
        return Err("a path may use letters, digits and _ - + @ only".into());
    }
    Ok(())
}

fn text(value: &Value, what: &str, max: usize) -> Result<String, String> {
    let Some(s) = value.as_str().filter(|s| printable(s, max)) else {
        return Err(format!("\"{what}\" has to be text of at most {max} characters, with no control characters"));
    };
    Ok(s.trim().to_string())
}

/// What a surface may say about where it is drawn: nothing else is read, so nothing else can be asked for
fn surface_options(surface: &str, m: &Map<String, Value>) -> Result<Value, String> {
    let what = format!("contributes.{surface}");
    let pick = |key: &str, allowed: &[&str]| -> Result<Option<String>, String> {
        match m.get(key) {
            None => Ok(None),
            Some(v) => match v.as_str() {
                Some(s) if allowed.contains(&s) => Ok(Some(s.to_string())),
                _ => Err(format!("\"{what}.{key}\" has to be one of {}", allowed.join(", "))),
            },
        }
    };
    let mut out = Map::new();
    match surface {
        "bar" => {
            only(m, &["position", "order"], &what)?;
            out.insert("position".into(), pick("position", &["left", "center", "right"])?.unwrap_or_else(|| "right".into()).into());
            let order = m.get("order").map(|v| v.as_i64().filter(|n| (-100..=100).contains(n)).ok_or("\"order\" has to be a whole number from -100 to 100")).transpose()?;
            out.insert("order".into(), order.unwrap_or(0).into());
        }
        "desktop" => {
            only(m, &["corner"], &what)?;
            out.insert("corner".into(), pick("corner", &["top-left", "top-right", "bottom-left", "bottom-right"])?.unwrap_or_else(|| "top-right".into()).into());
        }
        "search" => {
            only(m, &["prefix"], &what)?;
            let prefix = m.get("prefix").map(|v| text(v, "search.prefix", 16)).transpose()?.unwrap_or_default();
            if !prefix.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b":-_".contains(&b)) {
                return Err("a search prefix uses lower-case letters, digits, : - and _".into());
            }
            out.insert("prefix".into(), prefix.into());
        }
        "dock" => {
            only(m, &["app"], &what)?;
            let app = text(m.get("app").unwrap_or(&Value::Null), "dock.app", 128)?;
            if !app.ends_with(".desktop") || !app.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) || app.starts_with('.') {
                return Err("\"dock.app\" has to be an app's launcher, such as org.gnome.Nautilus.desktop".into());
            }
            // A count on Amethystora's own apps (Security, Updates, the Vault) or on Settings would pass for the system's
            if app.starts_with("amethystora-") || app.starts_with("org.amethystora.") || app == "org.gnome.Settings.desktop" {
                return Err("\"dock.app\" cannot be one of Amethystora's own apps or Settings: a badge there would pass for the system's".into());
            }
            out.insert("app".into(), app.into());
        }
        // Files' Scripts menu: the files a person gives it there reach the action named, through `plugin pick` (it needs `pick`)
        "files-menu" => {
            only(m, &["action"], &what)?;
            let action = text(m.get("action").unwrap_or(&Value::Null), "files-menu.action", 32)?;
            if !valid_action(&action) {
                return Err("\"files-menu.action\" has to be one of the plugin's actions".into());
            }
            out.insert("action".into(), action.into());
        }
        _ => only(m, &[], &what)?,
    }
    Ok(Value::Object(out))
}

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("\"{what}\" has to be an object"))
}

fn why(map: &Map<String, Value>, what: &str) -> Result<String, String> {
    match map.get("why") {
        Some(v) => text(v, &format!("{what}.why"), 160),
        None => Err(format!("{what} needs a \"why\": the sentence a person reads before granting it")),
    }
}

fn only(map: &Map<String, Value>, known: &[&str], what: &str) -> Result<(), String> {
    match map.keys().find(|k| !known.contains(&k.as_str())) {
        Some(k) => Err(format!("unknown field \"{k}\" in {what}")),
        None => Ok(()),
    }
}

fn names(map: &Map<String, Value>, allowed: &[&str], what: &str) -> Result<Vec<String>, String> {
    let Some(list) = map.get("names").and_then(Value::as_array).filter(|l| !l.is_empty() && l.len() <= 16) else {
        return Err(format!("{what} needs \"names\": a list of one to sixteen names"));
    };
    list.iter()
        .map(|n| match n.as_str() {
            Some(s) if allowed.contains(&s) => Ok(s.to_string()),
            _ => Err(format!("{what} has a name that is not one of {}", allowed.join(", "))),
        })
        .collect()
}

impl Manifest {
    pub fn parse(source: &str) -> Result<Manifest, String> {
        Self::parse_with(source, false)
    }

    /// `system` lets the id start with amethystora., which only the image's own plugins in /usr/share/amethystora/plugins do
    pub fn parse_with(source: &str, system: bool) -> Result<Manifest, String> {
        if source.len() > MAX_MANIFEST {
            return Err("manifest.json is larger than 64 KiB".into());
        }
        let value: Value = serde_json::from_str(source).map_err(|e| format!("manifest.json is not JSON: {e}"))?;
        let top = object(&value, "manifest.json")?;
        only(top, TOP, "manifest.json")?;
        if top.get("schema").and_then(Value::as_u64) != Some(1) {
            return Err("\"schema\" has to be 1".into());
        }
        let id = text(top.get("id").unwrap_or(&Value::Null), "id", 64)?;
        valid_id(&id, system)?;
        let name = text(top.get("name").unwrap_or(&Value::Null), "name", 40)?;
        let version = text(top.get("version").unwrap_or(&Value::Null), "version", 32)?;
        let core = version.split(['-', '+']).next().unwrap_or("");
        let numbers: Vec<&str> = core.split('.').collect();
        if numbers.len() != 3 || !numbers.iter().all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) {
            return Err("\"version\" has to look like 1.2.0".into());
        }
        if !version.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b)) {
            return Err("\"version\" may use letters, digits and . - + only".into());
        }
        let license = text(top.get("license").unwrap_or(&Value::Null), "license", 64)?;
        let command: Vec<String> = match top.get("command").and_then(Value::as_array) {
            Some(list) if !list.is_empty() && list.len() <= 16 => list.iter().map(|a| text(a, "command", 256)).collect::<Result<_, _>>()?,
            _ => return Err("\"command\" has to be the program and its arguments, as a list".into()),
        };
        // The program is a file of the plugin's own folder, which is the sandbox's working folder
        let program = &command[0];
        if !program.starts_with("./") || program.contains("//") || program.split('/').any(|p| p == "..") {
            return Err("the first word of \"command\" has to be a file of the plugin, written ./name, inside its folder".into());
        }
        let interval = match top.get("interval") {
            None => None,
            Some(v) => match v.as_u64() {
                Some(n) if (5..=86400).contains(&n) => Some(n),
                _ => return Err("\"interval\" has to be a whole number of seconds from 5 to 86400".into()),
            },
        };
        let mut contributes = Vec::new();
        let mut places = Map::new();
        if let Some(c) = top.get("contributes") {
            for (surface, value) in object(c, "contributes")? {
                if !SURFACES.contains(&surface.as_str()) {
                    return Err(format!("\"contributes\" has no surface called \"{surface}\" (one of {})", SURFACES.join(", ")));
                }
                places.insert(surface.clone(), surface_options(surface, object(value, &format!("contributes.{surface}"))?)?);
                contributes.push(surface.clone());
            }
        }
        let author = match top.get("author") {
            None => String::new(),
            Some(v) => text(v, "author", 80)?,
        };
        let mut settings = Vec::new();
        if let Some(list) = top.get("settings") {
            let list = list.as_array().filter(|l| l.len() <= 8).ok_or("\"settings\" has to be a list of at most eight")?;
            for item in list {
                let m = object(item, "a setting")?;
                only(m, &["name", "label", "secret"], "a setting")?;
                let name = text(m.get("name").unwrap_or(&Value::Null), "setting name", 32)?;
                if !valid_action(&name) {
                    return Err("a setting's name uses lower-case letters, digits and hyphens".into());
                }
                let label = text(m.get("label").unwrap_or(&Value::Null), "setting label", 60)?;
                settings.push(Setting { name, label, secret: m.get("secret").and_then(Value::as_bool).unwrap_or(false) });
            }
        }
        let mut actions = Vec::new();
        if let Some(a) = top.get("actions") {
            let list = a.as_array().filter(|l| l.len() <= 16).ok_or("\"actions\" has to be a list of at most sixteen ids")?;
            for item in list {
                match item.as_str() {
                    Some(s) if valid_action(s) => actions.push(s.to_string()),
                    _ => return Err("an action id uses lower-case letters, digits and hyphens, at most 32".into()),
                }
            }
        }
        let (needs, limits) = match top.get("needs") {
            None => (Vec::new(), Vec::new()),
            Some(n) => parse_needs(object(n, "needs")?)?,
        };
        // A surface that runs an action names one the manifest declares, and one that hands over a file needs `pick`
        for (surface, options) in &places {
            if let Some(action) = options.get("action").and_then(Value::as_str) {
                if !actions.iter().any(|a| a == action) {
                    return Err(format!("\"contributes.{surface}.action\" is {action:?}, which is not in \"actions\""));
                }
            }
        }
        if places.contains_key("files-menu") && !needs.iter().any(|n| n.perm == "pick") {
            return Err("\"files-menu\" hands a file over, which needs \"pick\" in \"needs\"".into());
        }
        Ok(Manifest { id, name, version, license, command, interval, contributes, places, author, settings, actions, needs, limits })
    }

    /// The normal forms of everything it asks for, sorted: what consent lists, and what a grant is a subset of
    pub fn permissions(&self) -> BTreeSet<String> {
        self.needs.iter().map(|n| n.perm.clone()).collect()
    }

    pub fn why_of(&self, perm: &str) -> Option<&str> {
        self.needs.iter().find(|n| n.perm == perm).map(|n| n.why.as_str())
    }
}

type Parsed = (Vec<Need>, Vec<(String, u64)>);

fn parse_needs(needs: &Map<String, Value>) -> Result<Parsed, String> {
    let mut out = Vec::new();
    let mut limits = Vec::new();
    let mut add = |perm: String, why: String| out.push(Need { perm, why });
    for (key, value) in needs {
        match key.as_str() {
            "network" => {
                let list = value.as_array().filter(|l| !l.is_empty() && l.len() <= 16).ok_or("\"network\" has to be a list of hosts")?;
                for item in list {
                    let m = object(item, "network entry")?;
                    only(m, &["host", "why", "limit"], "a network entry")?;
                    let host = m.get("host").and_then(Value::as_str).unwrap_or("").to_string();
                    if !valid_host(&host) {
                        return Err(format!("\"{host}\" is not a host name (an address, \"any\" and a port are never accepted)"));
                    }
                    if let Some(l) = m.get("limit") {
                        match l.as_u64() {
                            Some(n) if (1..=MAX_LIMIT).contains(&n) => limits.push((host.clone(), n)),
                            _ => return Err("\"limit\" is bytes an hour, up to 64 MiB".into()),
                        }
                    }
                    add(format!("network={host}"), why(m, "a network entry")?);
                }
            }
            "network-local" => {
                let list = value.as_array().filter(|l| !l.is_empty() && l.len() <= 8).ok_or("\"network-local\" has to be a list")?;
                for item in list {
                    let m = object(item, "network-local entry")?;
                    only(m, &["host", "port", "why"], "a network-local entry")?;
                    let host = m.get("host").and_then(Value::as_str).unwrap_or("");
                    let port = m.get("port").and_then(Value::as_u64).filter(|p| (1..=65535).contains(p));
                    let host_ok = valid_host(host) || host.parse::<std::net::Ipv4Addr>().is_ok();
                    let Some(port) = port.filter(|_| host_ok) else {
                        return Err("a network-local entry has a host name or an IPv4 address, and a port".into());
                    };
                    add(format!("network-local={host}:{port}"), why(m, "a network-local entry")?);
                }
            }
            "files" => {
                let list = value.as_array().filter(|l| !l.is_empty() && l.len() <= 8).ok_or("\"files\" has to be a list")?;
                for item in list {
                    let m = object(item, "files entry")?;
                    only(m, &["path", "mode", "why"], "a files entry")?;
                    let path = m.get("path").and_then(Value::as_str).unwrap_or("");
                    valid_home_path(path)?;
                    let mode = match m.get("mode").and_then(Value::as_str) {
                        None | Some("ro") => "ro",
                        Some("rw") => "rw",
                        _ => return Err("a files entry's \"mode\" is ro or rw".into()),
                    };
                    if mode == "rw" && starts_programs(path.trim_start_matches("~/").split('/').next().unwrap_or("")) {
                        return Err("a folder that starts programs is never given read-write".into());
                    }
                    add(format!("files={path}:{mode}"), why(m, "a files entry")?);
                }
            }
            "state" | "control" | "events" | "change" => {
                let m = object(value, key)?;
                only(m, &["names", "why"], key)?;
                let list = if key == "change" {
                    // A Control entry's id: the agent's change path and its never list decide each one, later
                    let l = m.get("names").and_then(Value::as_array).filter(|l| !l.is_empty() && l.len() <= 8).ok_or("\"change\" needs \"names\"")?;
                    l.iter()
                        .map(|n| match n.as_str() {
                            Some(s) if valid_action(s) => Ok(s.to_string()),
                            _ => Err("a change name is a Control entry's id".to_string()),
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    let allowed = match key.as_str() {
                        "state" => STATES,
                        "control" => CONTROLS,
                        _ => EVENTS,
                    };
                    names(m, allowed, key)?
                };
                let reason = why(m, key)?;
                for n in list {
                    add(format!("{key}={n}"), reason.clone());
                }
            }
            "pick" | "notify" | "open" | "clipboard-write" | "window" => {
                let m = object(value, key)?;
                only(m, &["why"], key)?;
                add(key.clone(), why(m, key)?);
            }
            other => return Err(format!("\"{other}\" is not a permission a plugin can ask for")),
        }
    }
    Ok((out, limits))
}

/// What no shipped plugin may carry: its grant needs no administrator, so it may reach nothing outside the machine's own state
pub fn shipped_refuses(perm: &str) -> bool {
    ["network", "network-local", "files", "window", "change"].iter().any(|p| perm == *p || perm.starts_with(&format!("{p}=")))
}

/// Whether a permission argument has the shape the helper takes: a short set of characters, so that the dialog's text
/// is what runs
pub fn valid_perm_arg(arg: &str) -> bool {
    (1..=256).contains(&arg.len()) && arg.bytes().all(|b| b.is_ascii_alphanumeric() || b"._/@:*=,+-~".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"schema":1,"id":"dev.example.ci-build","name":"CI build","version":"1.2.0","license":"MIT","command":["./run.sh"],
        "interval":60,"contributes":{"pill":{},"bar":{"position":"right"}},"actions":["open-page"],
        "needs":{"network":[{"host":"api.github.com","why":"Reads the build's status"}],"notify":{"why":"Says when a build fails"},
        "state":{"names":["clock","disk"],"why":"Shows the time"},"files":[{"path":"~/Documents/ci","why":"Reads its notes"}]}}"#;

    fn with(edit: impl Fn(&mut Value)) -> String {
        let mut v: Value = serde_json::from_str(GOOD).unwrap();
        edit(&mut v);
        v.to_string()
    }

    #[test]
    fn reads_a_good_manifest() {
        let m = Manifest::parse(GOOD).unwrap();
        assert_eq!(m.id, "dev.example.ci-build");
        let p: Vec<String> = m.permissions().into_iter().collect();
        assert_eq!(p, ["files=~/Documents/ci:ro", "network=api.github.com", "notify", "state=clock", "state=disk"]);
        assert_eq!(m.why_of("notify"), Some("Says when a build fails"));
    }

    #[test]
    fn refuses_every_bad_shape() {
        type Edit = Box<dyn Fn(&mut Value)>;
        let cases: Vec<(&str, Edit)> = vec![
            ("amethystora. id", Box::new(|v| v["id"] = "amethystora.pill".into())),
            ("one label", Box::new(|v| v["id"] = "plain".into())),
            ("upper case id", Box::new(|v| v["id"] = "Dev.Example".into())),
            ("schema", Box::new(|v| v["schema"] = 2.into())),
            ("version", Box::new(|v| v["version"] = "latest".into())),
            ("name with control", Box::new(|v| v["name"] = "a\u{7}b".into())),
            ("name with bidi", Box::new(|v| v["name"] = "a\u{202E}b".into())),
            ("long name", Box::new(|v| v["name"] = "x".repeat(41).into())),
            ("command empty", Box::new(|v| v["command"] = serde_json::json!([]))),
            ("command not relative", Box::new(|v| v["command"] = serde_json::json!(["/bin/sh"]))),
            ("command leaves", Box::new(|v| v["command"] = serde_json::json!(["./../x"]))),
            ("interval low", Box::new(|v| v["interval"] = 1.into())),
            ("unknown field", Box::new(|v| v["extra"] = 1.into())),
            ("unknown surface", Box::new(|v| v["contributes"] = serde_json::json!({"lock-screen": {}}))),
            ("unknown permission", Box::new(|v| v["needs"]["root"] = serde_json::json!({"why": "x"}))),
            ("no why", Box::new(|v| v["needs"]["notify"] = serde_json::json!({}))),
            ("no why on network", Box::new(|v| v["needs"]["network"] = serde_json::json!([{"host": "a.example.com"}]))),
            ("address host", Box::new(|v| v["needs"]["network"] = serde_json::json!([{"host": "192.168.1.1", "why": "x"}]))),
            ("any host", Box::new(|v| v["needs"]["network"] = serde_json::json!([{"host": "any", "why": "x"}]))),
            ("port in host", Box::new(|v| v["needs"]["network"] = serde_json::json!([{"host": "a.example.com:22", "why": "x"}]))),
            ("huge limit", Box::new(|v| v["needs"]["network"] = serde_json::json!([{"host": "a.example.com", "why": "x", "limit": 1u64 << 40}]))),
            ("home itself", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "~/", "why": "x"}]))),
            ("dot folder", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "~/.ssh", "why": "x"}]))),
            ("traversal", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "~/a/../b", "why": "x"}]))),
            ("absolute", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "/etc", "why": "x"}]))),
            ("private", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "~/Private/x", "why": "x"}]))),
            ("space in path", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "~/a b", "why": "x"}]))),
            ("rw on bin", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "~/bin", "mode": "rw", "why": "x"}]))),
            ("bad mode", Box::new(|v| v["needs"]["files"] = serde_json::json!([{"path": "~/a", "mode": "x", "why": "x"}]))),
            ("unknown state", Box::new(|v| v["needs"]["state"] = serde_json::json!({"names": ["security"], "why": "x"}))),
            ("unknown control", Box::new(|v| v["needs"]["control"] = serde_json::json!({"names": ["power-off"], "why": "x"}))),
        ];
        for (what, edit) in cases {
            assert!(Manifest::parse(&with(edit)).is_err(), "{what}");
        }
        assert!(Manifest::parse("[]").is_err());
        assert!(Manifest::parse(&" ".repeat(MAX_MANIFEST + 1)).is_err());
    }

    #[test]
    fn surfaces_carry_only_what_the_shell_reads() {
        let m = Manifest::parse(GOOD).unwrap();
        assert_eq!(m.places["bar"]["position"], "right");
        assert!(m.places["pill"].as_object().unwrap().is_empty());
        let edited = |surface: &str, options: Value| {
            Manifest::parse(&with(|v| {
                let mut places = Map::new();
                places.insert(surface.to_string(), options.clone());
                v["contributes"] = Value::Object(places);
            }))
        };
        assert_eq!(edited("dock", serde_json::json!({"app": "org.gnome.Nautilus.desktop"})).unwrap().places["dock"]["app"], "org.gnome.Nautilus.desktop");
        assert_eq!(edited("search", serde_json::json!({"prefix": "ci:"})).unwrap().places["search"]["prefix"], "ci:");
        assert_eq!(edited("desktop", serde_json::json!({})).unwrap().places["desktop"]["corner"], "top-right");
        for (surface, options) in [
            ("dock", serde_json::json!({})),
            ("dock", serde_json::json!({"app": "../x.desktop"})),
            ("dock", serde_json::json!({"app": "x"})),
            ("dock", serde_json::json!({"app": "amethystora-security.desktop"})),
            ("dock", serde_json::json!({"app": "org.amethystora.Vault.desktop"})),
            ("dock", serde_json::json!({"app": "org.gnome.Settings.desktop"})),
            ("search", serde_json::json!({"prefix": "A B"})),
            ("bar", serde_json::json!({"position": "under"})),
            ("pill", serde_json::json!({"plate": "link"})),
            ("tile", serde_json::json!({"x": 1})),
            ("desktop", serde_json::json!({"corner": "middle"})),
        ] {
            assert!(edited(surface, options.clone()).is_err(), "{surface} {options}");
        }
    }

    #[test]
    fn the_files_menu_names_an_action_of_its_own_and_needs_pick() {
        let files = |needs_pick: bool, action: &str| {
            Manifest::parse(&with(|v| {
                v["contributes"]["files-menu"] = serde_json::json!({"action": action});
                if needs_pick {
                    v["needs"]["pick"] = serde_json::json!({"why": "Reads the file you give it"});
                }
            }))
        };
        assert_eq!(files(true, "open-page").unwrap().places["files-menu"]["action"], "open-page");
        assert!(files(false, "open-page").is_err(), "without pick");
        assert!(files(true, "not-declared").is_err(), "an action the manifest does not declare");
        assert!(files(true, "Bad Action").is_err());
    }

    #[test]
    fn a_secret_setting_is_declared_and_named() {
        let m = Manifest::parse(&with(|v| v["settings"] = serde_json::json!([{"name": "token", "label": "Access token", "secret": true}]))).unwrap();
        assert_eq!(m.settings, [Setting { name: "token".into(), label: "Access token".into(), secret: true }]);
        assert!(Manifest::parse(&with(|v| v["settings"] = serde_json::json!([{"name": "Tok en", "label": "x"}]))).is_err());
        assert_eq!(Manifest::parse(&with(|v| v["author"] = "Agent".into())).unwrap().author, "Agent");
    }

    #[test]
    fn host_rules() {
        assert!(valid_host("api.github.com") && valid_host("*.example.org"));
        assert!(!valid_host("localhost") && !valid_host("1.2.3.4") && !valid_host("a.b.") && !valid_host("*.*.a.com"));
        assert!(host_matches("*.example.org", "a.example.org") && !host_matches("*.example.org", "example.org"));
        assert!(!host_matches("*.example.org", "xexample.org") && host_matches("a.com", "a.com") && !host_matches("a.com", "b.a.com"));
    }

    #[test]
    fn perm_arguments() {
        assert!(valid_perm_arg("network=api.github.com") && valid_perm_arg("files=~/Documents/ci:ro"));
        for bad in ["a b", "a\nb", "a;b", "a\"b", "a$b", "", "a`b"] {
            assert!(!valid_perm_arg(bad), "{bad:?}");
        }
        assert!(shipped_refuses("network=a.example.com") && shipped_refuses("window") && !shipped_refuses("notify"));
    }
}
