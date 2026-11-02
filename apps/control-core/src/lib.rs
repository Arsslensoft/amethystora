//! Amethystora's own settings outside security, as /usr/libexec/amethystora-control-list gives them, and the
//! one way to change one: what Amethystora Control and the agent's tools (amethystora-agent-tools) share.
//!
//! What runs is only ever the argument vector the list has for an entry's id and one of the values it gave
//! for it, never through a shell. The few things a caller types are checked here first: a slider's number
//! against the list's range, a new widget's name against the rule amethystora-widgets keeps for names, and
//! a repository to trust against the rule amethystora-trust-image keeps, and a text shortcut against the rules
//! of amethystora-text-shortcuts. A refusal says which of these it was, and the caller puts it in its own words.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

pub mod contrast;

pub const LIST: &str = "/usr/libexec/amethystora-control-list";
/// A widget's name, as amethystora-widgets new takes it
const WIDGET_ID: &str = r"^[a-z0-9][a-z0-9-]*$";
/// A repository as amethystora-trust-image takes it: a registry with a dot or a port, a path in lower
/// case, no tag and no digest. The helper's own expression, letter for letter, and it checks it again.
const REPOSITORY: &str = r"^[a-z0-9]([a-z0-9.-]*[a-z0-9])?(\.[a-z0-9-]+|:[0-9]+)(/[a-z0-9]+(([._]|__|-+)[a-z0-9]+)*)+$";

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("a valid expression"))
}

// --- The list ---------------------------------------------------------------------------------------------

/// A command of the list, and whether it opens in a terminal. `command` is None unless it is a list of
/// words, which is all that is ever run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Command {
    pub terminal: bool,
    pub command: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Choice {
    pub value: String,
    pub title: String,
    pub text: Option<String>,
    pub terminal: bool,
    pub command: Option<Vec<String>>,
    /// A theme's palette, which its swatch is drawn in
    pub colors: BTreeMap<String, String>,
    /// A font's family, which its name is drawn in
    pub font: Option<String>,
    /// A colour the choice is (an accent), which its dot is drawn in
    pub color: Option<String>,
    /// Whether a theme is light, for its preview
    pub light: bool,
    /// A theme's opening picture, which its preview stands on
    pub picture: Option<String>,
    /// The picture a wallpaper's tile shows, when it is not the wallpaper itself (Daylight's picture of the hour)
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Widget {
    pub id: String,
    pub name: Option<String>,
    pub on: bool,
    pub broken: bool,
}

/// A widget of the gallery (R6-29): one of the examples that ship, its words, and where it may be added
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GalleryWidget {
    pub example: String,
    pub name: String,
    pub text: Option<String>,
    /// "bar", "desktop" or both
    pub places: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Webapp {
    pub name: String,
    pub address: String,
}

/// A palette the theme maker can start from (a theme, or the wallpaper's colours over the current theme's), as the
/// list gives it: only a theme's palette keys, each a colour
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Start {
    pub value: String,
    pub title: String,
    pub light: bool,
    pub colors: BTreeMap<String, String>,
}

/// A theme's palette keys, which a theme's colors.toml holds and the theme maker sends, in the order it writes them
pub const THEME_KEYS: [&str; 22] = [
    "background", "foreground", "accent", "cursor", "selection_foreground", "selection_background", "color0", "color1",
    "color2", "color3", "color4", "color5", "color6", "color7", "color8", "color9", "color10", "color11", "color12",
    "color13", "color14", "color15",
];
/// A theme's name as amethystora-theme-file takes it
const THEME_NAME: &str = r"^[A-Za-z0-9][A-Za-z0-9 _-]{0,39}$";

/// A text shortcut as the list names it: its abbreviation and how many lines its text has, never the text
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shortcut {
    pub abbr: String,
    pub lines: u32,
}

/// Another shortcut on the same keys, as the keybindings helper names it
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clash {
    pub schema: String,
    pub key: String,
    pub owner: String,
    pub title: String,
}

/// A keyboard shortcut as /usr/libexec/amethystora-keybindings lists it: its schema and key (or custom and an id), whose
/// it is (gnome, desktop, tiling or custom), its words, the combinations it holds and ships with, and its clashes
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Keybinding {
    pub schema: String,
    pub key: String,
    pub owner: String,
    pub title: String,
    pub value: Vec<String>,
    pub default: Vec<String>,
    pub changed: bool,
    pub active: bool,
    pub clash: Vec<Clash>,
}

/// An item of the top bar as the list names it (the bar entry): its id, its name, where it is and whether it can be hidden
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BarItem {
    pub id: String,
    pub name: String,
    pub zone: String,
    pub fixed: bool,
}

/// Where a top bar item can go: the bar's three boxes, or hidden
pub const BAR_ZONES: [&str; 4] = ["left", "center", "right", "hidden"];

/// One of the actions the list gives a pool of disks
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PoolAction {
    pub id: String,
    pub title: String,
    pub terminal: bool,
    pub command: Option<Vec<String>>,
}

/// A pool of disks, or a row of a `rows` entry (what starts at login, the apps changed in the grid): its words, its
/// state, the word on its tag (a row's only) and the actions the list gave it
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Disk {
    pub id: String,
    pub title: String,
    pub text: Option<String>,
    pub state: String,
    pub tag: Option<String>,
    pub actions: Vec<PoolAction>,
}

/// An entry of the list, as amethystora-control-list's item() makes it, with what its kind adds
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Item {
    pub page: String,
    pub id: String,
    pub kind: String,
    pub group: Option<String>,
    pub title: String,
    pub text: Option<String>,
    pub state: Option<String>,
    pub detail: Option<String>,
    pub available: bool,
    pub reason: Option<String>,
    pub manual: Option<String>,
    pub usage: Option<String>,
    pub choices: Vec<Choice>,
    pub terminal: bool,
    pub command: Option<Vec<String>>,
    pub button: Option<String>,
    pub danger: bool,
    // A slider's
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub unit: Option<String>,
    // The widgets', the web apps' and the setup file's
    pub widgets: Vec<Widget>,
    pub bar: bool,
    pub examples: Vec<String>,
    pub gallery: Vec<GalleryWidget>,
    pub agentic: bool,
    pub commands: HashMap<String, Command>,
    pub webapps: Vec<Webapp>,
    pub file: Option<String>,
    pub saved: Option<f64>,
    // The pools of disks
    pub disks: Vec<Disk>,
    // A rows entry's rows
    pub rows: Vec<Disk>,
    // The text shortcuts
    pub shortcuts: Vec<Shortcut>,
    // The theme maker's palettes to start from
    pub starts: Vec<Start>,
    // A graph entry's points, each [how far along, value], in order
    pub points: Vec<(f64, f64)>,
    // Every keyboard shortcut, and whether the tiling is on (its shortcuts work only then)
    pub keybindings: Vec<Keybinding>,
    pub tiling: bool,
    // The top bar's items (the bar entry)
    pub bar_items: Vec<BarItem>,
}

/// The list as the script gave it: {version, sealed, elsewhere, items}, of which the page uses the last two
#[derive(Debug, Clone, Default, PartialEq)]
pub struct List {
    /// Whether the Updates window's automatic updates are on, "on" or "off"
    pub automatic_updates: Option<String>,
    /// Whether the Backups window is installed
    pub backups: bool,
    pub items: Vec<Item>,
}

// Read leniently, as the page read the parsed JSON: a field that is missing or of another type is as if
// it were not there, and only a command that is a list of words is ever run

/// Text, or None for nothing or an empty string, which the page treated alike
pub fn text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn string(value: Option<&Value>) -> String {
    text(value).unwrap_or_default()
}

/// JavaScript's truthiness, for the flags the page tested as `if (item.agentic)`
fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_) | Value::Object(_)) => true,
        _ => false,
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.as_f64(),
        _ => None,
    }
}

fn words(value: Option<&Value>) -> Option<Vec<String>> {
    let list = value?.as_array()?;
    list.iter().map(|word| word.as_str().map(str::to_string)).collect()
}

fn array(value: Option<&Value>) -> &[Value] {
    value.and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn command(value: &Value) -> Command {
    Command { terminal: truthy(value.get("terminal")), command: words(value.get("command")) }
}

fn choice(value: &Value) -> Choice {
    let colors = value
        .get("colors")
        .and_then(Value::as_object)
        .map(|colors| colors.iter().filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string()))).collect())
        .unwrap_or_default();
    Choice {
        value: string(value.get("value")),
        title: string(value.get("title")),
        text: text(value.get("text")),
        terminal: truthy(value.get("terminal")),
        command: words(value.get("command")),
        colors,
        font: text(value.get("font")),
        color: text(value.get("color")),
        light: truthy(value.get("light")),
        picture: text(value.get("picture")),
        thumbnail: text(value.get("thumbnail")),
    }
}

fn item(value: &Value) -> Item {
    let field = |name: &str| value.get(name);
    Item {
        page: string(field("page")),
        id: string(field("id")),
        kind: string(field("kind")),
        group: text(field("group")),
        title: string(field("title")),
        text: text(field("text")),
        state: text(field("state")),
        detail: text(field("detail")),
        available: truthy(field("available")),
        reason: text(field("reason")),
        manual: text(field("manual")),
        usage: text(field("usage")),
        choices: array(field("choices")).iter().map(choice).collect(),
        terminal: truthy(field("terminal")),
        command: words(field("command")),
        button: text(field("button")),
        danger: truthy(field("danger")),
        min: number(field("min")),
        max: number(field("max")),
        step: number(field("step")),
        unit: text(field("unit")),
        widgets: array(field("widgets"))
            .iter()
            .map(|widget| Widget {
                id: string(widget.get("id")),
                name: text(widget.get("name")),
                on: truthy(widget.get("on")),
                broken: truthy(widget.get("broken")),
            })
            .collect(),
        bar: truthy(field("bar")),
        examples: array(field("examples")).iter().filter_map(|example| example.as_str().map(str::to_string)).collect(),
        gallery: array(field("gallery"))
            .iter()
            .map(|entry| GalleryWidget {
                example: string(entry.get("example")),
                name: string(entry.get("name")),
                text: text(entry.get("text")),
                places: array(entry.get("places")).iter().filter_map(|place| place.as_str().map(str::to_string)).collect(),
            })
            .filter(|entry| !entry.example.is_empty() && !entry.name.is_empty())
            .collect(),
        agentic: truthy(field("agentic")),
        commands: field("commands")
            .and_then(Value::as_object)
            .map(|commands| commands.iter().map(|(name, value)| (name.clone(), command(value))).collect())
            .unwrap_or_default(),
        webapps: array(field("webapps"))
            .iter()
            .map(|webapp| Webapp { name: string(webapp.get("name")), address: string(webapp.get("address")) })
            .collect(),
        file: text(field("file")),
        saved: number(field("saved")),
        disks: array(field("disks")).iter().map(disk).collect(),
        rows: array(field("rows")).iter().map(disk).filter(|row| !row.id.is_empty()).collect(),
        points: array(field("points"))
            .iter()
            .filter_map(|point| Some((number(point.get(0))?, number(point.get(1))?)))
            .collect(),
        shortcuts: array(field("shortcuts"))
            .iter()
            .map(|shortcut| Shortcut {
                abbr: string(shortcut.get("abbr")),
                lines: number(shortcut.get("lines")).filter(|n| *n >= 1.0 && *n <= 2000.0).map_or(1, |n| n as u32),
            })
            .filter(|shortcut| !shortcut.abbr.is_empty())
            .collect(),
        starts: array(field("starts"))
            .iter()
            .map(|start| Start {
                value: string(start.get("value")),
                title: string(start.get("title")),
                light: truthy(start.get("light")),
                colors: start
                    .get("colors")
                    .and_then(Value::as_object)
                    .map(|colors| {
                        colors
                            .iter()
                            .filter(|(key, _)| THEME_KEYS.contains(&key.as_str()))
                            .filter_map(|(key, value)| Some((key.clone(), value.as_str().filter(|v| is_colour(v))?.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .filter(|start| !start.value.is_empty() && start.colors.contains_key("background") && start.colors.contains_key("foreground"))
            .collect(),
        keybindings: array(field("keybindings"))
            .iter()
            .map(|binding| Keybinding {
                schema: string(binding.get("schema")),
                key: string(binding.get("key")),
                owner: string(binding.get("owner")),
                title: string(binding.get("title")),
                value: words(binding.get("value")).unwrap_or_default(),
                default: words(binding.get("default")).unwrap_or_default(),
                changed: truthy(binding.get("changed")),
                active: truthy(binding.get("active")),
                clash: array(binding.get("clash"))
                    .iter()
                    .map(|clash| Clash {
                        schema: string(clash.get("schema")),
                        key: string(clash.get("key")),
                        owner: string(clash.get("owner")),
                        title: string(clash.get("title")),
                    })
                    .collect(),
            })
            .filter(|binding| !binding.schema.is_empty() && !binding.key.is_empty())
            .collect(),
        tiling: truthy(field("tiling")),
        bar_items: array(field("bar_items"))
            .iter()
            .map(|entry| BarItem {
                id: string(entry.get("id")),
                name: string(entry.get("name")),
                zone: string(entry.get("zone")),
                fixed: truthy(entry.get("fixed")),
            })
            .filter(|entry| !entry.id.is_empty() && BAR_ZONES.contains(&entry.zone.as_str()))
            .collect(),
    }
}

fn disk(disk: &Value) -> Disk {
    Disk {
        id: string(disk.get("id")),
        title: string(disk.get("title")),
        text: text(disk.get("text")),
        state: string(disk.get("state")),
        tag: text(disk.get("tag")),
        actions: array(disk.get("actions"))
            .iter()
            .map(|action| PoolAction {
                id: string(action.get("id")),
                title: string(action.get("title")),
                terminal: truthy(action.get("terminal")),
                command: words(action.get("command")),
            })
            .collect(),
    }
}

/// The list from what the script printed, or None when that was not JSON
pub fn parse_list(stdout: &str) -> Option<List> {
    let value: Value = serde_json::from_str(stdout).ok()?;
    let elsewhere = value.get("elsewhere");
    Some(List {
        automatic_updates: text(elsewhere.and_then(|e| e.get("automatic_updates"))),
        backups: truthy(elsewhere.and_then(|e| e.get("backups"))),
        items: array(value.get("items")).iter().map(item).collect(),
    })
}

impl List {
    pub fn find(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn first_of_kind(&self, kind: &str) -> Option<&Item> {
        self.items.iter().find(|item| item.kind == kind)
    }
}

// --- Running what the list says ---------------------------------------------------------------------------

/// Why something is not run. Refused is anything the list does not offer now; the rest are what was typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Not offered by the list, or not available here now
    Refused,
    /// A new widget's name that amethystora-widgets would not take, or one taken already
    WidgetName,
    /// A repository amethystora-trust-image would not take
    Repository,
    /// One of Amethystora's own repositories, which keep Amethystora's key
    OwnRepository,
    /// No key, or one that is not there
    NoKey,
    /// A text shortcut's abbreviation amethystora-text-shortcuts would not take
    ShortcutAbbr,
    /// A text shortcut's text that is empty or too long, or that has a character that cannot be typed
    ShortcutLength,
    ShortcutCharacter,
    /// A theme's name amethystora-theme-file would not take
    ThemeName,
    /// A palette with a key missing or a value that is not a colour
    ThemeColours,
    /// Keys that are not a shortcut: no Super, Ctrl or Alt with a key that would then not be typed, or not keys at all
    Accelerator,
}

/// A command of the list with values checked here in the place of its placeholders, {value} and the like,
/// each of which stands for a whole argument
fn fill(command: &[String], values: &[(&str, &str)]) -> Vec<String> {
    command
        .iter()
        .map(|arg| values.iter().find(|(key, _)| key == arg).map(|(_, value)| value.to_string()).unwrap_or_else(|| arg.clone()))
        .collect()
}

/// What to run, once everything about it has been checked: an argument vector, in a terminal or here
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub argv: Vec<String>,
    pub terminal: bool,
    /// What it reads on stdin: a text shortcut's text, which never goes on a command line other accounts can read
    pub stdin: Option<String>,
}

/// Only a command that is a list of words, at least one, is ever run
fn runnable(command: Option<&Vec<String>>, terminal: bool) -> Result<Run, Refusal> {
    match command {
        Some(argv) if !argv.is_empty() => Ok(Run { argv: argv.clone(), terminal, stdin: None }),
        _ => Err(Refusal::Refused),
    }
}

fn filled(command: Option<&Vec<String>>, terminal: bool, values: &[(&str, &str)]) -> Result<Run, Refusal> {
    runnable(command, terminal).map(|run| Run { argv: fill(&run.argv, values), terminal, stdin: None })
}

/// A slider's number as the page sends it: one to nine digits, inside the list's range
pub fn range_value(item: &Item, value: &str) -> Option<String> {
    if value.is_empty() || value.len() > 9 || !value.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number: u64 = value.parse().ok()?;
    let n = number as f64;
    // A bound the list does not give holds nothing back, as a comparison with undefined did
    if item.min.is_some_and(|min| n < min) || item.max.is_some_and(|max| n > max) {
        return None;
    }
    Some(number.to_string())
}

/// A setting set to one of its choices, or a slider to a number in its range
pub fn change(list: Option<&List>, id: &str, value: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.find(id)).filter(|item| item.available).ok_or(Refusal::Refused)?;
    if item.kind == "range" {
        let number = range_value(item, value).ok_or(Refusal::Refused)?;
        return filled(item.command.as_ref(), item.terminal, &[("{value}", &number)]);
    }
    let choice = item.choices.iter().find(|choice| choice.value == value).ok_or(Refusal::Refused)?;
    runnable(choice.command.as_ref(), choice.terminal)
}

pub fn act(list: Option<&List>, id: &str) -> Result<Run, Refusal> {
    match list.and_then(|list| list.find(id)) {
        Some(item) if item.available && item.kind == "action" => runnable(item.command.as_ref(), item.terminal),
        _ => Err(Refusal::Refused),
    }
}

/// The rule amethystora-widgets has for a widget's name, as the page checks it before it sends one
pub fn valid_widget_name(item: &Item, name: &str) -> bool {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, WIDGET_ID).is_match(name) && name.len() <= 40 && !item.widgets.iter().any(|widget| widget.id == name)
}

/// A widget the list named: on, off, fix or remove; or restart and make, which name none
pub fn widget(list: Option<&List>, action: &str, id: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.find("widgets")).ok_or(Refusal::Refused)?;
    let command = item.commands.get(action);
    let Some(command) = command.filter(|_| item.available && action != "new" && action != "add") else {
        return Err(Refusal::Refused);
    };
    if (action == "make" || action == "fix") && !item.agentic {
        return Err(Refusal::Refused);
    }
    let names_one = command.command.as_ref().is_some_and(|words| words.iter().any(|word| word == "{id}"));
    if names_one && !item.widgets.iter().any(|widget| widget.id == id) {
        return Err(Refusal::Refused);
    }
    filled(command.command.as_ref(), command.terminal, &[("{id}", id)])
}

/// A new widget from one of the examples, under a name of its own
pub fn new_widget(list: Option<&List>, id: &str, example: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.find("widgets")).ok_or(Refusal::Refused)?;
    if !item.available || !item.examples.iter().any(|known| known == example) {
        return Err(Refusal::Refused);
    }
    if !valid_widget_name(item, id) {
        return Err(Refusal::WidgetName);
    }
    let command = item.commands.get("new").ok_or(Refusal::Refused)?;
    filled(command.command.as_ref(), command.terminal, &[("{id}", id), ("{example}", example)])
}

/// A widget of the gallery added to the bar or the desktop in one click (R6-29): an example the gallery lists, at a
/// place it lists for it; amethystora-widgets picks its name
pub fn gallery_widget(list: Option<&List>, example: &str, place: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.find("widgets")).ok_or(Refusal::Refused)?;
    let offered = item.gallery.iter().any(|entry| entry.example == example && entry.places.iter().any(|known| known == place));
    if !item.available || !offered || !matches!(place, "bar" | "desktop") {
        return Err(Refusal::Refused);
    }
    let command = item.commands.get("add").ok_or(Refusal::Refused)?;
    filled(command.command.as_ref(), command.terminal, &[("{example}", example), ("{place}", place)])
}

/// A web app the list named, removed; or a new one, added in a terminal
pub fn webapp(list: Option<&List>, action: &str, name: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.find("webapps")).ok_or(Refusal::Refused)?;
    let Some(command) = item.commands.get(action).filter(|_| item.available) else {
        return Err(Refusal::Refused);
    };
    let names_one = command.command.as_ref().is_some_and(|words| words.iter().any(|word| word == "{name}"));
    if names_one && !item.webapps.iter().any(|webapp| webapp.name == name) {
        return Err(Refusal::Refused);
    }
    filled(command.command.as_ref(), command.terminal, &[("{name}", name)])
}

/// A top bar item the list named, put in one of the four places at a position in it (0 is first), or the bar put back as
/// the image has it ("reset", which names none). A fixed item is never hidden, and the position is at most the number of
/// items already there.
pub fn bar_place(list: Option<&List>, id: &str, zone: &str, position: usize) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("bar")).filter(|item| item.available).ok_or(Refusal::Refused)?;
    let command = item.commands.get("place").ok_or(Refusal::Refused)?;
    let known = item.bar_items.iter().find(|entry| entry.id == id).ok_or(Refusal::Refused)?;
    if !BAR_ZONES.contains(&zone) || (zone == "hidden" && known.fixed) {
        return Err(Refusal::Refused);
    }
    let there = item.bar_items.iter().filter(|entry| entry.zone == zone && entry.id != id).count();
    if position > there {
        return Err(Refusal::Refused);
    }
    // A list that would put anything else in the arguments is not followed
    let placeholders = ["{item}", "{zone}", "{position}"];
    if command.command.as_ref().is_some_and(|words| words.iter().any(|word| word.contains('{') && !placeholders.contains(&word.as_str()))) {
        return Err(Refusal::Refused);
    }
    let position = position.to_string();
    let run = filled(command.command.as_ref(), command.terminal, &[("{item}", id), ("{zone}", zone), ("{position}", &position)])?;
    if run.terminal {
        return Err(Refusal::Refused);
    }
    Ok(run)
}

pub fn bar_reset(list: Option<&List>) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("bar")).filter(|item| item.available).ok_or(Refusal::Refused)?;
    let command = item.commands.get("reset").ok_or(Refusal::Refused)?;
    if command.command.as_ref().is_some_and(|words| words.iter().any(|word| word.contains('{'))) {
        return Err(Refusal::Refused);
    }
    runnable(command.command.as_ref(), command.terminal)
}

/// One of the actions the list gave a pool of disks
pub fn pool(list: Option<&List>, action: &str, id: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("disks")).ok_or(Refusal::Refused)?;
    let found = item.disks.iter().find(|disk| disk.id == id).and_then(|disk| disk.actions.iter().find(|candidate| candidate.id == action));
    match found {
        Some(found) if item.available => runnable(found.command.as_ref(), found.terminal),
        _ => Err(Refusal::Refused),
    }
}

/// One of the actions the list gave a row of a `rows` entry (CARD is the entry's id)
pub fn row(list: Option<&List>, card: &str, action: &str, id: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.find(card)).filter(|item| item.kind == "rows").ok_or(Refusal::Refused)?;
    let found = item.rows.iter().find(|row| row.id == id).and_then(|row| row.actions.iter().find(|candidate| candidate.id == action));
    match found {
        Some(found) if item.available => runnable(found.command.as_ref(), found.terminal),
        _ => Err(Refusal::Refused),
    }
}

// --- Text shortcuts ---------------------------------------------------------------------------------------
//
// The rules of /usr/libexec/amethystora-text-shortcuts, which checks them again and is the one that writes the file:
// an abbreviation is 2 to 32 characters, none a white space or a control or format character but the zero width
// joiner; a text 1 to 2000 characters, none a control or format character but a line break, a tab, the zero width
// joiner and non-joiner and the marks that set a text's direction. The regex crate here has no general categories,
// so white space (Zs, Zl, Zp) and the format characters (Cf) are Unicode 16's, as lists.

const SPACES: &[(u32, u32)] = &[
    (0x09, 0x0d), (0x20, 0x20), (0xa0, 0xa0), (0x1680, 0x1680), (0x2000, 0x200a), (0x2028, 0x2029), (0x202f, 0x202f),
    (0x205f, 0x205f), (0x3000, 0x3000),
];
const FORMAT: &[(u32, u32)] = &[
    (0xad, 0xad), (0x600, 0x605), (0x61c, 0x61c), (0x6dd, 0x6dd), (0x70f, 0x70f), (0x890, 0x891), (0x8e2, 0x8e2),
    (0x180e, 0x180e), (0x200b, 0x200f), (0x202a, 0x202e), (0x2060, 0x2064), (0x2066, 0x206f), (0xfeff, 0xfeff),
    (0xfff9, 0xfffb), (0x110bd, 0x110bd), (0x110cd, 0x110cd), (0x13430, 0x1343f), (0x1bca0, 0x1bca3),
    (0x1d173, 0x1d17a), (0xe0001, 0xe0001), (0xe0020, 0xe007f),
];
const ABBR_ALLOWED: &[char] = &['\u{200d}'];
const TEXT_ALLOWED: &[char] = &['\n', '\t', '\u{200c}', '\u{200d}', '\u{200e}', '\u{200f}', '\u{61c}'];

fn within(c: char, ranges: &[(u32, u32)]) -> bool {
    ranges.iter().any(|&(low, high)| (low..=high).contains(&(c as u32)))
}

fn hidden(c: char) -> bool {
    c.is_control() || within(c, FORMAT)
}

/// An abbreviation amethystora-text-shortcuts takes
pub fn valid_shortcut_abbr(abbr: &str) -> bool {
    let count = abbr.chars().count();
    (2..=32).contains(&count) && abbr.chars().all(|c| !within(c, SPACES) && (!hidden(c) || ABBR_ALLOWED.contains(&c)))
}

/// Why amethystora-text-shortcuts would not take a text, or None when it would
pub fn shortcut_text_problem(text: &str) -> Option<Refusal> {
    let count = text.chars().count();
    if !(1..=2000).contains(&count) {
        return Some(Refusal::ShortcutLength);
    }
    text.chars().any(|c| hidden(c) && !TEXT_ALLOWED.contains(&c)).then_some(Refusal::ShortcutCharacter)
}

/// A text shortcut added (or its text changed), removed, or its text read for editing. The abbreviation of remove
/// and show is one the list named; add's, and its text, are checked against the helper's rules. The abbreviation is
/// one argument; the text goes on stdin, never on the command line, where every account could read it in /proc
pub fn shortcut(list: Option<&List>, action: &str, abbr: &str, text: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("shortcuts")).filter(|item| item.available).ok_or(Refusal::Refused)?;
    let command = item.commands.get(action).ok_or(Refusal::Refused)?;
    // A list that would put anything but the abbreviation in the arguments is not followed
    if command.command.as_ref().is_some_and(|words| words.iter().any(|word| word.contains('{') && word != "{abbr}")) {
        return Err(Refusal::Refused);
    }
    match action {
        "add" => {
            if !valid_shortcut_abbr(abbr) {
                return Err(Refusal::ShortcutAbbr);
            }
            if let Some(problem) = shortcut_text_problem(text) {
                return Err(problem);
            }
            let run = filled(command.command.as_ref(), command.terminal, &[("{abbr}", abbr)])?;
            if run.terminal {
                return Err(Refusal::Refused);
            }
            Ok(Run { stdin: Some(text.to_string()), ..run })
        }
        "remove" | "show" if item.shortcuts.iter().any(|shortcut| shortcut.abbr == abbr) => {
            filled(command.command.as_ref(), command.terminal, &[("{abbr}", abbr)])
        }
        _ => Err(Refusal::Refused),
    }
}

// --- A theme of one's own ---------------------------------------------------------------------------------

/// #rrggbb, in lower case, as the helper takes it
pub fn is_colour(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// A theme's name the helper takes: 1 to 40 letters, digits, spaces, dashes and underscores, a letter or digit first
pub fn valid_theme_name(name: &str) -> bool {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, THEME_NAME).is_match(name)
}

/// A palette made in the theme maker saved as the account's theme and applied: the name one argument, the palette on
/// stdin, every one of THEME_KEYS a colour and nothing else, as `key = "#rrggbb"` lines
pub fn save_theme(list: Option<&List>, name: &str, colors: &BTreeMap<String, String>) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("theme-maker")).filter(|item| item.available).ok_or(Refusal::Refused)?;
    let command = item.commands.get("save").ok_or(Refusal::Refused)?;
    if command.command.as_ref().is_some_and(|words| words.iter().any(|word| word.contains('{') && word != "{name}")) {
        return Err(Refusal::Refused);
    }
    if !valid_theme_name(name) {
        return Err(Refusal::ThemeName);
    }
    if colors.len() != THEME_KEYS.len() || !THEME_KEYS.iter().all(|key| colors.get(*key).is_some_and(|value| is_colour(value))) {
        return Err(Refusal::ThemeColours);
    }
    let run = filled(command.command.as_ref(), command.terminal, &[("{name}", name)])?;
    if run.terminal {
        return Err(Refusal::Refused);
    }
    let palette: String = THEME_KEYS.iter().map(|key| format!("{key} = \"{}\"\n", colors[*key])).collect();
    Ok(Run { stdin: Some(palette), ..run })
}

/// A colour file from elsewhere imported as a theme, in a terminal, which shows it and asks first: the file picked in
/// the window's own file dialog, which has to be there
pub fn import_theme(list: Option<&List>, file: Option<&Path>) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("theme-maker")).filter(|item| item.available).ok_or(Refusal::Refused)?;
    let command = item.commands.get("import").ok_or(Refusal::Refused)?;
    let Some(file) = file.filter(|file| file.is_file()).and_then(Path::to_str) else {
        return Err(Refusal::Refused);
    };
    filled(command.command.as_ref(), command.terminal, &[("{file}", file)])
}

// --- Keyboard shortcuts ----------------------------------------------------------------------------------
//
// The rules of /usr/libexec/amethystora-keybindings, which checks them again: a combination is modifiers in angle
// brackets and one key, the modifiers GTK and Mutter know, and Super, Ctrl or Alt among them unless the key is a
// function key, Print, Pause, Scroll_Lock, Break, Sys_Req or an XF86 key; a modifier alone is not one.

const MODIFIER_NAMES: &[(&str, &str)] = &[
    ("shift", "shift"), ("control", "control"), ("ctrl", "control"), ("ctl", "control"), ("primary", "control"),
    ("alt", "alt"), ("mod1", "alt"), ("super", "super"), ("mod4", "super"), ("meta", "meta"), ("hyper", "hyper"),
];
const MODIFIER_KEYS: &[&str] = &[
    "shift_l", "shift_r", "control_l", "control_r", "alt_l", "alt_r", "super_l", "super_r", "meta_l", "meta_r", "hyper_l",
    "hyper_r", "iso_level3_shift",
];

/// (modifiers, key) of a combination such as <Super><Alt>v, or None when it is not one
pub fn parse_accel(accel: &str) -> Option<(Vec<&'static str>, String)> {
    let mut rest = accel;
    let mut mods: Vec<&'static str> = Vec::new();
    while let Some(after) = rest.strip_prefix('<') {
        let end = after.find('>')?;
        let name = &after[..end];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        let known = MODIFIER_NAMES.iter().find(|(n, _)| name.eq_ignore_ascii_case(n))?.1;
        if !mods.contains(&known) {
            mods.push(known);
        }
        rest = &after[end + 1..];
    }
    if rest.is_empty() || rest.len() > 64 || !rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((mods, rest.to_string()))
}

fn alone(key: &str) -> bool {
    let function = key.strip_prefix('F').is_some_and(|n| !n.is_empty() && n.len() <= 2 && n.chars().all(|c| c.is_ascii_digit()) && !n.starts_with('0'));
    function || matches!(key, "Print" | "Pause" | "Scroll_Lock" | "Break" | "Sys_Req") || (key.starts_with("XF86") && key.len() > 4)
}

/// A combination amethystora-keybindings takes for a shortcut
pub fn valid_accel(accel: &str) -> bool {
    let Some((mods, key)) = parse_accel(accel) else { return false };
    if MODIFIER_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
        return false;
    }
    mods.iter().any(|m| matches!(*m, "super" | "control" | "alt")) || alone(&key)
}

/// A shortcut the list named set to a combination, taken off, or put back as the image ships it: the schema and key
/// each one argument, the combination checked first
pub fn keybinding(list: Option<&List>, action: &str, schema: &str, key: &str, accel: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("keybindings")).filter(|item| item.available).ok_or(Refusal::Refused)?;
    let command = item.commands.get(action).ok_or(Refusal::Refused)?;
    // A list that would put anything else in the arguments is not followed
    if command.command.as_ref().is_some_and(|words| words.iter().any(|word| word.contains('{') && !matches!(word.as_str(), "{schema}" | "{key}" | "{accel}"))) {
        return Err(Refusal::Refused);
    }
    if !item.keybindings.iter().any(|binding| binding.schema == schema && binding.key == key) {
        return Err(Refusal::Refused);
    }
    let accel = match action {
        "set" if valid_accel(accel) => accel,
        "set" => return Err(Refusal::Accelerator),
        "clear" | "reset" => "",
        _ => return Err(Refusal::Refused),
    };
    let run = filled(command.command.as_ref(), command.terminal, &[("{schema}", schema), ("{key}", key), ("{accel}", accel)])?;
    if run.argv.iter().any(String::is_empty) {
        return Err(Refusal::Refused);
    }
    Ok(run)
}

/// The same keys, as GNOME compares them: the modifiers as a set (Primary and Ctrl as Control), a letter in either case
pub fn same_keys(a: &str, b: &str) -> bool {
    match (parse_accel(a), parse_accel(b)) {
        (Some((mut ma, ka)), Some((mut mb, kb))) => {
            ma.sort_unstable();
            mb.sort_unstable();
            ma == mb && (ka == kb || (ka.len() == 1 && ka.eq_ignore_ascii_case(&kb)))
        }
        _ => false,
    }
}

// --- An image of one's own --------------------------------------------------------------------------------

/// The rule amethystora-trust-image has for a repository
pub fn valid_repository(repository: &str) -> bool {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, REPOSITORY).is_match(repository)
}

/// Amethystora's own repositories, which keep Amethystora's key, as amethystora-trust-image has them
pub fn own_repository(image_info: &str) -> Option<String> {
    let info: Value = serde_json::from_str(image_info).ok()?;
    let vendor = text(info.get("image-vendor"))?;
    Some(format!("ghcr.io/{vendor}"))
}

/// Trusting a repository with the key picked in a file dialog. `own` is Amethystora's own repository.
pub fn trust_image(list: Option<&List>, repository: &str, own: Option<&str>, key: Option<&Path>) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.find("trust-image")).filter(|item| item.available).ok_or(Refusal::Refused)?;
    if !valid_repository(repository) {
        return Err(Refusal::Repository);
    }
    if own.is_some_and(|own| repository == own || repository.starts_with(&format!("{own}/"))) {
        return Err(Refusal::OwnRepository);
    }
    // A path that is not text cannot be handed on as it is: as if none had been chosen
    let Some(key) = key.filter(|key| key.exists()).and_then(Path::to_str) else {
        return Err(Refusal::NoKey);
    };
    filled(item.command.as_ref(), item.terminal, &[("{repository}", repository), ("{key}", key)])
}

// --- The setup file ---------------------------------------------------------------------------------------

/// The setup file saved here, or applied in a terminal, where `ame setup apply` shows what it adds first
pub fn setup(list: Option<&List>, action: &str) -> Result<Run, Refusal> {
    let item = list.and_then(|list| list.first_of_kind("setup")).ok_or(Refusal::Refused)?;
    match item.commands.get(action) {
        Some(command) if item.available => runnable(command.command.as_ref(), command.terminal),
        _ => Err(Refusal::Refused),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST_JSON: &str = r#"{"version": 1, "sealed": false, "elsewhere": {"automatic_updates": "on", "backups": true}, "items": [
        {"page": "appearance", "id": "transition-length", "kind": "range", "group": null, "title": "Transition length", "text": "How long",
         "state": "1200", "available": true, "min": 200, "max": 5000, "step": 100, "unit": "ms", "terminal": false,
         "command": ["ame", "desktop", "transition", "grow", "{value}"]},
        {"page": "desktop", "id": "greeting", "kind": "switch", "title": "Terminal greeting", "state": "on", "available": true,
         "choices": [{"value": "on", "title": "On", "text": null, "terminal": false, "command": ["ame", "desktop", "greeting", "on"]},
                     {"value": "off", "title": "Off", "text": null, "terminal": true, "command": ["ame", "desktop", "greeting", "off"]}]},
        {"page": "apps", "id": "gaming", "kind": "switch", "title": "Games", "state": "off", "available": false, "reason": "No",
         "choices": [{"value": "on", "title": "On", "terminal": true, "command": ["ame", "apps", "gaming", "on"]}]},
        {"page": "apps", "id": "cli", "kind": "action", "title": "Tools", "available": true, "terminal": true, "command": ["ame", "apps", "cli"], "button": "Install…"},
        {"page": "apps", "id": "bad", "kind": "action", "title": "Bad", "available": true, "terminal": false, "command": ["ame", 3]},
        {"page": "desktop", "id": "widgets", "kind": "widgets", "title": "Top bar widgets", "available": true, "bar": false, "agentic": false,
         "widgets": [{"id": "pomodoro", "name": "Pomodoro", "on": true, "broken": false}, {"id": "half", "name": null, "on": false, "broken": true}],
         "examples": ["hello", "pomodoro"],
         "gallery": [{"example": "clock", "name": "Clock", "text": "The time", "places": ["desktop", "bar"]},
                     {"example": "pomodoro", "name": "Pomodoro", "places": ["bar"]}, {"example": "", "name": "Nameless"}],
         "commands": {"on": {"terminal": false, "command": ["ame", "desktop", "widgets", "on", "{id}"]},
                      "add": {"terminal": false, "command": ["ame", "desktop", "widgets", "add", "{example}", "{place}"]},
                      "new": {"terminal": false, "command": ["ame", "desktop", "widgets", "new", "{id}", "{example}"]},
                      "restart": {"terminal": false, "command": ["ame", "desktop", "widgets", "restart"]},
                      "fix": {"terminal": true, "command": ["ame", "desktop", "widgets", "fix", "{id}"]}}},
        {"page": "apps", "id": "webapps", "kind": "webapps", "title": "Web apps", "available": true,
         "webapps": [{"name": "Element", "address": "https://app.element.io"}],
         "commands": {"install": {"terminal": true, "command": ["ame", "apps", "webapps", "install"]},
                      "remove": {"terminal": false, "command": ["ame", "apps", "webapps", "remove", "{name}"]}}},
        {"page": "system", "id": "trust-image", "kind": "trust-image", "title": "Own", "available": true, "terminal": true,
         "command": ["ame", "system", "trust-image", "{repository}", "{key}"]},
        {"page": "system", "id": "disks", "kind": "disks", "title": "Pools", "available": true,
         "disks": [{"id": "media", "title": "media, at /var/mnt/media", "text": "Two copies.", "state": "ok",
                    "actions": [{"id": "forget", "title": "Forget…", "terminal": true, "command": ["ame", "system", "raid", "forget", "media"]}]}]},
        {"page": "apps", "id": "startup", "kind": "rows", "title": "Starting with you", "available": true,
         "rows": [{"id": "autostart:chat.desktop", "title": "Chat", "text": "Starts hidden.", "state": "on", "tag": "You",
                   "actions": [{"id": "off", "title": "Off", "terminal": false, "command": ["ame", "apps", "startup", "off", "autostart:chat.desktop"]}]},
                  {"id": "", "title": "No id", "state": "on", "actions": []}]},
        {"page": "setup", "id": "setup-file", "kind": "setup", "title": "Your setup file", "available": true, "file": "/home/a/.config/amethystora/setup.json",
         "saved": 1760000000, "state": "saved",
         "commands": {"save": {"terminal": false, "command": ["ame", "setup", "save"]}, "apply": {"terminal": true, "command": ["ame", "setup", "apply"]}}},
        {"page": "desktop", "id": "text-shortcut-list", "kind": "shortcuts", "title": "Your shortcuts", "available": true,
         "shortcuts": [{"abbr": ";sig", "lines": 2}, {"abbr": ";addr", "lines": 1}, {"abbr": "", "lines": 1}],
         "commands": {"add": {"terminal": false, "command": ["ame", "desktop", "text-shortcut", "add", "{abbr}"]},
                      "remove": {"terminal": false, "command": ["ame", "desktop", "text-shortcut", "remove", "{abbr}"]},
                      "show": {"terminal": false, "command": ["ame", "desktop", "text-shortcut", "show", "{abbr}"]}}}
    ]}"#;

    fn list() -> List {
        parse_list(LIST_JSON).expect("the list parses")
    }

    fn argv(run: Result<Run, Refusal>) -> Vec<String> {
        run.expect("allowed").argv
    }

    #[test]
    fn a_graph_keeps_only_whole_points() {
        let list = parse_list(r#"{"items": [{"page": "system", "id": "start-times", "kind": "graph", "title": "Starts", "available": true,
            "points": [[0, 12.5], [3, 13], ["x", 1], [4], 7]}]}"#).unwrap();
        assert_eq!(list.items[0].points, vec![(0.0, 12.5), (3.0, 13.0)]);
    }

    #[test]
    fn reads_the_list_leniently() {
        let list = list();
        assert_eq!(list.automatic_updates.as_deref(), Some("on"));
        assert!(list.backups);
        assert_eq!(list.items.len(), 12);
        let greeting = list.find("greeting").unwrap();
        assert_eq!(greeting.choices[1].terminal, true);
        assert_eq!(greeting.group, None);
        // A command with something other than words in it is never run
        assert_eq!(list.find("bad").unwrap().command, None);
        let widgets = list.find("widgets").unwrap();
        assert_eq!(widgets.widgets[1].name, None);
        assert!(widgets.widgets[1].broken);
        assert!(!widgets.bar);
        assert_eq!(list.find("setup-file").unwrap().saved, Some(1760000000.0));
        assert!(parse_list("not json").is_none());
        assert!(parse_list("{}").unwrap().items.is_empty());
    }

    #[test]
    fn changes_only_to_what_the_list_offers() {
        let list = list();
        assert_eq!(argv(change(Some(&list), "greeting", "off")), ["ame", "desktop", "greeting", "off"]);
        assert!(change(Some(&list), "greeting", "on").unwrap().terminal == false);
        assert_eq!(change(Some(&list), "greeting", "maybe"), Err(Refusal::Refused));
        // Not available here
        assert!(change(Some(&list), "gaming", "on").is_err());
        assert!(change(Some(&list), "nothing", "on").is_err());
        assert!(change(None, "greeting", "on").is_err());
        assert_eq!(argv(act(Some(&list), "cli")), ["ame", "apps", "cli"]);
        assert!(act(Some(&list), "greeting").is_err());
        assert!(act(Some(&list), "bad").is_err());
    }

    #[test]
    fn sliders_take_numbers_in_range() {
        let list = list();
        assert_eq!(argv(change(Some(&list), "transition-length", "1500")), ["ame", "desktop", "transition", "grow", "1500"]);
        assert_eq!(argv(change(Some(&list), "transition-length", "0300")), ["ame", "desktop", "transition", "grow", "300"]);
        for bad in ["100", "5001", "-300", "1e3", "12.5", "", "1234567890", " 300", "300 "] {
            assert!(change(Some(&list), "transition-length", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn widgets_by_the_lists_names() {
        let list = list();
        assert_eq!(argv(widget(Some(&list), "on", "pomodoro")), ["ame", "desktop", "widgets", "on", "pomodoro"]);
        assert!(widget(Some(&list), "on", "weather").is_err());
        assert_eq!(argv(widget(Some(&list), "restart", "")), ["ame", "desktop", "widgets", "restart"]);
        // The agent's commands only while the agentic features are on, and new only through new_widget
        assert!(widget(Some(&list), "fix", "pomodoro").is_err());
        assert!(widget(Some(&list), "new", "x").is_err());
        assert!(widget(Some(&list), "off", "pomodoro").is_err());
        assert_eq!(argv(new_widget(Some(&list), "my-clock", "hello")), ["ame", "desktop", "widgets", "new", "my-clock", "hello"]);
        assert_eq!(new_widget(Some(&list), "my-clock", "elsewhere"), Err(Refusal::Refused));
        for bad in ["My-clock", "-clock", "pomodoro", "a b", "", "clock;rm", &"a".repeat(41)] {
            assert_eq!(new_widget(Some(&list), bad, "hello"), Err(Refusal::WidgetName), "{bad}");
        }
        assert!(new_widget(Some(&list), &"a".repeat(40), "hello").is_ok());
    }

    #[test]
    fn gallery_widgets_where_the_gallery_offers_them() {
        let list = list();
        let widgets = list.find("widgets").unwrap();
        // An entry without its example or name is not offered
        assert_eq!(widgets.gallery.len(), 2);
        assert_eq!(widgets.gallery[0].places, ["desktop", "bar"]);
        assert_eq!(argv(gallery_widget(Some(&list), "clock", "desktop")), ["ame", "desktop", "widgets", "add", "clock", "desktop"]);
        assert_eq!(argv(gallery_widget(Some(&list), "clock", "bar")), ["ame", "desktop", "widgets", "add", "clock", "bar"]);
        assert_eq!(gallery_widget(Some(&list), "pomodoro", "desktop"), Err(Refusal::Refused));
        assert_eq!(gallery_widget(Some(&list), "hello", "bar"), Err(Refusal::Refused));
        assert_eq!(gallery_widget(Some(&list), "clock", "elsewhere"), Err(Refusal::Refused));
        // Only through gallery_widget, with its two values checked
        assert!(widget(Some(&list), "add", "clock").is_err());
    }

    #[test]
    fn the_bar_by_the_lists_items() {
        let list = parse_list(r#"{"items": [
            {"page": "desktop", "id": "top-bar-layout", "kind": "bar", "title": "Bar", "available": true,
             "bar_items": [{"id": "logo", "name": "Menu", "zone": "left", "fixed": false},
                           {"id": "clock", "name": "Clock", "zone": "center", "fixed": true},
                           {"id": "tray", "name": "Icons", "zone": "right", "fixed": false},
                           {"id": "odd", "name": "Odd", "zone": "middle", "fixed": false}],
             "commands": {"place": {"terminal": false, "command": ["ame", "desktop", "top-bar", "item", "{item}", "{zone}", "{position}"]},
                          "reset": {"terminal": false, "command": ["ame", "desktop", "top-bar", "layout", "reset"]}}}]}"#)
        .unwrap();
        let bar = list.find("top-bar-layout").unwrap();
        assert_eq!(bar.bar_items.len(), 3, "an item in no known place is left out");
        assert_eq!(argv(bar_place(Some(&list), "clock", "left", 0)), ["ame", "desktop", "top-bar", "item", "clock", "left", "0"]);
        assert_eq!(argv(bar_place(Some(&list), "logo", "left", 0)), ["ame", "desktop", "top-bar", "item", "logo", "left", "0"]);
        assert_eq!(argv(bar_place(Some(&list), "tray", "hidden", 0)), ["ame", "desktop", "top-bar", "item", "tray", "hidden", "0"]);
        // A fixed item is never hidden; an item or a place the list did not give, or a position past the end, is refused
        assert!(bar_place(Some(&list), "clock", "hidden", 0).is_err());
        assert!(bar_place(Some(&list), "weather", "left", 0).is_err());
        assert!(bar_place(Some(&list), "logo", "middle", 0).is_err());
        assert!(bar_place(Some(&list), "logo", "left", 1).is_err());
        assert!(bar_place(Some(&list), "tray", "left", 2).is_err());
        assert_eq!(argv(bar_place(Some(&list), "tray", "left", 1)), ["ame", "desktop", "top-bar", "item", "tray", "left", "1"]);
        assert_eq!(argv(bar_reset(Some(&list))), ["ame", "desktop", "top-bar", "layout", "reset"]);
        assert!(bar_place(None, "logo", "left", 0).is_err());
    }

    #[test]
    fn web_apps_pools_and_setup() {
        let list = list();
        assert_eq!(argv(webapp(Some(&list), "remove", "Element")), ["ame", "apps", "webapps", "remove", "Element"]);
        assert!(webapp(Some(&list), "remove", "Other").is_err());
        assert!(webapp(Some(&list), "install", "").unwrap().terminal);
        assert!(webapp(Some(&list), "rename", "Element").is_err());
        assert_eq!(argv(pool(Some(&list), "forget", "media")), ["ame", "system", "raid", "forget", "media"]);
        assert!(pool(Some(&list), "forget", "archive").is_err());
        assert!(pool(Some(&list), "erase", "media").is_err());
        let startup = list.find("startup").expect("the rows entry");
        assert_eq!(startup.rows.len(), 1);
        assert_eq!(startup.rows[0].tag.as_deref(), Some("You"));
        assert_eq!(argv(row(Some(&list), "startup", "off", "autostart:chat.desktop")),
            ["ame", "apps", "startup", "off", "autostart:chat.desktop"]);
        assert!(!row(Some(&list), "startup", "off", "autostart:chat.desktop").unwrap().terminal);
        assert!(row(Some(&list), "startup", "on", "autostart:chat.desktop").is_err());
        assert!(row(Some(&list), "startup", "off", "autostart:other.desktop").is_err());
        assert!(row(Some(&list), "disks", "forget", "media").is_err());
        assert_eq!(argv(setup(Some(&list), "save")), ["ame", "setup", "save"]);
        assert!(setup(Some(&list), "apply").unwrap().terminal);
        assert!(setup(Some(&list), "remove").is_err());
    }

    #[test]
    fn text_shortcuts_by_the_helpers_rules() {
        let list = list();
        let item = list.find("text-shortcut-list").unwrap();
        // An entry without an abbreviation is left out; none of them carries a text
        assert_eq!(item.shortcuts, [Shortcut { abbr: ";sig".into(), lines: 2 }, Shortcut { abbr: ";addr".into(), lines: 1 }]);
        // The text goes on stdin, never on the command line; one that looks like a placeholder or an option too
        for text in ["Date: {date}", "{abbr}", "--yes; rm -rf ~"] {
            let run = shortcut(Some(&list), "add", ";d", text).unwrap();
            assert_eq!(run.argv, ["ame", "desktop", "text-shortcut", "add", ";d"]);
            assert_eq!(run.stdin.as_deref(), Some(text));
            assert!(!run.terminal);
        }
        assert_eq!(shortcut(Some(&list), "remove", ";sig", "").unwrap().stdin, None);
        // A list that puts the text, or anything else, among the arguments is not followed
        let leaky = parse_list(&LIST_JSON.replace(r#""add", "{abbr}"]"#, r#""add", "{abbr}", "{text}"]"#)).unwrap();
        assert_eq!(shortcut(Some(&leaky), "add", ";d", "x"), Err(Refusal::Refused));
        assert_eq!(argv(shortcut(Some(&list), "remove", ";sig", "")), ["ame", "desktop", "text-shortcut", "remove", ";sig"]);
        assert_eq!(argv(shortcut(Some(&list), "show", ";addr", "")), ["ame", "desktop", "text-shortcut", "show", ";addr"]);
        assert_eq!(shortcut(Some(&list), "remove", ";nope", ""), Err(Refusal::Refused));
        assert_eq!(shortcut(Some(&list), "show", "", ""), Err(Refusal::Refused));
        assert_eq!(shortcut(Some(&list), "list", "", ""), Err(Refusal::Refused));
        assert_eq!(shortcut(None, "add", ";x", "x"), Err(Refusal::Refused));
        for bad in [";", "a b", ";x\u{a0}", ";x\u{3000}", ";x\u{1b}", ";x\u{200b}", ";x\u{202e}", ";x\u{200c}", &";".repeat(33)] {
            assert_eq!(shortcut(Some(&list), "add", bad, "x"), Err(Refusal::ShortcutAbbr), "{bad:?}");
        }
        for good in [";sig", "ab", ":shrug", ";👨\u{200d}👩", &"é".repeat(32)] {
            assert!(valid_shortcut_abbr(good), "{good:?}");
        }
        assert_eq!(shortcut(Some(&list), "add", ";x", ""), Err(Refusal::ShortcutLength));
        assert_eq!(shortcut(Some(&list), "add", ";x", &"x".repeat(2001)), Err(Refusal::ShortcutLength));
        assert!(shortcut(Some(&list), "add", ";x", &"😀".repeat(2000)).is_ok());
        for bad in ["\u{1b}[31m", "a\rb", "a\u{0}", "a\u{ad}b", "a\u{2066}b", "a\u{feff}"] {
            assert_eq!(shortcut(Some(&list), "add", ";x", bad), Err(Refusal::ShortcutCharacter), "{bad:?}");
        }
        for good in ["Best wishes,\nSam", "a\tb", "\u{200f}مرحبا", "a\u{200c}b"] {
            assert_eq!(shortcut_text_problem(good), None, "{good:?}");
        }
        // Not a setting with values: what changes a setting (the agent's change) never reaches it
        assert_eq!(change(Some(&list), "text-shortcut-list", "on"), Err(Refusal::Refused));
        assert_eq!(act(Some(&list), "text-shortcut-list"), Err(Refusal::Refused));
    }

    #[test]
    fn repositories_as_the_helper_takes_them() {
        for good in ["ghcr.io/you/your-image", "registry.example.com:5000/a/b", "localhost:5000/x", "quay.io/a_b/c__d/e--f"] {
            assert!(valid_repository(good), "{good}");
        }
        for bad in ["ghcr.io/you/image:stable", "ghcr.io/You/image", "ghcr.io", "you/your-image", "ghcr.io/you/image@sha256:00",
            "ghcr.io/you/-image", "ghcr.io//image", "-ghcr.io/you/image"] {
            assert!(!valid_repository(bad), "{bad}");
        }
        assert_eq!(own_repository(r#"{"image-vendor": "arsslensoft"}"#).as_deref(), Some("ghcr.io/arsslensoft"));
        assert_eq!(own_repository(r#"{"image-vendor": ""}"#), None);
        assert_eq!(own_repository("nope"), None);
    }

    #[test]
    fn trusts_only_with_a_key_and_not_amethystoras_own() {
        let list = list();
        let key = std::env::temp_dir().join("amethystora-control-core-test-cosign.pub");
        std::fs::write(&key, "key").unwrap();
        let own = Some("ghcr.io/arsslensoft");
        let run = trust_image(Some(&list), "ghcr.io/you/image", own, Some(&key)).unwrap();
        assert!(run.terminal);
        assert_eq!(run.argv, ["ame", "system", "trust-image", "ghcr.io/you/image", key.to_str().unwrap()]);
        assert_eq!(trust_image(Some(&list), "ghcr.io/arsslensoft/amethystora", own, Some(&key)), Err(Refusal::OwnRepository));
        assert!(trust_image(Some(&list), "ghcr.io/arsslensoft", own, Some(&key)).is_err());
        assert!(trust_image(Some(&list), "ghcr.io/arsslensoftx/image", own, Some(&key)).is_ok());
        assert_eq!(trust_image(Some(&list), "ghcr.io/you/image:tag", own, Some(&key)), Err(Refusal::Repository));
        assert_eq!(trust_image(Some(&list), "ghcr.io/you/image", own, None), Err(Refusal::NoKey));
        assert!(trust_image(Some(&list), "ghcr.io/you/image", own, Some(Path::new("/nonexistent/cosign.pub"))).is_err());
        let _ = std::fs::remove_file(key);
    }

    #[test]
    fn the_theme_maker_sends_only_a_whole_palette() {
        let list = parse_list(
            r##"{"items": [{"page": "appearance", "id": "theme-maker", "kind": "theme-maker", "title": "Make a theme", "available": true,
            "starts": [{"value": "nord", "title": "Nord", "light": false, "colors": {"background": "#2e3440", "foreground": "#d8dee9",
                "accent": "#81a1c1", "evil": "#000000", "color1": "$(id)"}},
                {"value": "broken", "title": "Broken", "colors": {"accent": "#000000"}}],
            "commands": {"save": {"terminal": false, "command": ["ame", "desktop", "theme", "save", "{name}"]},
                "import": {"terminal": true, "command": ["ame", "desktop", "theme", "import", "{file}"]}}}]}"##,
        )
        .unwrap();
        let item = list.first_of_kind("theme-maker").unwrap();
        // Only a palette's keys, each a colour, and only a start with a ground and a text
        assert_eq!(item.starts.len(), 1);
        assert_eq!(item.starts[0].colors.keys().map(String::as_str).collect::<Vec<_>>(), ["accent", "background", "foreground"]);
        let colors: BTreeMap<String, String> = THEME_KEYS.iter().map(|key| (key.to_string(), "#123abc".to_string())).collect();
        let run = save_theme(Some(&list), "My Theme", &colors).unwrap();
        assert_eq!(run.argv, ["ame", "desktop", "theme", "save", "My Theme"]);
        assert!(!run.terminal);
        let stdin = run.stdin.unwrap();
        assert_eq!(stdin.lines().count(), 22);
        assert!(stdin.starts_with("background = \"#123abc\"\n"));
        for bad in ["", " lead", "../x", "a/b", "x".repeat(41).as_str(), "name;rm"] {
            assert_eq!(save_theme(Some(&list), bad, &colors), Err(Refusal::ThemeName), "{bad}");
        }
        let mut short = colors.clone();
        short.remove("color15");
        assert_eq!(save_theme(Some(&list), "Mine", &short), Err(Refusal::ThemeColours));
        let mut upper = colors.clone();
        upper.insert("accent".into(), "#ABCDEF".into());
        assert_eq!(save_theme(Some(&list), "Mine", &upper), Err(Refusal::ThemeColours));
        let mut extra = colors.clone();
        extra.insert("evil".into(), "#000000".into());
        assert_eq!(save_theme(Some(&list), "Mine", &extra), Err(Refusal::ThemeColours));
        let file = std::env::temp_dir().join("amethystora-control-core-test-kitty.conf");
        std::fs::write(&file, "background #000000\n").unwrap();
        let run = import_theme(Some(&list), Some(&file)).unwrap();
        assert!(run.terminal);
        assert_eq!(run.argv, ["ame", "desktop", "theme", "import", file.to_str().unwrap()]);
        assert!(import_theme(Some(&list), Some(Path::new("/nonexistent/x.conf"))).is_err());
        assert!(import_theme(Some(&list), None).is_err());
        let _ = std::fs::remove_file(file);
    }

    const KEYS_JSON: &str = r#"{"items": [
        {"page": "keyboard", "id": "keyboard-shortcuts", "kind": "keybindings", "title": "Every shortcut", "available": true,
         "tiling": true,
         "keybindings": [
            {"schema": "org.gnome.settings-daemon.plugins.media-keys", "key": "previous", "owner": "gnome", "title": "Previous track",
             "value": ["<Super>Left"], "default": ["AudioPrev"], "changed": true, "active": true,
             "clash": [{"schema": "org.gnome.shell.extensions.amethystora-ui.tiling.keybindings", "key": "switch-left", "owner": "tiling",
                        "title": "Switch to the window on the left"}]},
            {"schema": "custom", "key": "custom27", "owner": "desktop", "title": "AI agent", "value": [], "default": ["<Super><Control><Shift>a"],
             "changed": true, "active": true, "clash": []},
            {"schema": "", "key": "nothing", "owner": "gnome", "title": "No schema", "value": [], "default": []}],
         "commands": {"set": {"terminal": false, "command": ["ame", "desktop", "shortcut", "set", "{schema}", "{key}", "{accel}"]},
                      "clear": {"terminal": false, "command": ["ame", "desktop", "shortcut", "clear", "{schema}", "{key}"]},
                      "reset": {"terminal": false, "command": ["ame", "desktop", "shortcut", "reset", "{schema}", "{key}"]}}}
    ]}"#;

    #[test]
    fn keybindings_by_the_helpers_rules() {
        let list = parse_list(KEYS_JSON).unwrap();
        let item = list.find("keyboard-shortcuts").unwrap();
        assert!(item.tiling);
        // A row without a schema is left out
        assert_eq!(item.keybindings.len(), 2);
        assert_eq!(item.keybindings[0].clash[0].owner, "tiling");
        assert!(item.keybindings[1].value.is_empty());
        const MEDIA: &str = "org.gnome.settings-daemon.plugins.media-keys";
        assert_eq!(argv(keybinding(Some(&list), "set", MEDIA, "previous", "<Super><Alt>v")),
            ["ame", "desktop", "shortcut", "set", MEDIA, "previous", "<Super><Alt>v"]);
        assert_eq!(argv(keybinding(Some(&list), "clear", MEDIA, "previous", "ignored")), ["ame", "desktop", "shortcut", "clear", MEDIA, "previous"]);
        assert_eq!(argv(keybinding(Some(&list), "reset", "custom", "custom27", "")), ["ame", "desktop", "shortcut", "reset", "custom", "custom27"]);
        // Only a shortcut the list named, only its three actions
        assert_eq!(keybinding(Some(&list), "set", MEDIA, "next", "<Super>n"), Err(Refusal::Refused));
        assert_eq!(keybinding(Some(&list), "set", "org.gnome.nope", "previous", "<Super>n"), Err(Refusal::Refused));
        assert_eq!(keybinding(Some(&list), "remove", MEDIA, "previous", ""), Err(Refusal::Refused));
        assert_eq!(keybinding(None, "set", MEDIA, "previous", "<Super>n"), Err(Refusal::Refused));
        for bad in ["a", "<Shift>a", "Return", "<Super>", "<Super>Super_L", "<Super", "<Nope>a", "<Super>a b", "<Super>a;rm", "--yes", "",
            "<Super>é"] {
            assert_eq!(keybinding(Some(&list), "set", MEDIA, "previous", bad), Err(Refusal::Accelerator), "{bad:?}");
        }
        for good in ["F5", "<Shift>F12", "Print", "XF86AudioPlay", "<Primary>q", "<Control><Alt>t", "<Mod4>Left", "<super>v"] {
            assert!(valid_accel(good), "{good}");
        }
        assert!(same_keys("<Super><Alt>v", "<Alt><Super>V"));
        assert!(same_keys("<Primary>q", "<Control>q"));
        assert!(!same_keys("<Super>Left", "<Super><Shift>Left"));
        assert!(!same_keys("<Super>Left", "Left"));
        // Not a setting with values: the agent's change never reaches it
        assert_eq!(change(Some(&list), "keyboard-shortcuts", "on"), Err(Refusal::Refused));
        // A list that would put anything else among the arguments is not followed
        let leaky = parse_list(&KEYS_JSON.replace(r#""{key}", "{accel}"]"#, r#""{key}", "{accel}", "{title}"]"#)).unwrap();
        assert_eq!(keybinding(Some(&leaky), "set", MEDIA, "previous", "<Super>n"), Err(Refusal::Refused));
    }
}
