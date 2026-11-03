//! What a plugin prints (plans/PLAN-plugins.md, "What it prints"): the widget format plus `progress`, `until` and `card`.
//! The host reads it, refuses what is not the format and cleans the rest, so that the shell draws only what passed:
//! text and no markup, theme icon names and no paths, a bounded number of elements, every number clamped.
//!
//!   plain text, or {"text", "icon", "class", "click", "hidden", "private", "progress", "until",
//!                   "menu": [{"label", "icon", "action"} | {"separator": true}], "card": [element...]}
//! an element of a card is {"type": "text", "text"}, {"type": "row", "label", "value"}, {"type": "progress", "label",
//! "value"}, {"type": "button", "label", "action"}, {"type": "switch", "label", "on", "action"}, {"type": "list",
//! "items": [text...]} or {"type": "meter", "label", "value", "min", "max"}. An action is an id from the manifest.

use serde_json::{Map, Value, json};

use crate::manifest::{is_bidi, valid_action};

/// All that is read of what a run printed
pub const MAX_OUTPUT: usize = 64 * 1024;
pub const MAX_CARD: usize = 20;
pub const MAX_MENU: usize = 20;
const CLASSES: &[&str] = &["accent", "good", "warning", "critical"];
const TOP: &[&str] = &["text", "icon", "class", "click", "hidden", "private", "progress", "until", "menu", "card"];

/// Text for the shell: control and bidirectional characters out, tabs and line ends as a space, at most `max` characters
pub fn clean(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if out.chars().count() >= max {
            break;
        }
        if c == '\n' || c == '\t' || c == '\r' {
            out.push(' ');
        } else if !c.is_control() && !is_bidi(c) {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// A theme icon's name, never a path: letters, digits, hyphens and dots
fn icon(value: &Value) -> Result<String, String> {
    match value.as_str() {
        Some(name) if name.len() <= 64 && !name.is_empty() && !name.starts_with('.') && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) => Ok(name.into()),
        _ => Err("\"icon\" has to be the name of an icon in the theme, never a path".into()),
    }
}

fn string(map: &Map<String, Value>, key: &str, max: usize) -> Result<String, String> {
    match map.get(key) {
        None => Ok(String::new()),
        Some(Value::String(s)) => Ok(clean(s, max)),
        Some(_) => Err(format!("\"{key}\" has to be text")),
    }
}

fn number(value: Option<&Value>, key: &str) -> Result<f64, String> {
    match value.and_then(Value::as_f64) {
        Some(n) if n.is_finite() => Ok(n),
        _ => Err(format!("\"{key}\" has to be a number")),
    }
}

fn action(map: &Map<String, Value>, actions: &[String]) -> Result<Option<String>, String> {
    match map.get("action") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(a)) if valid_action(a) && actions.contains(a) => Ok(Some(a.clone())),
        Some(_) => Err("an \"action\" has to be one of the \"actions\" the manifest names".into()),
    }
}

fn element(value: &Value, actions: &[String]) -> Result<Value, String> {
    let m = value.as_object().ok_or("a card element has to be an object")?;
    let kind = m.get("type").and_then(Value::as_str).unwrap_or("");
    let label = string(m, "label", 80)?;
    Ok(match kind {
        "text" => json!({"type": "text", "text": string(m, "text", 400)?}),
        "row" => json!({"type": "row", "label": label, "value": string(m, "value", 120)?}),
        "progress" => json!({"type": "progress", "label": label, "value": number(m.get("value"), "value")?.clamp(0.0, 1.0)}),
        "button" => json!({"type": "button", "label": label, "action": action(m, actions)?}),
        "switch" => json!({"type": "switch", "label": label, "on": m.get("on").and_then(Value::as_bool).unwrap_or(false), "action": action(m, actions)?}),
        "list" => {
            let items = m.get("items").and_then(Value::as_array).filter(|l| l.len() <= MAX_CARD).ok_or("a list has at most 20 items")?;
            json!({"type": "list", "items": items.iter().map(|i| clean(i.as_str().unwrap_or(""), 120)).collect::<Vec<_>>()})
        }
        "meter" => {
            let (min, max) = (number(m.get("min").or(Some(&json!(0))), "min")?, number(m.get("max").or(Some(&json!(100))), "max")?);
            if min >= max {
                return Err("a meter's \"min\" has to be below its \"max\"".into());
            }
            json!({"type": "meter", "label": label, "value": number(m.get("value"), "value")?.clamp(min, max), "min": min, "max": max})
        }
        other => return Err(format!("{other:?} is not an element a card can hold")),
    })
}

/// The update in what one run printed: all of it when it is one JSON object, else the last line that holds anything
pub fn last_update(stdout: &str) -> &str {
    let whole = stdout.trim();
    if whole.starts_with('{') && serde_json::from_str::<Value>(whole).is_ok() {
        return whole;
    }
    whole.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim()
}

/// What one run printed, as the shell may draw it. `actions` are the manifest's
pub fn parse(stdout: &str, actions: &[String]) -> Result<Value, String> {
    if stdout.len() > MAX_OUTPUT {
        return Err("it printed more than 64 KiB".into());
    }
    let line = last_update(stdout);
    if line.is_empty() {
        return Err("it printed nothing; print {\"hidden\": true} to hide the plugin".into());
    }
    if !line.starts_with('{') {
        return Ok(json!({"text": clean(line, 200), "icon": "", "class": "", "click": null, "hidden": false, "private": false, "progress": null, "until": null, "menu": [], "card": []}));
    }
    let data: Value = serde_json::from_str(line).map_err(|e| format!("what it printed is not JSON ({e}); print each update on one line"))?;
    let m = data.as_object().ok_or("what it printed has to be one JSON object")?;
    if let Some(k) = m.keys().find(|k| !TOP.contains(&k.as_str())) {
        return Err(format!("unknown field {k:?} in the output"));
    }
    let class = string(m, "class", 16)?;
    if !class.is_empty() && !CLASSES.contains(&class.as_str()) {
        return Err(format!("\"class\" has to be one of {}", CLASSES.join(", ")));
    }
    let mut menu = Vec::new();
    for item in m.get("menu").map(|v| v.as_array().ok_or("\"menu\" has to be a list")).transpose()?.into_iter().flatten() {
        if menu.len() >= MAX_MENU {
            return Err("a menu has at most 20 items".into());
        }
        let i = item.as_object().ok_or("a menu item has to be an object")?;
        if i.get("separator").and_then(Value::as_bool) == Some(true) {
            menu.push(json!({"separator": true}));
        } else {
            let icon = i.get("icon").map(icon).transpose()?.unwrap_or_default();
            menu.push(json!({"label": string(i, "label", 80)?, "icon": icon, "action": action(i, actions)?}));
        }
    }
    let card = match m.get("card") {
        None => Vec::new(),
        Some(Value::Array(list)) if list.len() <= MAX_CARD => list.iter().map(|e| element(e, actions)).collect::<Result<_, _>>()?,
        Some(Value::Array(_)) => return Err(format!("a card has at most {MAX_CARD} elements")),
        Some(_) => return Err("\"card\" has to be a list".into()),
    };
    let progress = match m.get("progress") {
        None | Some(Value::Null) => Value::Null,
        Some(v) => json!(number(Some(v), "progress")?.clamp(0.0, 1.0)),
    };
    let until = match m.get("until") {
        None | Some(Value::Null) => Value::Null,
        Some(v) => json!(v.as_u64().filter(|t| *t < 1 << 40).ok_or("\"until\" has to be a time, in whole seconds since 1970")?),
    };
    let click = match m.get("click") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::String(a)) if actions.contains(a) => json!(a),
        Some(_) => return Err("\"click\" has to be one of the \"actions\" the manifest names".into()),
    };
    for (key, kind) in [("hidden", "true or false"), ("private", "true or false")] {
        if m.get(key).is_some_and(|v| !v.is_boolean()) {
            return Err(format!("\"{key}\" has to be {kind}"));
        }
    }
    Ok(json!({
        "text": string(m, "text", 200)?,
        "icon": m.get("icon").map(icon).transpose()?.unwrap_or_default(),
        "class": class,
        "click": click,
        "hidden": m.get("hidden").and_then(Value::as_bool).unwrap_or(false),
        "private": m.get("private").and_then(Value::as_bool).unwrap_or(false),
        "progress": progress,
        "until": until,
        "menu": menu,
        "card": card,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions() -> Vec<String> {
        vec!["open-page".into(), "toggle".into()]
    }

    #[test]
    fn plain_text_and_the_last_line() {
        assert_eq!(parse("hello\n", &[]).unwrap()["text"], "hello");
        assert_eq!(parse("noise\n{\"text\":\"a\"}\n", &[]).unwrap()["text"], "a");
        assert_eq!(parse("{\n \"text\": \"over lines\"\n}\n", &[]).unwrap()["text"], "over lines");
        assert!(parse("", &[]).is_err() && parse(" \n\n", &[]).is_err());
    }

    #[test]
    fn clamps_and_cleans() {
        let out = parse(r#"{"text":"a\u202eb\u0007c\nd","progress":7,"until":1700000000,"card":[{"type":"meter","label":"x","value":500,"max":10},{"type":"progress","label":"p","value":-1}]}"#, &[]).unwrap();
        assert_eq!(out["text"], "abc d");
        assert_eq!(out["progress"], 1.0);
        assert_eq!(out["card"][0]["value"], 10.0);
        assert_eq!(out["card"][1]["value"], 0.0);
        assert_eq!(out["until"], 1700000000u64);
        assert_eq!(parse(&format!("{{\"text\":\"{}\"}}", "x".repeat(900)), &[]).unwrap()["text"].as_str().unwrap().len(), 200);
        // Markup is text: nothing is interpreted, and the shell draws it as it is
        assert_eq!(parse(r#"{"text":"<b>x</b>"}"#, &[]).unwrap()["text"], "<b>x</b>");
    }

    #[test]
    fn refuses_what_is_not_the_format() {
        let big_card = format!("{{\"card\":[{}]}}", vec![r#"{"type":"text","text":"x"}"#; 10_000].join(","));
        let long = format!("{{\"text\":\"{}\"}}", "x".repeat(MAX_OUTPUT));
        for (what, text) in [
            ("a card of 10,000 rows", big_card.as_str()),
            ("a megabyte line", long.as_str()),
            ("an unknown field", r#"{"text":"a","plate":"system"}"#),
            ("a path as an icon", r#"{"icon":"/etc/passwd"}"#),
            ("a path with dots as an icon", r#"{"icon":"../x"}"#),
            ("a fifo as an icon", r#"{"icon":"/proc/self/fd/0"}"#),
            ("an unknown class", r#"{"class":"system"}"#),
            ("an action that is not the manifest's", r#"{"click":"rm-all"}"#),
            ("a button for an unknown action", r#"{"card":[{"type":"button","label":"x","action":"shell"}]}"#),
            ("an unknown element", r#"{"card":[{"type":"text-field"}]}"#),
            ("a meter backwards", r#"{"card":[{"type":"meter","value":1,"min":5,"max":1}]}"#),
            ("a progress that is not a number", r#"{"progress":"half"}"#),
            ("hidden as text", r#"{"hidden":"yes"}"#),
            ("an until in the far future", r#"{"until":99999999999999}"#),
            ("not JSON", "{oops"),
        ] {
            assert!(parse(text, &actions()).is_err(), "{what}");
        }
        let menu = format!("{{\"menu\":[{}]}}", vec![r#"{"label":"x"}"#; MAX_MENU + 1].join(","));
        assert!(parse(&menu, &[]).is_err());
    }

    #[test]
    fn actions_pass_when_the_manifest_names_them() {
        let out = parse(r#"{"click":"open-page","menu":[{"label":"Open","icon":"folder-symbolic","action":"open-page"},{"separator":true}],"card":[{"type":"button","label":"Go","action":"toggle"},{"type":"list","items":["a","b"]}]}"#, &actions()).unwrap();
        assert_eq!(out["click"], "open-page");
        assert_eq!(out["menu"][0]["icon"], "folder-symbolic");
        assert_eq!(out["card"][1]["items"][1], "b");
    }
}
