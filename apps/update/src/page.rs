//! The page of Amethystora Updates, as update.js drew it: what the machine runs, what an update is doing,
//! and the firmware and the timer. Everything is built as widgets with text in them and never as markup
//! from outside: the log is whatever the updater and the commands it ran printed.

use std::collections::HashMap;
use std::rc::Rc;

use amethystora_ui::gem::{Gem, Motion};
use amethystora_ui::i18n::{self, DateStyle, Piece};
use amethystora_ui::ui::{self, Bar};
use amethystora_ui::{adw, frame, gtk, process, t, t_parts};
use gtk::prelude::*;

use crate::backend::{self, Deployment, Device, Package, Step, Update, Waiting};
use crate::icons::icon;
use crate::Updates;

/// The parts of the page that change while an update runs
#[derive(Default)]
pub struct Live {
    pub kind: String,
    pub title: Option<gtk::Label>,
    pub lead: Option<gtk::Label>,
    pub bar: Option<Bar>,
    pub percent: Option<gtk::Label>,
    pub steps: Option<gtk::Box>,
    pub step_nodes: HashMap<String, StepNodes>,
    pub log: Option<(gtk::Label, gtk::ScrolledWindow)>,
}

pub struct StepNodes {
    icon: gtk::Box,
    spinner: amethystora_ui::icon::Icon,
    label: gtk::Label,
    badge: gtk::Box,
}

/// The tinted square a row's icon sits in, as the page's `.row-icon`
fn row_icon(name: &str, classes: &[&str]) -> gtk::Box {
    let square = ui::hbox(0, classes);
    square.set_valign(gtk::Align::Center);
    // Its own size, whatever the icon in it asks for
    square.set_hexpand(false);
    let mark = icon(name, 20);
    mark.set_hexpand(true);
    square.append(&mark);
    square
}

// --- Words ----------------------------------------------------------------------------------------------

fn time(seconds: f64) -> String {
    i18n::date(seconds, DateStyle::Time)
}

/// When something happened, or will: "today at 18:00", "tomorrow at 00:00", "Sat 26 Sep at 12:00"
fn when(seconds: f64) -> String {
    let days = i18n::days_from_today(seconds);
    let day = if days.abs() <= 1 { i18n::days(days) } else { i18n::date(seconds, DateStyle::WeekdayDayMonthShort) };
    t!("{day} at {time}", day = day, time = time(seconds))
}

fn day(seconds: f64) -> String {
    i18n::date(seconds, DateStyle::DayMonthYear)
}

/// ghcr.io/arsslensoft/amethystora-dx:stable -> ("amethystora-dx", "stable")
fn image_parts(image: &str) -> (String, String) {
    let before_digest = image.split('@').next().unwrap_or("");
    let last = before_digest.rsplit('/').next().unwrap_or("");
    match last.split_once(':') {
        Some((name, tag)) => (name.to_string(), tag.to_string()),
        None => (last.to_string(), String::new()),
    }
}

/// A tag that names one build, such as stable-44.20260922, rather than a stream
fn held_on(image: &str) -> String {
    let (_, tag) = image_parts(image);
    // /^(?:[a-z-]+-)?\d+\.\d{8}/
    let rest = match tag.rfind(|c: char| c == '-') {
        Some(at) if tag[..at].chars().all(|c| c.is_ascii_lowercase() || c == '-') && at > 0 => &tag[at + 1..],
        _ => tag.as_str(),
    };
    let (major, minor) = rest.split_once('.').unwrap_or(("", ""));
    let digits = minor.chars().take_while(char::is_ascii_digit).count();
    if !major.is_empty() && major.chars().all(|c| c.is_ascii_digit()) && digits >= 8 { tag } else { String::new() }
}

// --- The steps ------------------------------------------------------------------------------------------

/// uupd's modules, by the title it gives them, in the order it runs them: what each row says, what it is in
/// a sentence of its own, and the sentence for it while it runs
struct About {
    icon: &'static str,
    title: String,
    text: String,
    what: String,
    doing: String,
}

fn about(name: &str) -> Option<About> {
    Some(match name {
        "System" => About {
            icon: "layers",
            title: t!("The system"),
            text: t!("Amethystora itself. The new version is set up next to the one running and takes over at the next restart."),
            what: t!("the system"),
            doing: t!("Updating the system"),
        },
        "Brew" => About {
            icon: "terminal",
            title: t!("Command-line tools"),
            text: t!("What you installed with Homebrew."),
            what: t!("the command-line tools"),
            doing: t!("Updating the command-line tools"),
        },
        "Flatpak" => About {
            icon: "apps",
            title: t!("Apps"),
            text: t!("Flatpak apps, the whole machine’s and those of everyone signed in."),
            what: t!("the apps"),
            doing: t!("Updating the apps"),
        },
        "Distrobox" => About {
            icon: "box",
            title: t!("Containers"),
            text: t!("What you installed from other distributions with amepkg. Upgraded by themselves every day, and after Update now."),
            what: t!("the containers"),
            doing: t!("Upgrading the containers"),
        },
        _ => return None,
    })
}

/// What bootc is doing, as uupd names it: the step's own words, and the page's sentence for it
fn system_phase(detail: &str) -> Option<(String, String)> {
    Some(match detail {
        "Bootc" => (t!("Starting"), t!("Looking for a new version of the system")),
        "Downloading" => (t!("Downloading"), t!("Downloading the new system")),
        "Importing" => (t!("Unpacking"), t!("Unpacking the new system")),
        "Deploying" => (t!("Setting up"), t!("Setting up the new system")),
        "Loading" => (t!("Finishing"), t!("Finishing the new system")),
        _ => return None,
    })
}

/// The step's own words for what it is doing, and the sentence the page says it in
fn step_activity(name: &str, step: &Step) -> (String, String) {
    match name {
        "System" => {
            let (phase, sentence) = system_phase(&step.detail).unwrap_or_else(|| (t!("Updating"), about("System").expect("known").doing));
            match step.percent {
                Some(percent) if percent > 0.0 => {
                    let percent = i18n::percent(percent.round() / 100.0);
                    (t!("{phase} · {percent}", phase = phase, percent = percent), sentence)
                }
                _ => (phase, sentence),
            }
        }
        "Flatpak" => match step.detail.strip_prefix("Apps for User: ").filter(|user| !user.is_empty()) {
            Some(user) => (t!("Apps of {user}", user = user), t!("Updating the apps of {user}", user = user)),
            None => (t!("The machine’s apps"), t!("Updating the machine’s apps")),
        },
        "Distrobox" => (if step.detail.is_empty() { t!("Starting") } else { step.detail.clone() }, about("Distrobox").expect("known").doing),
        _ => (t!("Updating"), about(name).map(|a| a.doing).unwrap_or_else(|| t!("Updating {name}", name = name))),
    }
}

impl Updates {
    pub(crate) fn hero_kind(&self) -> &'static str {
        let update = self.update.borrow();
        let state = self.state.borrow();
        let machine = state.status.as_ref().and_then(|s| s.machine.as_ref());
        if state.starting || update.as_ref().is_some_and(|u| u.state == "running") {
            "running"
        } else if state.confirming {
            "restart"
        } else if update.as_ref().is_some_and(|u| u.state == "failed") {
            "failed"
        } else if machine.is_some_and(|m| m.latest.is_some()) {
            "previous"
        } else if machine.is_some_and(|m| m.next.is_some()) {
            "ready"
        } else if update.as_ref().is_some_and(|u| u.state == "done") {
            "done"
        } else {
            "idle"
        }
    }

    /// The words and colour of a step's state, and whether it is turning
    fn step_state(&self, name: &str, step: &Step, update: &Update) -> (String, &'static str, bool) {
        let ended = update.state != "running";
        let next = self.state.borrow().status.as_ref().and_then(|s| s.machine.as_ref()).is_some_and(|m| m.next.is_some());
        match step.state.as_str() {
            "running" => (step_activity(name, step).0, "accent", true),
            "done" => (if name == "System" && next { t!("Ready for the restart") } else { t!("Done") }, "ok", false),
            "current" => (t!("Already the newest"), "ok", false),
            "failed" => (t!("Did not finish"), "off", false),
            _ if !ended => (t!("Waiting"), "", false),
            // uupd leaves out the system when there is no new version, and says so only in its debug log
            _ if name == "System" && update.state == "done" => (t!("Already the newest"), "ok", false),
            _ => (t!("Not run"), "", false),
        }
    }

    /// A step's state, set on the row in place: the spinner keeps turning while the words change. Without
    /// the journal there is nothing to say about any one step, and the rows only say what they update. The
    /// containers' own log is the account's, so their step always has something to say once it has begun.
    fn set_step(&self, nodes: &StepNodes, name: &str, step: &Step, update: Option<&Update>) {
        let known = update.is_some_and(|u| u.journal || (name == "Distrobox" && u.containers.is_some()));
        let (label, tone, spinning) = match update.filter(|_| known) {
            Some(update) => self.step_state(name, step, update),
            None => (String::new(), "", false),
        };
        nodes.badge.set_visible(!label.is_empty());
        if tone.is_empty() {
            nodes.badge.set_css_classes(&["state"]);
        } else {
            nodes.badge.set_css_classes(&["state", tone]);
        }
        nodes.label.set_label(&label);
        nodes.spinner.set_visible(spinning);
        nodes.spinner.set_spinning(spinning);
        nodes.icon.set_css_classes(&["row-icon", "tone", if tone == "off" { "off" } else { "accent" }]);
    }

    /// What the Containers row adds to what it updates: which did not upgrade, or when they last did
    fn containers_detail(&self, update: Option<&Update>) -> String {
        let failed = update.and_then(|u| u.containers.as_ref()).map(|c| c.failed.clone()).unwrap_or_default();
        if !failed.is_empty() {
            return t!(
                "Did not upgrade: {containers}. What they said is in the log: {command}",
                containers = i18n::list(&failed),
                command = "journalctl --user -u amethystora-pkg-upgrade"
            );
        }
        let upgraded = self.state.borrow().status.as_ref().and_then(|s| s.containers.upgraded);
        match upgraded {
            Some(upgraded) if update.is_none_or(|u| u.containers.is_none()) => t!("Last upgraded {when}", when = when(upgraded)),
            _ => String::new(),
        }
    }

    fn step_row(&self, name: &str, step: &Step, update: Option<&Update>) -> gtk::Box {
        let about = about(name).unwrap_or(About { icon: "box", title: name.to_string(), text: String::new(), what: String::new(), doing: String::new() });
        let icon_box = row_icon(about.icon, &["row-icon"]);
        let spinner = icon("spinner", 15);
        let label = ui::line("", &[]);
        let badge = ui::hbox(7, &["state"]);
        badge.set_valign(gtk::Align::Center);
        badge.append(&spinner);
        badge.append(&label);
        let nodes = StepNodes { icon: icon_box.clone(), spinner, label, badge: badge.clone() };
        self.set_step(&nodes, name, step, update);
        self.live.borrow_mut().step_nodes.insert(name.to_string(), nodes);
        let extra = if name == "Distrobox" { self.containers_detail(update) } else { String::new() };
        let text = ui::vbox(2, &["row-text"]);
        text.set_hexpand(true);
        text.append(&ui::label(&about.title, &["row-title"]));
        text.append(&ui::label(&about.text, &["row-detail"]));
        if !extra.is_empty() {
            text.append(&ui::label(&extra, &["row-detail"]));
        }
        let row = ui::hbox(14, &["row"]);
        row.append(&icon_box);
        row.append(&text);
        row.append(&badge);
        row
    }

    fn steps_card(&self) -> gtk::Box {
        let update = self.update.borrow();
        let mut idle: Vec<(String, Step)> = backend::MODULES.iter().map(|m| (m.to_string(), Step::default())).collect();
        if self.state.borrow().status.as_ref().is_some_and(|s| s.containers.count > 0) {
            idle.push(("Distrobox".into(), Step::default()));
        }
        let steps = update.as_ref().map(|u| u.steps.clone()).unwrap_or(idle);
        self.live.borrow_mut().step_nodes.clear();
        let card = ui::vbox(0, &["card", "list"]);
        for (name, step) in &steps {
            card.append(&self.step_row(name, step, update.as_ref()));
        }
        card
    }

    /// What is happening right now, in a few words
    fn activity(update: Option<&Update>) -> String {
        let Some(update) = update else { return t!("Starting the update…") };
        match update.steps.iter().find(|(_, step)| step.state == "running") {
            Some((name, step)) => step_activity(name, step).1,
            None => t!("Looking for new versions…"),
        }
    }

    fn running_title(update: Option<&Update>) -> String {
        if update.is_some_and(|u| u.automatic) {
            t!("Updating by itself")
        } else if update.is_some_and(|u| u.retry) {
            t!("Trying again")
        } else {
            t!("Updating")
        }
    }

    // --- Building blocks ----------------------------------------------------------------------------------

    fn button(&self, label: &str, icon_name: Option<&str>, primary: bool, small: bool, disabled: bool, on_click: impl Fn(&Rc<Updates>) + 'static) -> gtk::Button {
        let mut classes = vec!["btn"];
        if small {
            classes.push("small");
        }
        if primary {
            classes.push("primary");
        }
        let weak = self.me.clone();
        let button = ui::button(label, icon_name.map(|name| icon(name, 16)), &classes, move || {
            if let Some(this) = weak.upgrade() {
                on_click(&this);
            }
        });
        button.set_sensitive(!disabled);
        button
    }

    fn link_button(&self, label: &str, icon_name: &str, on_click: impl Fn(&Rc<Updates>) + 'static) -> gtk::Button {
        let weak = self.me.clone();
        let button = ui::button(label, Some(icon(icon_name, 15)), &["link", "small"], move || {
            if let Some(this) = weak.upgrade() {
                on_click(&this);
            }
        });
        button.set_halign(gtk::Align::Start);
        button
    }

    fn manual_link(&self, label: &str, icon_name: &str, page: &'static str) -> gtk::Button {
        self.link_button(label, icon_name, move |_| process::manual(page))
    }

    fn copy_button(command: &str) -> gtk::Button {
        ui::copy_button(command, icon("copy", 14), &t!("Copy"), &t!("Copied"), &t!("Copy the command"), &["btn", "small"])
    }

    fn section(title: &str, body: &[gtk::Widget]) -> gtk::Box {
        let section = ui::vbox(12, &["section"]);
        // In capitals the language's own way; the stylesheet's text-transform is not measured by GTK
        section.append(&ui::label(&title.to_uppercase(), &["h2"]));
        for child in body {
            section.append(child);
        }
        section
    }

    fn callout(tone: &str, title: &str, text: &str, extra: Option<gtk::Widget>) -> gtk::Box {
        let callout = ui::hbox(14, &["callout", tone]);
        let mark = icon(if tone == "check" { "alert" } else { "info" }, 20);
        mark.set_valign(gtk::Align::Start);
        callout.append(&mark);
        let body = ui::vbox(6, &["grow"]);
        body.set_hexpand(true);
        body.append(&ui::label(title, &["strong"]));
        if !text.is_empty() {
            body.append(&ui::limit(&ui::label(text, &["p"]), 640));
        }
        if let Some(extra) = extra {
            body.append(&extra);
        }
        callout.append(&body);
        callout
    }

    fn command_block(lead: &str, command: &str) -> gtk::Box {
        let wrap = ui::vbox(8, &["command-wrap"]);
        wrap.append(&ui::label(lead, &["soft", "small"]));
        let line = ui::hbox(12, &["command"]);
        let code = ui::label(command, &["code"]);
        code.set_hexpand(true);
        code.set_selectable(true);
        code.set_direction(gtk::TextDirection::Ltr);
        line.append(&code);
        let copy = Self::copy_button(command);
        copy.set_valign(gtk::Align::Center);
        line.append(&copy);
        wrap.append(&ui::limit(&line, 560));
        wrap
    }

    fn actions(children: Vec<gtk::Widget>, center: bool) -> adw::WrapBox {
        let actions = ui::wrap(12, 10, &["actions"]);
        actions.set_align(if center { 0.5 } else { 0.0 });
        if center {
            actions.set_halign(gtk::Align::Center);
        }
        for child in children {
            actions.append(&child);
        }
        actions
    }

    // --- The hero -----------------------------------------------------------------------------------------

    /// The stone of the boot splash, lit, with a band of light passing down it while something happens
    fn gem(active: bool) -> Gem {
        let gem = Gem::halo(132.0, 116.0, 30.0);
        gem.add_css_class("gem");
        gem.set_motion(if active { Motion::Sweep } else { Motion::Still });
        gem
    }

    fn hero(&self, kind: &str) -> gtk::Box {
        let hero = ui::vbox(0, &["hero"]);
        hero.set_halign(gtk::Align::Fill);
        let centered = |label: gtk::Label| {
            label.set_justify(gtk::Justification::Center);
            label.set_xalign(0.5);
            label.set_halign(gtk::Align::Center);
            label
        };
        if kind == "running" {
            let title = centered(ui::label("", &["h1"]));
            let lead = centered(ui::label("", &["lead"]));
            let bar = Bar::new();
            bar.set_hexpand(true);
            bar.set_valign(gtk::Align::Center);
            let percent = ui::line("", &["percent"]);
            {
                let mut live = self.live.borrow_mut();
                live.title = Some(title.clone());
                live.lead = Some(lead.clone());
                live.bar = Some(bar.clone());
                live.percent = Some(percent.clone());
            }
            self.update_live();
            hero.append(&Self::gem(true));
            hero.append(&title);
            hero.append(&ui::limit_center(&lead, 580));
            let progress = ui::hbox(14, &["progress"]);
            progress.append(&bar);
            progress.append(&percent);
            hero.append(&ui::limit_center(&progress, 460));
            hero.append(&centered(ui::label(&t!("You can keep working, or close this window: the update carries on without it."), &["note"])));
            return hero;
        }

        let status = self.state.borrow().status.clone().unwrap_or_default();
        let update = self.update.borrow().clone();
        let next = status.machine.as_ref().and_then(|m| m.next.clone());
        let recent = status.recent.clone();
        let auto = &status.automatic;
        let title;
        let lead;
        // Each ends in a time, which can end in "p.m.": joined with dots between them rather than after
        let mut notes: Vec<String> = Vec::new();
        let mut actions: Vec<gtk::Widget> = Vec::new();
        match kind {
            "failed" => {
                let update = update.as_ref().expect("a failed update");
                let failed: Vec<String> = update
                    .steps
                    .iter()
                    .filter(|(_, step)| step.state == "failed")
                    .map(|(name, _)| about(name).map(|a| a.what).unwrap_or_else(|| name.clone()))
                    .collect();
                let finished = update.steps.iter().any(|(_, step)| step.state == "done" || step.state == "current");
                title = if !failed.is_empty() && finished { t!("Some updates did not finish") } else { t!("The update did not finish") };
                lead = if !failed.is_empty() {
                    let stopped = t!("Updating {steps} stopped with an error.", steps = i18n::list(&failed));
                    if finished { format!("{stopped} {}", t!("Everything else is up to date.")) } else { stopped }
                } else {
                    update.errors.first().cloned().unwrap_or_else(|| t!("The updater stopped before it got to the end."))
                };
                if update.retrying {
                    notes.push(t!("It tries again by itself in a minute"));
                }
                actions.push(self.button(&t!("Try again"), Some("refresh"), true, false, false, |this| this.update_now()).upcast());
                if next.is_some() {
                    actions.push(self.button(&t!("Restart now"), Some("power"), false, false, false, |this| this.restart_now()).upcast());
                }
                if let Some(agent) = self.ask_agent() {
                    actions.push(agent.upcast());
                }
            }
            "previous" => {
                let machine = status.machine.as_ref().expect("a machine");
                let (running, latest) = (&machine.running, machine.latest.as_ref().expect("the newest"));
                title = t!("You’re on the previous version");
                lead = [
                    match running.built {
                        Some(built) => t!(
                            "This machine started Amethystora {version}, built {date}, from the boot menu.",
                            version = &running.version,
                            date = day(built)
                        ),
                        None => t!("This machine started Amethystora {version} from the boot menu.", version = &running.version),
                    },
                    match latest.built {
                        Some(built) => t!("The newest, built {date}, is still the one it starts by default.", date = day(built)),
                        None => t!("The newest is still the one it starts by default."),
                    },
                    t!("Keep this one while the newest has a problem you are waiting to see fixed."),
                ]
                .join(" ");
                notes.push(t!("Keeping it asks for your password"));
                let keeping = self.state.borrow().keeping;
                actions.push(self.button(&t!("Keep this version"), Some("back"), true, false, keeping, |this| this.keep()).upcast());
                actions.push(self.button(&t!("Restart into the latest"), Some("power"), false, false, false, |_| restart()).upcast());
                actions.push(self.manual_link(&t!("Rolling back in the manual"), "book", "rolling-back").upcast());
            }
            "ready" => {
                title = t!("Restart to finish");
                let version = next.as_ref().map(|n| n.version.clone()).unwrap_or_default();
                lead = if status.machine.as_ref().is_some_and(|m| m.quick) {
                    t!(
                        "Amethystora {version} is ready next to the system you are running. It leaves the kernel as it is, so the restart takes seconds: only the system starts again, not the computer, and your files and settings stay as they are.",
                        version = version
                    )
                } else {
                    t!(
                        "Amethystora {version} is ready next to the system you are running. It takes over when you restart, all at once, and your files and settings stay as they are.",
                        version = version
                    )
                };
                notes.push(t!("No hurry: shutting down at the end of the day does it too"));
                actions.push(self.button(&t!("Restart now"), Some("power"), true, false, false, |this| this.restart_now()).upcast());
                // Restart while you are away, once a night time is chosen (ame update night-restart)
                if let Some(night) = status.night.time.as_ref() {
                    let arming = self.state.borrow().arming;
                    if let Some(at) = status.night.armed {
                        notes.push(t!("Restarts {when} if the screen is locked, no app holds the logout and it is on mains power", when = when(at)));
                        actions.push(self.button(&t!("Not tonight"), None, false, false, arming, |this| this.tonight(false)).upcast());
                    } else {
                        actions.push(self.button(&t!("Tonight at {time}", time = night), Some("clock"), false, false, arming, |this| this.tonight(true)).upcast());
                    }
                }
                actions.push(self.button(&t!("Check again"), Some("refresh"), false, false, false, |this| this.update_now()).upcast());
            }
            "restart" => {
                title = t!("Restart now?");
                lead = t!("Your apps close, and the login screen is back in a few seconds. Save what is open in them first.");
                let restarting = self.state.borrow().restarting;
                actions.push(self.button(&t!("Restart"), Some("power"), true, false, restarting, |this| this.restart_quickly()).upcast());
                actions.push(self.button(&t!("Not now"), None, false, false, restarting, |this| this.not_now()).upcast());
            }
            "done" => {
                title = t!("Everything is up to date");
                lead = t!("The system, your apps and your command-line tools are the newest there are.");
                if let Some(finished) = update.as_ref().and_then(|u| u.finished) {
                    notes.push(t!("Checked {when}", when = when(finished)));
                }
                actions.push(self.button(&t!("Check again"), Some("refresh"), false, false, false, |this| this.update_now()).upcast());
            }
            _ => {
                if let Some(recent) = recent.as_ref().filter(|r| r.result != "success") {
                    title = t!("The last update did not finish");
                    lead = t!("It stopped {when}. Try again; if it fails again, this window shows where.", when = when(recent.finished).trim_end_matches('.'));
                    actions.push(self.button(&t!("Try again"), Some("refresh"), true, false, false, |this| this.update_now()).upcast());
                    if let Some(agent) = self.ask_agent() {
                        actions.push(agent.upcast());
                    }
                } else if self.waiting().is_some_and(|w| w.any()) {
                    title = t!("An update is waiting");
                    lead = t!("What is ready to download is listed below. Update now to install it: nothing changes before the next restart.");
                    actions.push(self.button(&t!("Update now"), Some("download"), true, false, false, |this| this.update_now()).upcast());
                } else {
                    title = t!("Updates come by themselves");
                    lead = t!("The system, your apps and your command-line tools are updated in the background while you work. Update now to have anything new straight away.");
                    actions.push(self.button(&t!("Update now"), Some("download"), true, false, false, |this| this.update_now()).upcast());
                }
                if let Some(recent) = recent.as_ref().filter(|r| r.result == "success") {
                    notes.push(t!("Last updated {when}", when = when(recent.finished)));
                }
                if auto.enabled && auto.next.is_some() {
                    notes.push(t!("Next automatic update {when}", when = when(auto.next.expect("checked"))));
                } else if !auto.enabled {
                    notes.push(t!("Automatic updates are off"));
                }
            }
        }
        hero.append(&Self::gem(false));
        hero.append(&centered(ui::label(&title, &["h1"])));
        hero.append(&ui::limit_center(&centered(ui::label(&lead, &["lead"])), 580));
        hero.append(&Self::actions(actions, true));
        if !notes.is_empty() {
            // The page joined them with two spaces either side, which HTML showed as one
            hero.append(&centered(ui::label(&notes.join(" · "), &["note"])));
        }
        hero
    }

    /// The running hero, updated in place so that the stone's light is not restarted with every line
    pub(crate) fn update_live(&self) {
        let update = self.update.borrow().clone();
        let (title, lead, bar, percent) = {
            let live = self.live.borrow();
            match (&live.title, &live.lead, &live.bar, &live.percent) {
                (Some(t), Some(l), Some(b), Some(p)) => (t.clone(), l.clone(), b.clone(), p.clone()),
                _ => return,
            }
        };
        title.set_label(&Self::running_title(update.as_ref()));
        lead.set_label(&Self::activity(update.as_ref()));
        let overall = update.as_ref().filter(|u| u.journal).and_then(|u| u.overall).map(|o| o.clamp(2.0, 100.0));
        bar.set_fraction(overall.map(|o| o / 100.0));
        percent.set_label(&overall.map(|o| i18n::percent(o.round() / 100.0)).unwrap_or_default());
        percent.set_visible(overall.is_some());
        let steps = self.live.borrow().steps.clone();
        if let (Some(steps), Some(update)) = (steps, update.as_ref()) {
            let missing = update.steps.iter().any(|(name, _)| !self.live.borrow().step_nodes.contains_key(name));
            if missing {
                // A module the page did not know of has started, such as Distrobox
                let card = self.steps_card();
                if let Some(parent) = steps.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
                    parent.insert_child_after(&card, Some(&steps));
                    parent.remove(&steps);
                }
                self.live.borrow_mut().steps = Some(card);
            } else {
                let live = self.live.borrow();
                for (name, step) in &update.steps {
                    if let Some(nodes) = live.step_nodes.get(name) {
                        self.set_step(nodes, name, step, Some(update));
                    }
                }
            }
        }
        let log = self.live.borrow().log.clone();
        if let Some((label, scroller)) = log {
            self.fill_log(&label, &scroller);
        }
    }

    // --- This machine and automatic updates ---------------------------------------------------------------

    fn version_row(label: &str, deployment: &Deployment, extra: Option<gtk::Widget>) -> gtk::Box {
        let row = ui::hbox(14, &["row"]);
        let text = ui::vbox(2, &["row-text"]);
        text.set_hexpand(true);
        text.append(&ui::label(&label.to_uppercase(), &["row-label"]));
        text.append(&ui::label(format!("Amethystora {}", deployment.version).trim(), &["row-title"]));
        let detail: Vec<String> =
            [deployment.built.map(|built| t!("Built {date}", date = day(built))).unwrap_or_default(), deployment.image.clone()].into_iter().filter(|s| !s.is_empty()).collect();
        text.append(&ui::label(&detail.join(" · "), &["row-detail"]));
        if let Some(extra) = extra {
            text.append(&extra);
        }
        row.append(&text);
        row
    }

    fn machine_section(&self) -> gtk::Box {
        let info = self.state.borrow().status.as_ref().and_then(|s| s.machine.clone());
        let Some(info) = info else {
            let card = ui::vbox(0, &["card", "list"]);
            let row = ui::hbox(14, &["row", "soft"]);
            row.append(&ui::label(&t!("This machine did not say which system it runs."), &[]));
            card.append(&row);
            return Self::section(&t!("This machine"), &[card.upcast()]);
        };
        let held = held_on(&info.running.image);
        let mut body: Vec<gtk::Widget> = Vec::new();
        if !held.is_empty() {
            body.push(
                Self::callout(
                    "check",
                    &t!("This machine is held on one build"),
                    &t!("It runs {build} and receives no new system until it follows a stream again. Apps and command-line tools still update.", build = &held),
                    Some(self.manual_link(&t!("Following a stream again"), "book", "rolling-back#holding-on-to-one-build").upcast()),
                )
                .upcast(),
            );
        }
        let card = ui::vbox(0, &["card", "list"]);
        let running = if info.latest.is_some() { t!("Running now, from the boot menu") } else { t!("Running now") };
        card.append(&Self::version_row(&running, &info.running, None));
        if let Some(latest) = &info.latest {
            card.append(&Self::version_row(&t!("The newest, started by default"), latest, None));
        }
        if let Some(next) = &info.next {
            card.append(&Self::version_row(&t!("After the next restart"), next, None));
        }
        if let Some(previous) = &info.previous {
            let link = self.manual_link(&t!("Going back to it"), "back", "rolling-back");
            card.append(&Self::version_row(&t!("Kept to go back to"), previous, Some(link.upcast())));
        }
        body.push(card.upcast());
        Self::section(&t!("This machine"), &body)
    }

    fn automatic_section(&self) -> Option<gtk::Box> {
        let status = self.state.borrow().status.clone()?;
        let auto = status.automatic;
        let switching = self.state.borrow().switching;
        let detail = if auto.enabled {
            t!("New versions download in the background, and a new system waits for the next restart.")
        } else {
            t!("Nothing is updated unless you update it here, or with ame update in a terminal.")
        };
        let row = ui::hbox(14, &["row"]);
        let mark = row_icon("clock", &["row-icon", "tone", if auto.enabled { "ok" } else { "info" }]);
        row.append(&mark);
        let text = ui::vbox(2, &["row-text"]);
        text.set_hexpand(true);
        text.append(&ui::label(&if auto.enabled { t!("On") } else { t!("Off") }, &["row-title"]));
        text.append(&ui::label(&detail, &["row-detail"]));
        if let (true, Some(next)) = (auto.enabled, auto.next) {
            text.append(&ui::label(&t!("Next {when}", when = when(next)), &["row-detail"]));
        }
        row.append(&text);
        let side = ui::vbox(4, &["row-side"]);
        side.set_valign(gtk::Align::Center);
        let enabled = auto.enabled;
        let toggle = self.button(&if enabled { t!("Turn off") } else { t!("Turn on") }, None, false, true, switching, move |this| this.set_automatic(!enabled));
        toggle.set_halign(gtk::Align::End);
        side.append(&toggle);
        let ask = ui::label(&t!("Asks for your password"), &["faint", "small"]);
        ask.set_halign(gtk::Align::End);
        side.append(&ask);
        row.append(&side);
        let card = ui::vbox(0, &["card", "list"]);
        card.append(&row);
        Some(Self::section(&t!("Automatic updates"), &[card.upcast()]))
    }

    // --- What is waiting ----------------------------------------------------------------------------------

    /// What the last look found, while it still says something: not on a sealed machine, and not once an update has
    /// finished since it looked, which may have installed it
    fn waiting(&self) -> Option<Waiting> {
        let state = self.state.borrow();
        let status = state.status.as_ref()?;
        let mut waiting = status.waiting.clone()?;
        let updated = [
            status.recent.as_ref().filter(|r| r.result == "success").map(|r| r.finished),
            self.update.borrow().as_ref().filter(|u| u.state == "done").and_then(|u| u.finished),
        ];
        if let Some(checked) = waiting.checked {
            if updated.into_iter().flatten().any(|finished| finished > checked) {
                waiting = Waiting { error: waiting.error, ..Default::default() };
            }
        }
        Some(waiting)
    }

    fn waiting_row(icon_name: &str, title: &str, details: &[String]) -> gtk::Box {
        let row = ui::hbox(14, &["row"]);
        row.append(&row_icon(icon_name, &["row-icon", "tone", "accent"]));
        let text = ui::vbox(2, &["row-text"]);
        text.set_hexpand(true);
        text.append(&ui::label(title, &["row-title"]));
        for detail in details.iter().filter(|d| !d.is_empty()) {
            text.append(&ui::label(detail, &["row-detail"]));
        }
        row.append(&text);
        row
    }

    fn package_row(package: &Package) -> gtk::Box {
        let row = Self::waiting_row(
            "box",
            &package.name,
            &[t!("From {current} to {next}", current = &package.from, next = &package.to), t!("Added to the system with rpm-ostree install")],
        );
        // A package's name and versions read left to right in every language
        row.set_direction(gtk::TextDirection::Ltr);
        row
    }

    /// A newer system and newer versions of the layered packages, found without installing them
    /// (amethystora-update-check). Installing is Update now's.
    fn waiting_section(&self, kind: &str) -> Option<gtk::Box> {
        if !matches!(kind, "idle" | "done" | "failed") {
            return None;
        }
        let waiting = self.waiting()?;
        let looking = self.state.borrow().looking;
        let card = ui::vbox(0, &["card", "list"]);
        if let Some((version, built)) = &waiting.system {
            let title = if version.is_empty() { t!("A new version of the system") } else { format!("Amethystora {version}") };
            let built = built.map(|b| t!("Built {date}", date = day(b))).unwrap_or_default();
            card.append(&Self::waiting_row("download", &title, &[built, t!("Takes over at the restart after it is installed")]));
        }
        for package in &waiting.packages {
            card.append(&Self::package_row(package));
        }
        if !waiting.any() {
            let row = ui::hbox(14, &["row", "soft"]);
            let text = match waiting.checked {
                Some(checked) => t!("Nothing is waiting. Last looked {when}.", when = when(checked)),
                None => t!("Not looked since this version started. It looks once a day by itself, and when you press Check now."),
            };
            row.append(&ui::label(&text, &[]));
            card.append(&row);
        }
        let mut body: Vec<gtk::Widget> = Vec::new();
        if let Some(error) = &waiting.error {
            body.push(Self::callout("check", &t!("The last look did not work"), error, None).upcast());
        }
        body.push(card.upcast());
        let mut actions: Vec<gtk::Widget> = Vec::new();
        if waiting.any() && kind != "failed" {
            actions.push(self.button(&t!("Update now"), Some("download"), true, false, false, |this| this.update_now()).upcast());
        }
        actions.push(self.button(&if looking { t!("Checking…") } else { t!("Check now") }, Some("refresh"), false, true, looking, |this| this.look()).upcast());
        actions.push(self.manual_link(&t!("What is waiting, in the manual"), "book", "updates#what-is-waiting").upcast());
        body.push(Self::actions(actions, false).upcast());
        if let (true, Some(checked)) = (waiting.any(), waiting.checked) {
            body.push(ui::label(&t!("Looked {when}", when = when(checked)), &["faint", "small"]).upcast());
        }
        Some(Self::section(&t!("Waiting to be installed"), &body))
    }

    // --- Firmware -----------------------------------------------------------------------------------------

    fn firmware_row(device: &Device) -> gtk::Box {
        // LVFS's own words for an update that fixes something serious
        let urgent = device.urgency == "high" || device.urgency == "critical";
        let versions = if !device.current.is_empty() && !device.next.is_empty() {
            t!("From {current} to {next}", current = &device.current, next = &device.next)
        } else {
            device.next.clone()
        };
        let row = ui::hbox(14, &["row"]);
        let mark = row_icon("chip", &["row-icon", "tone", if urgent { "off" } else { "accent" }]);
        row.append(&mark);
        let text = ui::vbox(2, &["row-text"]);
        text.set_hexpand(true);
        text.append(&ui::label(&device.name, &["row-title"]));
        let detail: Vec<String> = [device.vendor.clone(), versions].into_iter().filter(|s| !s.is_empty()).collect();
        text.append(&ui::label(&detail.join(" · "), &["row-detail"]));
        if !device.summary.is_empty() {
            text.append(&ui::label(&device.summary, &["row-detail"]));
        }
        if device.restart {
            text.append(&ui::label(&t!("Written by the firmware itself at the next restart"), &["row-detail"]));
        }
        row.append(&text);
        if urgent {
            let state = ui::hbox(0, &["state", "off"]);
            state.set_valign(gtk::Align::Center);
            state.append(&ui::line(&t!("Important"), &[]));
            row.append(&state);
        }
        row
    }

    /// The devices whose makers published new firmware on LVFS. Installing it is fwupdmgr's, in a terminal.
    fn firmware_section(&self) -> Option<gtk::Box> {
        let chips = self.state.borrow().status.as_ref().map(|s| s.firmware.clone())?;
        let checking = self.state.borrow().checking_firmware;
        let card = ui::vbox(0, &["card", "list"]);
        if chips.devices.is_empty() {
            let row = ui::hbox(14, &["row", "soft"]);
            let text = chips.error.clone().unwrap_or_else(|| {
                t!("No firmware updates. fwupd looks for them every day on LVFS, where the makers of this machine and its devices publish them.")
            });
            row.append(&ui::label(&text, &[]));
            card.append(&row);
        } else {
            for device in &chips.devices {
                card.append(&Self::firmware_row(device));
            }
        }
        let mut actions: Vec<gtk::Widget> = Vec::new();
        if !chips.devices.is_empty() {
            actions.push(self.button(&t!("Update firmware…"), Some("terminal"), true, false, false, |_| process::in_terminal(&["fwupdmgr", "update"])).upcast());
        }
        actions.push(
            self.button(&if checking { t!("Checking…") } else { t!("Check now") }, Some("refresh"), false, true, checking, |this| this.check_firmware()).upcast(),
        );
        actions.push(self.manual_link(&t!("Firmware in the manual"), "book", "updates#firmware").upcast());
        Some(Self::section(&t!("Firmware"), &[card.upcast(), Self::actions(actions, false).upcast()]))
    }

    // --- What the updater said ----------------------------------------------------------------------------

    fn fill_log(&self, label: &gtk::Label, scroller: &gtk::ScrolledWindow) {
        let update = self.update.borrow();
        let Some(update) = update.as_ref() else { return };
        let mut lines = update.log.clone();
        for failure in &update.failures {
            if !failure.output.is_empty() {
                lines.extend([String::new(), format!("{}:", failure.context), failure.output.clone()]);
            }
        }
        let adjustment = scroller.vadjustment();
        let at_end = adjustment.upper() - adjustment.value() - adjustment.page_size() < 24.0;
        label.set_label(&lines.join("\n"));
        if at_end {
            let scroller = scroller.clone();
            gtk::glib::idle_add_local_once(move || {
                let adjustment = scroller.vadjustment();
                adjustment.set_value(adjustment.upper());
            });
        }
    }

    fn details_section(&self) -> Option<gtk::Widget> {
        let update = self.update.borrow().clone()?;
        let command = format!("journalctl -eu {}", update.unit);
        if !update.journal {
            if update.state == "running" {
                return None;
            }
            return Some(
                Self::section(
                    &t!("Details"),
                    &[Self::command_block(
                        &t!("What the updater said is in the system journal, which this account cannot read. An administrator sees it with:"),
                        &format!("sudo {command}"),
                    )
                    .upcast()],
                )
                .upcast(),
            );
        }
        let log = ui::label("", &["log"]);
        log.set_selectable(true);
        log.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        log.set_valign(gtk::Align::Start);
        log.set_direction(gtk::TextDirection::Ltr);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(320)
            .propagate_natural_height(true)
            .child(&log)
            .css_classes(["log-scroller"])
            .build();
        self.live.borrow_mut().log = Some((log.clone(), scroller.clone()));
        self.fill_log(&log, &scroller);
        let open = self.state.borrow().details;
        let details = ui::vbox(0, &["card", "details"]);
        let chevron = icon("chevron", 16).mirror_in_rtl();
        chevron.add_css_class("chevron");
        let summary = ui::hbox(10, &["summary"]);
        summary.append(&chevron);
        summary.append(&ui::line(&t!("What the updater said"), &[]));
        let toggle = gtk::Button::builder().child(&summary).css_classes(["plain", "summary-button"]).build();
        let revealer = gtk::Revealer::builder().reveal_child(open).transition_type(gtk::RevealerTransitionType::SlideDown).build();
        let inside = ui::vbox(0, &[]);
        inside.append(&scroller);
        let foot = ui::vbox(0, &["details-foot"]);
        foot.append(&Self::command_block(&t!("All of it, in a terminal:"), &command));
        inside.append(&foot);
        revealer.set_child(Some(&inside));
        if open {
            details.add_css_class("open");
        }
        let weak = self.me.clone();
        let (shown, card) = (revealer.clone(), details.clone());
        toggle.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else { return };
            let open = !shown.reveals_child();
            this.state.borrow_mut().details = open;
            shown.set_reveal_child(open);
            if open {
                card.add_css_class("open");
                let scroller = scroller.clone();
                gtk::glib::idle_add_local_once(move || {
                    let adjustment = scroller.vadjustment();
                    adjustment.set_value(adjustment.upper());
                });
            } else {
                card.remove_css_class("open");
            }
        });
        details.append(&toggle);
        details.append(&revealer);
        Some(details.upcast())
    }

    // --- The page -----------------------------------------------------------------------------------------

    /// The window: the top bar with the wordmark, and the page under it
    pub(crate) fn frame(&self) -> gtk::Widget {
        let top = ui::hbox(16, &["top"]);
        let brand = amethystora_ui::brand::in_a_line(&t!("Updates"), 22.0);
        brand.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Updates"))]);
        brand.set_hexpand(true);
        brand.set_halign(gtk::Align::Start);
        // Level with the window's buttons over the start of the bar, which stay at its top when the links
        // wrap onto a second line
        let mark = brand.clone();
        self.shell.window.connect_notify_local(Some("css-classes"), move |window, _| {
            mark.set_valign(if window.has_css_class("controls-start") { gtk::Align::Start } else { gtk::Align::Center });
        });
        top.append(&brand);
        let links = ui::wrap(20, 6, &["top-links"]);
        links.set_align(1.0);
        links.set_valign(gtk::Align::Center);
        links.upcast_ref::<gtk::Widget>().update_property(&[gtk::accessible::Property::Label(&t!("Elsewhere"))]);
        let releases = ui::button(&t!("Release notes"), Some(icon("external", 17)), &["link", "quiet"], {
            let window = self.shell.gtk_window();
            move || process::open_uri(Some(&window), backend::RELEASES)
        });
        let manual = ui::button(&t!("Updates in the manual"), Some(icon("book", 17)), &["link", "quiet"], || process::manual("updates"));
        links.append(&releases);
        links.append(&manual);
        top.append(&links);
        let controls = frame::controls();
        controls.set_valign(gtk::Align::Center);
        top.append(&controls);
        let handle = gtk::WindowHandle::builder().child(&top).build();

        let clamp = adw::Clamp::builder().maximum_size(820).tightening_threshold(820).child(&self.view).build();
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
        scroller.add_css_class("main");
        let frame = ui::vbox(0, &[]);
        frame.append(&handle);
        frame.append(&scroller);
        frame.upcast()
    }

    pub(crate) fn render(&self) {
        *self.live.borrow_mut() = Live::default();
        ui::clear(&self.view);
        self.view.set_css_classes(&["page"]);
        if self.state.borrow().status.is_none() {
            let loading = ui::vbox(0, &["loading"]);
            let spinner = icon("spinner", 28);
            spinner.set_spinning(true);
            loading.append(&spinner);
            self.view.append(&loading);
            return;
        }
        let kind = self.hero_kind();
        self.live.borrow_mut().kind = kind.to_string();
        let top = self.hero(kind);
        let steps = self.steps_card();
        self.live.borrow_mut().steps = Some(steps.clone());
        self.view.append(&top);
        let (notice, kept) = {
            let state = self.state.borrow();
            (state.notice.clone(), state.kept)
        };
        if !notice.is_empty() {
            self.view.append(&Self::callout("check", &t!("That did not work"), &notice, None));
        }
        if kept {
            self.view.append(&Self::callout(
                "info",
                &t!("This version is the one the machine starts now"),
                &t!("The next update brings the newest back, automatic or not. To stay on this one until a fix is out, turn automatic updates off below, or hold the machine on this build."),
                Some(self.manual_link(&t!("Holding on to one build"), "book", "rolling-back#holding-on-to-one-build").upcast()),
            ));
        }
        if let Some(waited) = self.waited_callout(kind) {
            self.view.append(&waited);
        }
        if let Some(saving) = self.saving_callout() {
            self.view.append(&saving);
        }
        let steps_title = if kind == "running" || self.update.borrow().is_some() { t!("This update") } else { t!("What gets updated") };
        self.view.append(&Self::section(&steps_title, &[steps.upcast()]));
        if let Some(details) = self.details_section() {
            self.view.append(&details);
        }
        if let Some(waiting) = self.waiting_section(kind) {
            self.view.append(&waiting);
        }
        self.view.append(&self.machine_section());
        if let Some(firmware) = self.firmware_section() {
            self.view.append(&firmware);
        }
        if let Some(automatic) = self.automatic_section() {
            self.view.append(&automatic);
        }
        let terminal = ui::hbox(0, &["terminal"]);
        terminal.set_halign(gtk::Align::Center);
        for piece in t_parts!("Also in a terminal: {command}", command = "ame update") {
            match piece {
                Piece::Text(text) => terminal.append(&ui::line(&text, &[])),
                Piece::Value(_) => {
                    let code = ui::line("ame update", &["code"]);
                    code.set_direction(gtk::TextDirection::Ltr);
                    terminal.append(&code);
                }
            }
        }
        self.view.append(&terminal);
    }

    /// A night that did not restart (Restart while you are away), said while the update still waits for it
    fn waited_callout(&self, kind: &str) -> Option<gtk::Box> {
        let status = self.state.borrow().status.clone()?;
        let waited = status.night.last.clone()?;
        if kind == "running" || !status.machine.as_ref().is_some_and(|m| m.next.is_some()) {
            return None;
        }
        let at = time(waited.night);
        let text = match waited.why.as_str() {
            "late" => t!("The computer was asleep at {time}, so it did not restart.", time = &at),
            "unlocked" => t!("The screen was not locked at {time}, so nothing was closed.", time = &at),
            "inhibited" => match waited.app.as_deref().and_then(backend::app_name) {
                Some(app) => t!("{app} had unsaved work, so nothing was closed.", app = app),
                None => t!("An app had unsaved work, so nothing was closed."),
            },
            "battery" => t!("The computer was on battery power, so it waited."),
            _ => t!("The restart was refused: somebody else may be signed in."),
        };
        let arming = self.state.borrow().arming;
        let mut buttons: Vec<gtk::Widget> = Vec::new();
        if status.night.time.is_some() && status.night.armed.is_none() {
            buttons.push(self.button(&t!("Tonight again"), Some("clock"), false, true, arming, |this| this.tonight(true)).upcast());
        }
        buttons.push(self.button(&t!("Restart now"), Some("power"), false, true, false, |this| this.restart_now()).upcast());
        Some(Self::callout("check", &t!("Not restarted last night"), &text, Some(Self::actions(buttons, false).upcast())))
    }

    /// Saving data: on a connection that counts it, the automatic updates wait (amethystora-metered)
    fn saving_callout(&self) -> Option<gtk::Box> {
        let metered = self.state.borrow().status.as_ref()?.metered.clone();
        if !metered.on {
            return None;
        }
        let mut text = t!("This connection counts data, so the automatic updates wait for one that does not, never longer than a week. Update now updates anyway.");
        if let Some(since) = metered.since {
            text = format!("{text} {}", t!("They have waited since {when}.", when = when(since)));
        }
        Some(Self::callout("info", &t!("Saving data"), &text, Some(self.manual_link(&t!("Saving data in the manual"), "book", "updates#saving-data").upcast())))
    }

    // --- What the buttons do ------------------------------------------------------------------------------

    /// Update anyway, from the pill's Saving data
    pub(crate) fn start_update(self: &Rc<Self>) {
        self.update_now();
    }

    /// Tonight's restart set or taken away, then what the helper says of it now
    fn tonight(self: &Rc<Self>, on: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.arming = true;
            state.notice.clear();
        }
        self.render();
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = backend::set_tonight(on).await;
            let night = backend::night().await;
            {
                let mut state = this.state.borrow_mut();
                state.arming = false;
                if let Some(status) = state.status.as_mut() {
                    status.night = night;
                }
                if let Err(error) = result {
                    state.notice = error;
                }
            }
            this.render();
        });
    }

    fn update_now(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            state.notice.clear();
            state.starting = true;
        }
        self.render();
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            if let Err(error) = this.start().await {
                {
                    let mut state = this.state.borrow_mut();
                    state.starting = false;
                    state.notice = error;
                }
                this.render();
            }
        });
    }

    /// Makes the running version the default, after an administrator's password. What it means for the
    /// updates to come is said straight after: the next one brings the newest back.
    fn keep(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            state.keeping = true;
            state.notice.clear();
        }
        self.render();
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = backend::keep_version().await;
            {
                let mut state = this.state.borrow_mut();
                state.keeping = false;
                if let (Some(machine), Some(status)) = (result.machine, state.status.as_mut()) {
                    status.machine = Some(machine);
                }
                if result.kept {
                    state.kept = true;
                } else if !result.declined {
                    state.notice = result.error.unwrap_or_default();
                }
            }
            this.render();
        });
    }

    /// GNOME's own restart dialog, unless the update can be finished with a soft reboot, which ends the session
    /// without it: then the window asks first
    fn restart_now(self: &Rc<Self>) {
        if !self.state.borrow().status.as_ref().and_then(|s| s.machine.as_ref()).is_some_and(|m| m.quick) {
            restart();
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.confirming = true;
            state.notice.clear();
        }
        self.render();
    }

    fn not_now(self: &Rc<Self>) {
        self.state.borrow_mut().confirming = false;
        self.render();
    }

    fn restart_quickly(self: &Rc<Self>) {
        self.state.borrow_mut().restarting = true;
        self.render();
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = backend::restart_quickly().await;
            if matches!(result, backend::Restart::Started) {
                return;
            }
            {
                let mut state = this.state.borrow_mut();
                state.restarting = false;
                state.confirming = false;
                if let backend::Restart::Failed(error) = &result {
                    state.notice = error.clone();
                }
            }
            this.render();
            if matches!(result, backend::Restart::Full) {
                restart();
            }
        });
    }

    fn set_automatic(self: &Rc<Self>, on: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.switching = true;
            state.notice.clear();
        }
        self.render();
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = backend::set_automatic(on).await;
            {
                let mut state = this.state.borrow_mut();
                state.switching = false;
                if let Some(status) = state.status.as_mut() {
                    status.automatic = result.automatic;
                }
                state.notice = result.error.unwrap_or_default();
            }
            this.render();
        });
    }

    /// Check now: the same look the timer runs, then what it found
    fn look(self: &Rc<Self>) {
        self.state.borrow_mut().looking = true;
        self.render();
        let booted = self.state.borrow().status.as_ref().and_then(|s| s.machine.as_ref()).map(|m| m.running.checksum.clone()).unwrap_or_default();
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = backend::look(&booted).await;
            {
                let mut state = this.state.borrow_mut();
                state.looking = false;
                match result {
                    Ok(waiting) => {
                        if let Some(status) = state.status.as_mut() {
                            status.waiting = waiting;
                        }
                    }
                    Err(error) => state.notice = error,
                }
            }
            this.render();
        });
    }

    fn check_firmware(self: &Rc<Self>) {
        self.state.borrow_mut().checking_firmware = true;
        self.render();
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = backend::refresh_firmware().await;
            {
                let mut state = this.state.borrow_mut();
                state.checking_firmware = false;
                if let Some(status) = state.status.as_mut() {
                    status.firmware = result;
                }
            }
            this.render();
        });
    }

    /// The failed update, handed to the AI agent in a terminal: it reads the updater's log itself and
    /// changes nothing until asked. Nothing at all while the account has the agentic features off.
    fn ask_agent(&self) -> Option<gtk::Button> {
        let agent = self.state.borrow().status.as_ref().is_some_and(|s| s.agent);
        agent.then(|| self.link_button(&t!("Ask the agent why"), "sparkle", |this| this.diagnose()))
    }
}

/// GNOME's own restart dialog, which gives open apps the chance to object, and restarts the computer; the open apps
/// are kept first for the login after it (Pick up where you left off)
fn restart() {
    gtk::glib::spawn_future_local(async {
        backend::keep_open_apps().await;
        process::launch("gnome-session-quit", &["--reboot"]);
    });
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_builds() {
        assert_eq!(held_on("ghcr.io/arsslensoft/amethystora:stable-44.20260922"), "stable-44.20260922");
        assert_eq!(held_on("ghcr.io/arsslensoft/amethystora:44.20260922"), "44.20260922");
        assert_eq!(held_on("ghcr.io/arsslensoft/amethystora:stable"), "");
        assert_eq!(held_on("ghcr.io/arsslensoft/amethystora-dx:latest-44.2026"), "");
        assert_eq!(image_parts("ghcr.io/arsslensoft/amethystora-dx:stable"), ("amethystora-dx".into(), "stable".into()));
    }
}
