//! Pick a look: the page a new account sees once (`amethystora-control welcome`, opened by the user-setup hook
//! 25-first-look.sh), with the theme, the style, how the dock grows, the pointer and the window layout on one page and a
//! preview of the desktop that changes as each is picked.
//!
//! Every row is the list's own entry of that id, and a pick runs that entry's command through backend.rs as the
//! entry's own card does; nothing is written into the page. A row whose entry the list does not give is not shown.
//! The preview draws from the picked theme's colors.toml, read from the themes the image ships, the moment it is
//! picked, while the theme itself takes a moment to apply.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use amethystora_ui::{adw, glib, gtk, palette, t, ui};
use gtk::cairo;
use gtk::prelude::*;

use crate::backend::Item;
use crate::page::heading;
use crate::Control;

const THEMES: &str = "/usr/share/amethystora/themes";
/// How long the preview's dock takes to change, before the session's own setting for animations
const DOCK_MS: f64 = 200.0;

thread_local! {
    /// Start fresh was pressed: the card stays away for as long as this window lives, however often the page is drawn again
    static FRESH: Cell<bool> = const { Cell::new(false) };
}

/// The list's entry that starts from a backup (`ame backup restore-from`)
const BACKUP_ENTRY: &str = "start-from-backup";

/// The entries the page shows, by id, in its order
const ROWS: [&str; 5] = ["theme", "style", "dock-magnify", "pointer", "layout"];

fn row_title(id: &str) -> String {
    match id {
        "theme" => t!("Theme"),
        "style" => t!("Style"),
        "dock-magnify" => t!("Dock"),
        "pointer" => t!("Pointer"),
        _ => t!("Windows"),
    }
}

/// A theme's id as the list gives it, which is a folder's name and nothing more
fn theme_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The first line of one of a theme's files (style.theme, cursor.theme)
fn theme_file(theme: &str, name: &str) -> Option<String> {
    if !theme_id(theme) {
        return None;
    }
    let text = std::fs::read_to_string(Path::new(THEMES).join(theme).join(name)).ok()?;
    Some(text.lines().next()?.trim().to_string()).filter(|line| !line.is_empty())
}

/// "#rrggbb" as cairo's red, green and blue
fn rgb(hex: &str) -> (f64, f64, f64) {
    let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0;
    (channel(1), channel(3), channel(5))
}

fn mix(a: (f64, f64, f64), b: (f64, f64, f64), amount: f64) -> (f64, f64, f64) {
    (a.0 + (b.0 - a.0) * amount, a.1 + (b.1 - a.1) * amount, a.2 + (b.2 - a.2) * amount)
}

/// What the preview shows
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub colors: BTreeMap<String, String>,
    pub style: String,
    pub dock: String,
    pub pointer: String,
}

impl Look {
    fn color(&self, key: &str, fallback: &str) -> (f64, f64, f64) {
        rgb(self.colors.get(key).map(String::as_str).unwrap_or(fallback))
    }
}

/// The radii a style gives a card and the top bar, from the current theme's shapes.conf (lib.sh's STYLE_SHAPE, which
/// render_templates writes for every style); floating, which has none, as the board draws it
fn radii(style: &str) -> (f64, f64) {
    let file = palette::current().join("theme").join("shapes.conf");
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let value = |token: &str| -> Option<f64> {
        let wanted = format!("{style}.{token}");
        text.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            if key.trim() == wanted { value.trim().parse().ok() } else { None }
        })
    };
    (value("card").unwrap_or(10.0), value("bar").unwrap_or(8.0))
}

/// The dock's five squares, as each look draws them: sizes and how far each is raised
fn dock_shape(dock: &str) -> [(f64, f64); 5] {
    match dock {
        "swell" => [(16.0, 0.0), (20.0, 0.0), (26.0, 0.0), (20.0, 0.0), (16.0, 0.0)],
        "lift" => [(16.0, 0.0), (16.0, 1.0), (20.0, 7.0), (16.0, 1.0), (16.0, 0.0)],
        _ => [(16.0, 0.0); 5],
    }
}

fn rounded(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let pi = std::f64::consts::PI;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -pi / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
    cr.arc(x + r, y + h - r, r, pi / 2.0, pi);
    cr.arc(x + r, y + r, r, pi, 1.5 * pi);
    cr.close_path();
}

fn fill_stroke(cr: &cairo::Context, fill: (f64, f64, f64, f64), line: (f64, f64, f64, f64)) {
    cr.set_source_rgba(fill.0, fill.1, fill.2, fill.3);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(line.0, line.1, line.2, line.3);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

/// The preview, in the board's own measures from its top left corner
fn draw(cr: &cairo::Context, width: f64, height: f64, look: &Look, dock: &[(f64, f64); 5], continue_text: &str) {
    let bg = look.color("background", "#110c18");
    let fg = look.color("foreground", "#efe4ff");
    let accent = look.color("accent", "#9148f0");
    let with = |c: (f64, f64, f64), a: f64| (c.0, c.1, c.2, a);
    rounded(cr, 0.0, 0.0, width, height, 14.0);
    cr.clip();

    // The background, with a highlight of the accent at the top
    cr.set_source_rgb(bg.0, bg.1, bg.2);
    let _ = cr.paint();
    let glow = mix(bg, accent, 0.18);
    let _ = cr.save();
    cr.translate(width * 0.7, 0.0);
    cr.scale(260.0, 160.0);
    let gradient = cairo::RadialGradient::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    gradient.add_color_stop_rgb(0.0, glow.0, glow.1, glow.2);
    gradient.add_color_stop_rgb(1.0, bg.0, bg.1, bg.2);
    let _ = cr.set_source(&gradient);
    cr.rectangle(-1.0, -1.0, 2.0, 2.0);
    let _ = cr.fill();
    let _ = cr.restore();

    // The top bar, as the style shapes it
    let (card_radius, bar_radius) = radii(&look.style);
    let pane = with(fg, 0.12);
    let rim = with(fg, 0.18);
    match look.style.as_str() {
        "islands" => {
            for (x, w) in [(8.0, 60.0), ((width - 70.0) / 2.0, 70.0), (width - 8.0 - 50.0, 50.0)] {
                rounded(cr, x, 8.0, w, 18.0, 9.0);
                fill_stroke(cr, pane, rim);
            }
        }
        "capsule" => {
            rounded(cr, 70.0, 8.0, width - 140.0, 18.0, 9.0);
            fill_stroke(cr, pane, rim);
        }
        style => {
            let radius = if style == "floating" { 8.0 } else { bar_radius.min(9.0) };
            rounded(cr, 8.0, 8.0, width - 16.0, 18.0, radius);
            fill_stroke(cr, pane, rim);
        }
    }

    // A window with a title strip, two lines of text and a button
    let radius = card_radius.min(16.0);
    rounded(cr, 30.0, 40.0, 200.0, 120.0, radius);
    fill_stroke(cr, (mix(bg, fg, 0.08).0, mix(bg, fg, 0.08).1, mix(bg, fg, 0.08).2, 1.0), with(fg, 0.12));
    let dots = [look.color("warning", look.colors.get("color3").map(String::as_str).unwrap_or("#f4b740")),
        look.color("success", look.colors.get("color2").map(String::as_str).unwrap_or("#3fca7e")),
        look.color("error", look.colors.get("color1").map(String::as_str).unwrap_or("#f2566a"))];
    for (i, dot) in dots.iter().enumerate() {
        let x = 30.0 + 200.0 - 7.0 - 3.5 - (2 - i) as f64 * 11.0;
        cr.arc(x, 40.0 + 9.0, 3.5, 0.0, 2.0 * std::f64::consts::PI);
        cr.set_source_rgb(dot.0, dot.1, dot.2);
        let _ = cr.fill();
    }
    for (y, w) in [(40.0 + 26.0, 90.0), (40.0 + 40.0, 120.0)] {
        rounded(cr, 42.0, y, w, 6.0, 3.0);
        cr.set_source_rgba(fg.0, fg.1, fg.2, 0.35);
        let _ = cr.fill();
    }
    cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(10.5);
    let extents = cr.text_extents(continue_text).ok();
    let text_width = extents.map(|e| e.x_advance()).unwrap_or(44.0);
    let (bx, by, bh) = (42.0, 40.0 + 58.0, 21.0);
    rounded(cr, bx, by, text_width + 20.0, bh, if look.style == "capsule" { 999.0 } else { 7.0 });
    cr.set_source_rgb(accent.0, accent.1, accent.2);
    let _ = cr.fill();
    cr.set_source_rgb(bg.0, bg.1, bg.2);
    cr.move_to(bx + 10.0, by + 14.5);
    let _ = cr.show_text(continue_text);

    // The dock, centred at the bottom, its squares bottom-aligned
    let colors = [accent, look.color("color6", "#6fd3e0"), look.color("color3", "#e8c06a"),
        look.color("color2", "#8ddba4"), look.color("color1", "#ef6b7d")];
    let inner: f64 = dock.iter().map(|(size, _)| size).sum::<f64>() + 4.0 * 5.0;
    let dock_width = inner + 16.0;
    let dx = (width - dock_width) / 2.0;
    let dy = height - 8.0 - 30.0;
    rounded(cr, dx, dy, dock_width, 30.0, 12.0);
    fill_stroke(cr, pane, rim);
    let mut x = dx + 8.0;
    for ((size, lift), color) in dock.iter().zip(colors) {
        rounded(cr, x, dy + 30.0 - 5.0 - size - lift, *size, *size, 5.0);
        cr.set_source_rgb(color.0, color.1, color.2);
        let _ = cr.fill();
        x += size + 5.0;
    }

    // The pointer: the arrow of the board's 24 by 30 drawing at 20 by 25
    let (fill, rim_color) = match look.pointer.as_str() {
        "ink" => (rgb("#15101c"), (1.0, 1.0, 1.0)),
        "facet" => (accent, (1.0, 1.0, 1.0)),
        _ => ((1.0, 1.0, 1.0), accent),
    };
    let arrow = |cr: &cairo::Context, ox: f64, oy: f64| {
        let k = 20.0 / 24.0;
        cr.move_to(ox + 4.0 * k, oy + 3.0 * k);
        cr.line_to(ox + 20.0 * k, oy + 16.0 * k);
        cr.line_to(ox + 12.6 * k, oy + 17.3 * k);
        cr.line_to(ox + 8.7 * k, oy + 24.0 * k);
        cr.close_path();
    };
    arrow(cr, 214.0, 114.0);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    let _ = cr.fill();
    arrow(cr, 214.0, 112.0);
    cr.set_source_rgb(fill.0, fill.1, fill.2);
    let _ = cr.fill_preserve();
    if look.pointer == "facet" {
        // The second face, lighter, from the tip down to the tail
        let _ = cr.save();
        cr.clip_preserve();
        cr.new_path();
        let k = 20.0 / 24.0;
        cr.move_to(214.0 + 4.0 * k, 112.0 + 3.0 * k);
        cr.line_to(214.0 + 20.0 * k, 112.0 + 16.0 * k);
        cr.line_to(214.0 + 12.6 * k, 112.0 + 17.3 * k);
        cr.close_path();
        let light = mix(accent, (1.0, 1.0, 1.0), 0.45);
        cr.set_source_rgb(light.0, light.1, light.2);
        let _ = cr.fill();
        let _ = cr.restore();
        arrow(cr, 214.0, 112.0);
    }
    cr.set_line_width(1.5);
    cr.set_line_join(cairo::LineJoin::Round);
    cr.set_source_rgb(rim_color.0, rim_color.1, rim_color.2);
    let _ = cr.stroke();
}

/// The preview's drawing, kept from one rendering of the page to the next so that its dock can ease into a new look
pub struct Preview {
    pub area: gtk::DrawingArea,
    look: RefCell<Option<Look>>,
    dock: RefCell<[(f64, f64); 5]>,
    from: RefCell<[(f64, f64); 5]>,
    start: Cell<i64>,
    tick: RefCell<Option<gtk::TickCallbackId>>,
}

impl Preview {
    fn new(continue_text: String) -> Rc<Preview> {
        let area = gtk::DrawingArea::builder().hexpand(true).vexpand(false).valign(gtk::Align::Start).content_height(252).content_width(330).build();
        area.add_css_class("welcome-preview");
        area.set_accessible_role(gtk::AccessibleRole::Img);
        area.update_property(&[gtk::accessible::Property::Label(&t!("Live preview of the chosen look"))]);
        let preview = Rc::new(Preview {
            area,
            look: RefCell::new(None),
            dock: RefCell::new(dock_shape("off")),
            from: RefCell::new(dock_shape("off")),
            start: Cell::new(0),
            tick: RefCell::new(None),
        });
        let weak = Rc::downgrade(&preview);
        preview.area.set_draw_func(move |_, cr, width, height| {
            let Some(preview) = weak.upgrade() else { return };
            if let Some(look) = preview.look.borrow().as_ref() {
                draw(cr, width as f64, height as f64, look, &preview.dock.borrow(), &continue_text);
            }
        });
        preview
    }

    /// A new look: drawn once, and the dock eased into its new shape over DOCK_MS when animations are on
    fn show(self: &Rc<Self>, look: Look) {
        let target = dock_shape(&look.dock);
        let first = self.look.borrow().is_none();
        if self.look.borrow().as_ref() == Some(&look) {
            return;
        }
        let animations = ui::animations_on();
        self.look.replace(Some(look));
        if first || !animations || *self.dock.borrow() == target {
            self.dock.replace(target);
            self.area.queue_draw();
            return;
        }
        self.from.replace(*self.dock.borrow());
        self.start.set(0);
        if let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
        }
        let weak = Rc::downgrade(self);
        let id = self.area.add_tick_callback(move |_, clock| {
            let Some(preview) = weak.upgrade() else { return glib::ControlFlow::Break };
            let now = clock.frame_time();
            if preview.start.get() == 0 {
                preview.start.set(now);
            }
            let t = ((now - preview.start.get()) as f64 / 1000.0 / DOCK_MS).min(1.0);
            // ease: the board's own curve, close enough at this size
            let eased = t * t * (3.0 - 2.0 * t);
            let from = *preview.from.borrow();
            let mut dock = [(0.0, 0.0); 5];
            for (i, slot) in dock.iter_mut().enumerate() {
                *slot = (from[i].0 + (target[i].0 - from[i].0) * eased, from[i].1 + (target[i].1 - from[i].1) * eased);
            }
            preview.dock.replace(dock);
            preview.area.queue_draw();
            if t >= 1.0 {
                preview.tick.replace(None);
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
        self.tick.replace(Some(id));
    }
}

/// Where the page says it was shown: the layout offer reads it, and gives way to the page's own Windows row
fn first_look_file() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| palette::home().join(".local/state"));
    state.join("amethystora").join("first-look.json")
}

/// Written whole under another name and moved into place, with a copy beside it; once
fn record_shown() {
    let file = first_look_file();
    if file.exists() {
        return;
    }
    let Some(folder) = file.parent() else { return };
    if std::fs::create_dir_all(folder).is_err() {
        return;
    }
    let shown = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let text = format!("{{\"version\": 1, \"shown\": {shown}}}\n");
    for path in [file.with_extension("json.bak"), file.clone()] {
        let next = path.with_extension("new");
        if std::fs::write(&next, &text).is_ok() {
            let _ = std::fs::rename(&next, &path);
        }
    }
}

impl Control {
    /// What a row has now: what was picked here, or what the list said
    fn chosen(&self, item: &Item) -> String {
        self.state.borrow().picked.get(&item.id).cloned().or_else(|| item.state.clone()).unwrap_or_default()
    }

    fn look(&self, items: &BTreeMap<&str, &Item>) -> Look {
        let pick = |id: &str| items.get(id).map(|item| self.chosen(item)).unwrap_or_default();
        let theme = pick("theme");
        // The theme's own colours, from the image's themes; one the account made itself has the list's
        let colors = Some(Path::new(THEMES).join(&theme))
            .filter(|_| theme_id(&theme))
            .and_then(|dir| palette::read_theme(&dir))
            .map(|palette| palette.colors)
            .or_else(|| {
                let item = items.get("theme")?;
                item.choices.iter().find(|choice| choice.value == theme).map(|choice| choice.colors.clone())
            })
            .unwrap_or_default();
        let mut style = pick("style");
        if style.is_empty() || style == "theme" {
            style = theme_file(&theme, "style.theme").unwrap_or_else(|| "floating".into());
        }
        let mut pointer = pick("pointer");
        if pointer.is_empty() || pointer == "theme" {
            let own = theme_file(&theme, "cursor.theme").unwrap_or_default();
            pointer = ["ink", "facet"].into_iter().find(|name| own.ends_with(name)).unwrap_or("halo").into();
        }
        Look { colors, style, dock: pick("dock-magnify"), pointer }
    }

    fn segment(&self, item: &Item, value: &str, title: &str, chosen: bool, dot: Option<String>) -> gtk::Button {
        let content = ui::hbox(6, &[]);
        if let Some(color) = dot {
            let swatch = gtk::DrawingArea::builder().content_width(12).content_height(12).valign(gtk::Align::Center).build();
            swatch.add_css_class("welcome-dot");
            swatch.set_draw_func(move |area, cr, _, _| {
                let (r, g, b) = rgb(&color);
                let ring = area.color();
                cr.arc(6.0, 6.0, 6.0, 0.0, 2.0 * std::f64::consts::PI);
                cr.set_source_rgb(r, g, b);
                let _ = cr.fill();
                cr.arc(6.0, 6.0, 5.5, 0.0, 2.0 * std::f64::consts::PI);
                cr.set_source_rgba(ring.red() as f64, ring.green() as f64, ring.blue() as f64, 0.3);
                cr.set_line_width(1.0);
                let _ = cr.stroke();
            });
            content.append(&swatch);
        }
        content.append(&ui::line(title, &[]));
        let button = gtk::Button::builder().child(&content).css_classes(["seg"]).build();
        if chosen {
            button.add_css_class("chosen");
        }
        button.update_state(&[gtk::accessible::State::Pressed(if chosen {
            gtk::AccessibleTristate::True
        } else {
            gtk::AccessibleTristate::False
        })]);
        button.set_sensitive(item.available && !self.busy(&item.id));
        let (id, value) = (item.id.clone(), value.to_string());
        let me = self.me.clone();
        button.connect_clicked(move |_| {
            let Some(this) = me.upgrade() else { return };
            this.state.borrow_mut().picked.insert(id.clone(), value.clone());
            this.change(&id, &value);
        });
        button
    }

    /// Have a backup? A card above the choices: Start fresh puts it away, Bring it all back runs the list's entry in a terminal
    fn backup_card(&self) -> gtk::Widget {
        let card = ui::vbox(8, &["welcome-card", "welcome-backup"]);
        card.append(&ui::label(&t!("Have a backup?").to_uppercase(), &["welcome-label"]));
        card.append(&ui::label(&t!("New computer, or the old one was lost. Plug in your backup disk, and have your recovery key ready."), &["lead"]));
        let buttons = ui::hbox(8, &["welcome-backup-buttons"]);
        let hide = card.clone();
        buttons.append(&ui::button(&t!("Start fresh"), None, &["seg"], move || {
            FRESH.with(|fresh| fresh.set(true));
            hide.set_visible(false);
        }));
        let me = self.me.clone();
        buttons.append(&ui::button(&t!("Bring it all back"), None, &["seg", "chosen"], move || {
            if let Some(this) = me.upgrade() {
                this.act(BACKUP_ENTRY);
            }
        }));
        card.append(&buttons);
        card.upcast()
    }

    /// The page itself, drawn again whenever the list comes back
    pub(crate) fn welcome_view(&self) -> gtk::Widget {
        let list = self.state.borrow().list.clone();
        let items: BTreeMap<&str, &Item> = list
            .as_deref()
            .map(|list| list.items.iter().filter(|item| ROWS.contains(&item.id.as_str()) && item.available).map(|item| (item.id.as_str(), item)).collect())
            .unwrap_or_default();

        let view = ui::vbox(20, &["page", "welcome"]);
        let head = ui::vbox(6, &["page-head"]);
        head.append(&heading(&t!("Pick a look"), "h1"));
        head.append(&ui::label(&t!("Change any of this later in Control."), &["lead"]));
        view.append(&head);

        // Have a backup? (R7-55): the list's own entry, run as its card runs it, in a terminal, where the recovery key is typed
        if !FRESH.with(Cell::get) && list.as_deref().is_some_and(|list| list.items.iter().any(|item| item.id == BACKUP_ENTRY && item.available)) {
            view.append(&self.backup_card());
        }

        let card = ui::hbox(16, &["welcome-card"]);
        let rows = ui::vbox(10, &["welcome-rows"]);
        rows.set_size_request(290, -1);
        for id in ROWS {
            let Some(item) = items.get(id) else { continue };
            let row = ui::vbox(6, &["welcome-row"]);
            let label = ui::label(&row_title(id).to_uppercase(), &["welcome-label"]);
            row.append(&label);
            let segments = adw::WrapBox::builder().child_spacing(6).line_spacing(6).build();
            let chosen = self.chosen(item);
            let mut choices: Vec<_> = item.choices.iter().collect();
            // Still first, as the dock grows more from left to right
            if id == "dock-magnify" {
                choices.sort_by_key(|choice| choice.value != "off");
            }
            for choice in choices {
                let dot = (id == "theme").then(|| choice.colors.get("accent").cloned()).flatten();
                segments.append(&self.segment(item, &choice.value, &choice.title, choice.value == chosen, dot));
            }
            row.append(&segments);
            if let Some(said) = self.said_box(item) {
                row.append(&said);
            }
            rows.append(&row);
        }
        // As wide as the board has it, however wide the window: the preview takes the rest
        card.append(&ui::limit(&rows, 290));

        let preview = self.welcome_preview.borrow_mut().get_or_insert_with(|| Preview::new(t!("Continue"))).clone();
        if let Some(parent) = preview.area.parent() {
            if let Ok(parent) = parent.downcast::<gtk::Box>() {
                parent.remove(&preview.area);
            }
        }
        preview.show(self.look(&items));
        card.append(&preview.area);
        view.append(&card);

        let foot = ui::hbox(0, &["welcome-foot"]);
        foot.set_halign(gtk::Align::End);
        let window = self.shell.window.clone();
        foot.append(&ui::button(&t!("Done"), None, &["welcome-done"], move || window.close()));
        view.append(&foot);

        // Shown: the layout offer gives way to the Windows row from now on
        view.connect_map(|_| record_shown());
        adw::Clamp::builder().maximum_size(920).tightening_threshold(920).child(&view).build().upcast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_ids_are_folder_names() {
        assert!(theme_id("tokyo-night"));
        assert!(!theme_id("../etc"));
        assert!(!theme_id("Nord"));
        assert!(!theme_id(""));
    }

    #[test]
    fn docks_by_look() {
        assert_eq!(dock_shape("swell").iter().map(|d| d.0).collect::<Vec<_>>(), [16.0, 20.0, 26.0, 20.0, 16.0]);
        assert_eq!(dock_shape("lift")[2], (20.0, 7.0));
        assert_eq!(dock_shape("off"), [(16.0, 0.0); 5]);
    }

    #[test]
    fn colours() {
        assert_eq!(rgb("#ff0000"), (1.0, 0.0, 0.0));
        assert_eq!(mix((0.0, 0.0, 0.0), (1.0, 1.0, 1.0), 0.18), (0.18, 0.18, 0.18));
    }
}
