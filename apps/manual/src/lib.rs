//! The Amethystora Manual: the Markdown pages in /usr/share/amethystora/manual, in a window of their own.
//!
//! The window only ever shows the manual. It reads nothing but those pages and pages.json, which lists and
//! groups them, and the palette of the current theme; every web link opens in the browser.
//!
//! A page translated into the session's language is manual/<language>/<page>.md, which Weblate writes from
//! the English one paragraph by paragraph; a page not translated yet is shown in English. A translation's
//! sections keep the ids of the English ones, which links between pages and from the other apps go by:
//! heading for heading, as long as it has as many as the English. While it has not, nothing says which of its
//! headings is which, so a link to a section it cannot find opens the English page at that section, and says so.
//!
//! A page is addressed by its file name: `updates`, or `updates/rolling-back` for a section, which is what
//! `amethystora-manual updates#rolling-back` opens.

mod cards;
mod columns;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::frame;
use amethystora_ui::i18n;
use amethystora_ui::markdown::{self, Block, Section};
use amethystora_ui::{adw, gdk, gio, glib, gtk, process, store, t, ui};
use adw::prelude::*;
use serde_json::Value;


const SHARE: &str = "/usr/share/amethystora";

fn manual_dir() -> PathBuf {
    Path::new(SHARE).join("manual")
}

pub fn main() -> glib::ExitCode {
    app::run(
        Spec {
            name: "manual",
            id: "org.amethystora.Manual",
            css: include_str!("manual.css"),
            width: 1180,
            height: 820,
            min_width: 560,
            min_height: 420,
        },
        Manual::new,
    )
}

/// One page of the manual, read and indexed once at start
struct Page {
    /// Its file name without .md, which routes go by
    id: String,
    /// The name of its group in pages.json, in the session's language
    section: String,
    markdown: String,
    title: String,
    /// The ids of its sections, the English page's when this is a translation with as many of them
    ids: Vec<String>,
    /// What search looks through: the page split at its headings
    sections: Vec<Section>,
    /// The English page, kept while a translation's headings do not line up with it
    english: Option<English>,
    /// Not translated into the session's language
    in_english: bool,
}

struct English {
    markdown: String,
    ids: Vec<String>,
}

impl Page {
    /// Whether a section only the English page can show is asked for
    fn english_for(&self, anchor: &str) -> bool {
        let has = |ids: &[String]| ids.iter().any(|id| id == anchor);
        !has(&self.ids) && self.english.as_ref().is_some_and(|english| has(&english.ids))
    }
}

/// The margins of the page's blocks, as manual.css set them (margin collapsing is markdown.rs's)
fn margins(block: Block) -> (i32, i32) {
    match block {
        Block::H1 => (0, 18),
        Block::Lead | Block::P | Block::List => (0, 16),
        Block::H2 => (46, 12),
        Block::H3 => (30, 8),
        // A browser's own for h4: 1.33 em of the 16 px text
        Block::H4 => (21, 21),
        Block::Item => (4, 4),
        Block::Code | Block::Quote => (0, 18),
        Block::Table => (0, 20),
        Block::Hr => (36, 36),
        Block::Image => (0, 16),
    }
}

/// How much of a page is drawn at once, and how much each frame after that, or while on the way to a section
const BLOCKS_AT_ONCE: usize = 12;
const BLOCKS_PER_FRAME: usize = 6;
const BLOCKS_HURRYING: usize = 40;
const BLOCKS_A_SCREEN: usize = 30;

/// A hit of the search: where it goes, what it is called and the words around the first one searched for
struct Hit {
    route: String,
    title: String,
    text: String,
}

pub struct Manual {
    me: Weak<Manual>,
    shell: Rc<Shell>,
    pages: Vec<Page>,
    shown: Cell<Option<usize>>,
    /// The shown page is the English one, for a section its translation cannot show
    in_english: Cell<bool>,
    /// Says so, over the page
    note: gtk::Box,
    /// Show me could not start: said over the page, since the Amethystora windows have no title bar to put it in
    show_me_note: gtk::Box,
    /// The routes visited, and where in them the window is, for Alt+Left and Alt+Right
    history: RefCell<Vec<String>>,
    at: Cell<usize>,
    toc: gtk::Box,
    toc_links: RefCell<Vec<(String, gtk::Button)>>,
    toc_scroller: gtk::ScrolledWindow,
    results: gtk::Box,
    results_scroller: gtk::ScrolledWindow,
    hits: RefCell<Vec<(String, gtk::Button)>>,
    selected: Cell<usize>,
    search: gtk::Entry,
    search_pause: ui::Pause,
    /// The page while it fills in, which turn of filling is the current one, and a section to go to once it is there
    filling: RefCell<Option<markdown::Filling>>,
    fill_turn: Cell<u64>,
    wanted: RefCell<Option<(String, bool)>>,
    article: gtk::Box,
    pager: gtk::Box,
    main: gtk::ScrolledWindow,
    headings: RefCell<Vec<(String, gtk::Widget)>>,
    /// Styled as the + between keys is, and never shown: its colour is read for the markup
    soft_probe: gtk::Label,
    scrolling: RefCell<Option<adw::TimedAnimation>>,
}

impl App for Manual {
    fn open(&self, args: &[String]) {
        // The page to open, `amethystora-manual keybindings` or `amethystora-manual updates#rollback`, as the
        // route the window uses
        let page = args.iter().find(|arg| !arg.starts_with('-')).map(|arg| arg.replacen('#', "/", 1)).unwrap_or_default();
        let first = self.shown.get().is_none();
        if !page.is_empty() || first {
            self.navigate(&page);
        }
        if first {
            // The page has the focus, as the Electron page had: arrows and Page Down scroll it
            self.main.set_focusable(true);
            let main = self.main.clone();
            glib::idle_add_local_once(move || {
                main.grab_focus();
            });
        }
    }

    fn reload(&self) {}

    fn zoomed(&self) {
        self.palette();
    }

    /// The theme changed: the page is drawn again, for the colours its keys are written in
    fn palette(&self) {
        if let Some(index) = self.shown.get() {
            let at = self.main.vadjustment().value();
            self.render(index);
            let main = self.main.clone();
            glib::idle_add_local_once(move || main.vadjustment().set_value(at));
        }
    }
}

/// The pages pages.json lists, in the session's language when they have been translated
fn read_pages() -> Vec<Page> {
    let lang = i18n::with(|i18n| i18n.lang.clone());
    let toc: Value = std::fs::read_to_string(manual_dir().join("pages.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Array(Vec::new()));
    let mut pages = Vec::new();
    for group in toc.as_array().into_iter().flatten() {
        let section = i18n::dynamic(group.get("section").and_then(Value::as_str).unwrap_or(""));
        for file in group.get("pages").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            // Nothing outside /usr/share/amethystora is read, and only .md files: the hotkeys page is
            // ../keybindings.md, beside the manual, where `ame desktop keybindings` reads it too
            let Some(full) = inside_share(&manual_dir().join(file)) else { continue };
            if full.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(english) = std::fs::read_to_string(&full) else { continue };
            let id = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let translated = if lang == "en" {
                None
            } else {
                full.file_name().and_then(|name| std::fs::read_to_string(manual_dir().join(&lang).join(name)).ok())
            };
            let markdown = translated.clone().unwrap_or_else(|| english.clone());
            let title = markdown
                .lines()
                .find_map(|line| line.strip_prefix("# ").map(|t| t.trim().to_string()))
                .unwrap_or_else(|| id.clone());
            // A page not translated is still named in the session's language, where the catalogs have its title
            let title = if translated.is_some() { title } else { i18n::dynamic(&title) };
            let own: Vec<String> = markdown::heading_texts(&markdown).iter().map(|h| markdown::slug(h)).collect();
            let (ids, english) = match &translated {
                Some(_) => {
                    let english_ids: Vec<String> = markdown::heading_texts(&english).iter().map(|h| markdown::slug(h)).collect();
                    if english_ids.len() == own.len() {
                        (english_ids, None)
                    } else {
                        (own, Some(English { markdown: english, ids: english_ids }))
                    }
                }
                None => (own, None),
            };
            let sections = markdown::sections(&markdown, &title, &ids);
            let in_english = translated.is_none();
            pages.push(Page { id, section: section.clone(), markdown, title, ids, sections, english, in_english });
        }
    }
    pages
}

/// The path with its . and .. worked out, if it is still under /usr/share/amethystora
fn inside_share(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out.starts_with(SHARE).then_some(out).filter(|p| p != Path::new(SHARE))
}

/// A guide's id, as guides.js writes them
fn valid_guide(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && !id.starts_with('-') && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// "#updates/rolling-back" for a link written `updates.md#rolling-back`, within the page `#page/x` for `#x`,
/// and a web address as it is
fn route(href: &str, page: &str) -> String {
    let lower = href.to_lowercase();
    if lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("mailto:") || lower.starts_with("guide:") {
        return href.to_string();
    }
    if let Some(local) = href.strip_prefix('#').filter(|l| !l.is_empty()) {
        return format!("#{page}/{local}");
    }
    let (path, anchor) = match href.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (href, None),
    };
    if let Some(name) = path.rsplit('/').next().and_then(|name| name.strip_suffix(".md")) {
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return match anchor.filter(|a| !a.is_empty()) {
                Some(anchor) => format!("#{name}/{anchor}"),
                None => format!("#{name}"),
            };
        }
    }
    href.to_string()
}

impl Manual {
    fn new(shell: Rc<Shell>) -> Rc<Manual> {
        let pages = read_pages();
        let search = gtk::Entry::builder().placeholder_text(t!("Search the manual")).css_classes(["search-input"]).build();
        search.update_property(&[gtk::accessible::Property::Label(&t!("Search the manual"))]);
        let toc = ui::vbox(0, &["toc"]);
        let results = ui::vbox(0, &["results"]);
        let toc_scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&toc).build();
        let results_scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&results).visible(false).build();
        let article = ui::vbox(0, &["page"]);
        let pager = ui::hbox(16, &["pager"]);
        pager.set_homogeneous(true);
        let column = ui::vbox(0, &["column"]);
        column.append(&article);
        column.append(&pager);
        let clamp = adw::Clamp::builder().maximum_size(780).tightening_threshold(780).child(&column).build();
        let main = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
        main.add_css_class("main");
        let this = Rc::new_cyclic(|me| Manual {
            me: me.clone(),
            shell: shell.clone(),
            pages,
            shown: Cell::new(None),
            in_english: Cell::new(false),
            note: ui::vbox(4, &["not-translated"]),
            show_me_note: ui::vbox(4, &["not-translated"]),
            history: RefCell::new(Vec::new()),
            at: Cell::new(0),
            toc,
            toc_links: RefCell::new(Vec::new()),
            toc_scroller,
            results,
            results_scroller,
            hits: RefCell::new(Vec::new()),
            search_pause: ui::Pause::default(),
            filling: RefCell::new(None),
            fill_turn: Cell::new(0),
            wanted: RefCell::new(None),
            selected: Cell::new(0),
            search,
            article,
            pager,
            main,
            headings: RefCell::new(Vec::new()),
            soft_probe: ui::line("", &["probe-soft"]),
            scrolling: RefCell::new(None),
        });
        shell.window.set_title(Some(&t!("Amethystora Manual")));
        shell.window.set_content(Some(&this.frame()));
        shell.narrow(860.0, "narrow");
        this.build_toc();
        this.keys();
        this
    }

    fn rc(&self) -> Rc<Manual> {
        self.me.upgrade().expect("the window is open")
    }

    fn frame(&self) -> gtk::Widget {
        let sidebar = ui::vbox(0, &["sidebar"]);
        // The wordmark of the boot splash and the login screen, the glowing stone as its "a": back to the
        // top of the page, as its link to # did
        let brand = amethystora_ui::brand::brand(&t!("Manual"), 21.0);
        let home = gtk::Button::builder().child(&brand).css_classes(["plain", "brand-button"]).halign(gtk::Align::Start).build();
        home.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Manual"))]);
        let weak = self.me.clone();
        home.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.navigate("");
            }
        });
        sidebar.append(&home);
        // The search box, with Ctrl K inside its end
        let overlay = gtk::Overlay::new();
        overlay.add_css_class("search");
        overlay.set_child(Some(&self.search));
        let hint = ui::hbox(3, &["hint"]);
        hint.set_direction(gtk::TextDirection::Ltr);
        hint.set_halign(gtk::Align::End);
        hint.set_valign(gtk::Align::Center);
        hint.set_can_target(false);
        for key in ["Ctrl", "K"] {
            hint.append(&ui::line(key, &["kbd"]));
        }
        overlay.add_overlay(&hint);
        sidebar.append(&overlay);
        self.toc.update_property(&[gtk::accessible::Property::Label(&t!("Pages"))]);
        self.results.update_property(&[gtk::accessible::Property::Label(&t!("Search results"))]);
        sidebar.append(&self.toc_scroller);
        sidebar.append(&self.results_scroller);
        self.soft_probe.set_visible(false);
        sidebar.append(&self.soft_probe);
        self.pager.update_property(&[gtk::accessible::Property::Label(&t!("Previous and next page"))]);

        let weak = self.me.clone();
        self.search.connect_changed(move |search| {
            let Some(this) = weak.upgrade() else { return };
            if search.text().trim().is_empty() {
                this.search_pause.cancel();
                this.show_results();
            } else {
                let again = this.clone();
                this.search_pause.after_typing(move || again.show_results());
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            match key {
                gdk::Key::Down | gdk::Key::Up => {
                    this.flush_search();
                    this.move_selection(if key == gdk::Key::Down { 1 } else { -1 });
                    glib::Propagation::Stop
                }
                gdk::Key::Escape => {
                    this.clear_search();
                    this.main.grab_focus();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.search.add_controller(keys);
        let weak = self.me.clone();
        self.search.connect_activate(move |_| {
            let Some(this) = weak.upgrade() else { return };
            this.flush_search();
            let hit = this.hits.borrow().get(this.selected.get()).map(|(route, _)| route.clone());
            if let Some(route) = hit {
                this.navigate(&route);
                this.clear_search();
            }
        });
        self.note.set_halign(gtk::Align::Center);
        self.note.set_valign(gtk::Align::End);
        self.note.set_visible(false);
        self.note.append(&ui::label(&t!("This section is not in the translation yet, so the page is shown in English."), &[]));
        let weak = self.me.clone();
        let back = ui::button(&t!("Show the translated page"), None, &["plain", "not-translated-back"], move || {
            if let Some(this) = weak.upgrade() {
                if let Some(page) = this.shown.get().and_then(|index| this.pages.get(index)) {
                    this.navigate(&page.id);
                }
            }
        });
        back.set_halign(gtk::Align::Start);
        self.note.append(&back);
        self.show_me_note.set_halign(gtk::Align::Center);
        self.show_me_note.set_valign(gtk::Align::Start);
        self.show_me_note.set_visible(false);
        self.show_me_note.append(&ui::label(&t!("Show me is off here: ame desktop show-me on"), &[]));
        let weak = self.me.clone();
        let close = ui::button(&t!("Close"), None, &["plain", "not-translated-back"], move || {
            if let Some(this) = weak.upgrade() {
                this.show_me_note.set_visible(false);
            }
        });
        close.set_halign(gtk::Align::Start);
        self.show_me_note.append(&close);
        let page_side = gtk::Overlay::builder().child(&self.main).build();
        page_side.add_overlay(&self.note);
        page_side.add_overlay(&self.show_me_note);
        let layout = columns::Columns::new(&sidebar, &page_side);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&layout));
        let controls = frame::controls();
        controls.add_css_class("floating");
        controls.set_halign(gtk::Align::End);
        controls.set_valign(gtk::Align::Start);
        overlay.add_overlay(&controls);
        gtk::WindowHandle::builder().child(&overlay).build().upcast()
    }

    /// Ctrl+K, Ctrl+F and / to search; Alt+Left and Alt+Right through the pages visited
    fn keys(&self) {
        let keys = gtk::ShortcutController::new();
        keys.set_scope(gtk::ShortcutScope::Global);
        let weak = self.me.clone();
        let focus = gtk::CallbackAction::new(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.search.grab_focus();
                this.search.select_region(0, -1);
            }
            glib::Propagation::Stop
        });
        for trigger in ["<Control>k", "<Control>f"] {
            keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger), Some(focus.clone())));
        }
        for (trigger, step) in [("<Alt>Left", -1i32), ("<Alt>Right", 1)] {
            let weak = self.me.clone();
            let action = gtk::CallbackAction::new(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    this.go(step);
                }
                glib::Propagation::Stop
            });
            keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger), Some(action)));
        }
        self.shell.window.add_controller(keys);
        // / jumps to the search box, unless it is being typed into
        let slash = gtk::EventControllerKey::new();
        slash.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.me.clone();
        slash.connect_key_pressed(move |_, key, _, _| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            if key == gdk::Key::slash {
                let focus = gtk::prelude::RootExt::focus(&this.shell.window);
                let typing = focus.is_some_and(|focus| focus.is_ancestor(&this.search));
                if !typing {
                    this.search.grab_focus();
                    this.search.select_region(0, -1);
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        self.shell.window.add_controller(slash);
    }

    fn build_toc(&self) {
        let mut group: Option<&str> = None;
        let mut links = Vec::new();
        for page in &self.pages {
            if group != Some(page.section.as_str()) {
                group = Some(&page.section);
                // In capitals the language's own way: GTK does not measure the stylesheet's text-transform
                self.toc.append(&ui::label(&page.section.to_uppercase(), &["toc-heading"]));
            }
            let link = gtk::Button::builder().css_classes(["plain", "toc-link"]).build();
            link.set_child(Some(&ui::label(&page.title, &[])));
            let (weak, id) = (self.me.clone(), page.id.clone());
            link.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.navigate(&id);
                }
            });
            self.toc.append(&link);
            links.push((page.id.clone(), link));
        }
        self.toc_links.replace(links);
    }

    fn remembered() -> Option<String> {
        store::load("manual").get("page").and_then(Value::as_str).map(str::to_string)
    }

    /// Goes to a route, `page` or `page/section`, as a click on a link did, and keeps it in the history
    fn navigate(&self, route: &str) {
        let route = route.trim_start_matches('#').to_string();
        {
            let mut history = self.history.borrow_mut();
            let at = self.at.get();
            if history.is_empty() || history.get(at) != Some(&route) {
                history.truncate(at + 1);
                history.push(route.clone());
                self.at.set(history.len() - 1);
            }
        }
        self.show(&route);
    }

    fn go(&self, step: i32) {
        let target = self.at.get() as i32 + step;
        let route = self.history.borrow().get(target.max(0) as usize).cloned();
        if target >= 0 {
            if let Some(route) = route {
                self.at.set(target as usize);
                self.show(&route);
            }
        }
    }

    fn page_index(&self, id: &str) -> Option<usize> {
        self.pages.iter().position(|page| page.id == id)
    }

    fn show(&self, route: &str) {
        let (id, anchor) = match route.split_once('/') {
            Some((id, anchor)) => (id.to_string(), Some(anchor.to_string())),
            None => (route.to_string(), None),
        };
        let Some(index) = self.page_index(&id).or_else(|| Self::remembered().and_then(|r| self.page_index(&r))).or((!self.pages.is_empty()).then_some(0)) else {
            return;
        };
        // Within a page the view glides to the section; a new page opens straight at it
        let in_english = anchor.as_deref().is_some_and(|anchor| self.pages[index].english_for(anchor));
        let same = self.shown.get() == Some(index) && self.in_english.get() == in_english;
        if !same {
            self.shown.set(Some(index));
            self.in_english.set(in_english);
            self.render(index);
        }
        self.wanted.take();
        if let Some(anchor) = anchor.as_ref() {
            // The page is still filling in: it goes there when the section has arrived
            if self.filling.borrow().is_some() && !self.headings.borrow().iter().any(|(id, _)| id == anchor) {
                self.wanted.replace(Some((anchor.clone(), same)));
                return;
            }
        }
        let target = anchor.and_then(|anchor| self.headings.borrow().iter().find(|(id, _)| *id == anchor).map(|(_, widget)| widget.clone()));
        self.land_on(target, same);
    }

    fn land_on(&self, target: Option<gtk::Widget>, same: bool) {
        let this = self.rc();
        // Once the new page has been laid out: a section's place means nothing before that, and an idle callback can come first
        self.main.add_tick_callback(move |_, _| {
            if target.as_ref().is_some_and(|target| target.width() == 0) {
                return glib::ControlFlow::Continue;
            }
            let y = match (&target, content_of(&this.main)) {
                (Some(target), Some(content)) => target.compute_point(&content, &amethystora_ui::graphene::Point::new(0.0, 0.0)).map(|p| (p.y() as f64 - 24.0).max(0.0)),
                _ => Some(0.0),
            };
            if let Some(y) = y {
                this.scroll_to(y, same);
            }
            glib::ControlFlow::Break
        });
    }

    fn scroll_to(&self, y: f64, smooth: bool) {
        let adjustment = self.main.vadjustment();
        if let Some(animation) = self.scrolling.take() {
            animation.pause();
        }
        let animations = ui::animations_on();
        if !smooth || !animations {
            adjustment.set_value(y);
            return;
        }
        let target = adw::CallbackAnimationTarget::new({
            let adjustment = adjustment.clone();
            move |value| adjustment.set_value(value)
        });
        let animation = adw::TimedAnimation::new(&self.main, adjustment.value(), y, 350, target);
        animation.set_easing(adw::Easing::EaseOutCubic);
        animation.play();
        self.scrolling.replace(Some(animation));
    }

    fn render(&self, index: usize) {
        let page = &self.pages[index];
        self.show_me_note.set_visible(false);
        ui::clear(&self.article);
        let eyebrow = ui::label(&page.section.to_uppercase(), &["eyebrow"]);
        self.article.append(&eyebrow);
        let english = page.english.as_ref().filter(|_| self.in_english.get());
        // Over the page rather than at its top, which a link to a section has scrolled away
        self.note.set_visible(english.is_some());
        self.pager.set_margin_bottom(if english.is_some() { 96 } else { 0 });
        let weak = self.me.clone();
        let page_id = page.id.clone();
        let mut options = markdown::Options::new(
            Box::new(move |href| route(href, &page_id)),
            Rc::new(move |uri| {
                if let Some(this) = weak.upgrade() {
                    this.follow(uri);
                }
            }),
            margins,
        );
        options.ids = english.map_or(&page.ids, |english| &english.ids).clone();
        options.keycaps = true;
        options.key_color = hex(&self.article.color());
        options.plus_color = hex(&self.soft_probe.color());
        options.copy = Some((t!("Copy"), t!("Copied")));
        options.anchors = true;
        options.head_capitals = true;
        // code: padding 2px 6px, border-radius 6px
        options.code_pad = 2;
        options.code_radius = 6.0;
        // The moving cards: the manual's own clips only, side by side when a paragraph holds several
        options.image_rows = true;
        options.image = Some(Rc::new(cards::card));
        let mut filling = markdown::Filling::new(english.map_or(&page.markdown, |english| &english.markdown), options);
        self.article.append(&filling.root());
        filling.advance(BLOCKS_AT_ONCE);
        self.headings.replace(filling.headings().to_vec());
        let reads_ltr = english.is_some() || page.in_english;
        self.filling.replace(Some(filling));
        self.pager.set_visible(false);
        self.fill_in(reads_ltr);

        ui::clear(&self.pager);
        let neighbour = |other: Option<&Page>, label: String, class: &str| -> gtk::Widget {
            let Some(other) = other else { return ui::hbox(0, &[]).upcast() };
            let content = ui::vbox(0, &[]);
            content.append(&ui::label(&label, &["pager-label"]));
            content.append(&ui::label(&other.title, &["pager-title"]));
            let link = gtk::Button::builder().child(&content).css_classes(["plain", "pager-link", class]).build();
            let (weak, id) = (self.me.clone(), other.id.clone());
            link.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.navigate(&id);
                }
            });
            if class == "next" {
                for child in [content.first_child(), content.last_child()].into_iter().flatten() {
                    if let Ok(label) = child.downcast::<gtk::Label>() {
                        label.set_xalign(1.0);
                    }
                }
            }
            link.upcast()
        };
        self.pager.append(&neighbour(index.checked_sub(1).and_then(|i| self.pages.get(i)), t!("Previous"), "prev"));
        self.pager.append(&neighbour(self.pages.get(index + 1), t!("Next"), "next"));
        for (id, link) in self.toc_links.borrow().iter() {
            if *id == page.id {
                link.add_css_class("current");
                link.update_state(&[gtk::accessible::State::Selected(Some(true))]);
                let (scroller, link) = (self.toc_scroller.clone(), link.clone());
                glib::idle_add_local_once(move || scroll_into_view(&scroller, &link));
            } else {
                link.remove_css_class("current");
                link.update_state(&[gtk::accessible::State::Selected(Some(false))]);
            }
        }
        self.shell.window.set_title(Some(&t!("{page} · Amethystora Manual", page = &page.title)));
        let mut saved = store::load("manual");
        if let Some(object) = saved.as_object_mut() {
            object.insert("page".into(), Value::String(page.id.clone()));
        }
        store::save("manual", &saved);
    }

    /// The rest of the page, a few blocks each frame: a long one laid out at once stops the window for two seconds. A
    /// link to a section further down hurries it there.
    fn fill_in(&self, reads_ltr: bool) {
        let turn = self.fill_turn.get() + 1;
        self.fill_turn.set(turn);
        let me = self.me.clone();
        self.article.add_tick_callback(move |_, _| {
            let Some(this) = me.upgrade() else { return glib::ControlFlow::Break };
            if this.fill_turn.get() != turn {
                return glib::ControlFlow::Break;
            }
            let hurry = this.wanted.borrow().is_some();
            let most = if hurry { BLOCKS_HURRYING } else { BLOCKS_PER_FRAME };
            let began = Instant::now();
            let mut more = true;
            {
                let mut filling = this.filling.borrow_mut();
                let Some(filling) = filling.as_mut() else { return glib::ControlFlow::Break };
                for _ in 0..most {
                    more = filling.advance(1);
                    if !more || (!hurry && began.elapsed() > Duration::from_millis(5)) {
                        break;
                    }
                }
                this.headings.replace(filling.headings().to_vec());
            }
            let wanted = this.wanted.borrow().clone();
            if let Some((anchor, same)) = wanted {
                let target = this.headings.borrow().iter().find(|(id, _)| *id == anchor).map(|(_, widget)| widget.clone());
                if target.is_some() || !more {
                    // A screenful below it first: a page that ends under the section could not bring it to the top
                    if let Some(filling) = this.filling.borrow_mut().as_mut() {
                        more = filling.advance(BLOCKS_A_SCREEN);
                        this.headings.replace(filling.headings().to_vec());
                    }
                    this.wanted.take();
                    this.land_on(target, same);
                }
            }
            if more {
                return glib::ControlFlow::Continue;
            }
            if let Some(filling) = this.filling.take() {
                let rendered = filling.finish();
                if reads_ltr {
                    left_to_right(rendered.root.upcast_ref());
                }
            }
            this.pager.set_visible(true);
            glib::ControlFlow::Break
        });
    }

    /// A link was followed: within the manual, or a web address in the browser
    fn follow(&self, uri: &str) {
        if let Some(route) = uri.strip_prefix('#') {
            self.navigate(route);
            return;
        }
        let lower = uri.to_lowercase();
        if lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("mailto:") {
            process::open_uri(Some(&self.shell.gtk_window()), uri);
        }
        // Show me (R7-61): only ever a guide's id, which the shell looks up in the image's own guides
        if let Some(id) = uri.strip_prefix("guide:").filter(|id| valid_guide(id)) {
            self.show_me(id.to_string());
        }
    }

    /// The desktop walks the guide, with this window out of the way. With Show me off, or no Amethystora UI, the page's words
    /// are what there is, and a link that does nothing would say less than that
    fn show_me(&self, id: String) {
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let started = match gio::bus_get_future(gio::BusType::Session).await {
                Ok(bus) => bus
                    .call_future(Some("org.gnome.Shell"), "/org/amethystora/Guide", "org.amethystora.Guide", "Start", Some(&(id,).to_variant()), Some(glib::VariantTy::new("(b)").expect("a valid type")), gio::DBusCallFlags::NONE, 5000)
                    .await
                    .ok()
                    .and_then(|answer| answer.get::<(bool,)>())
                    .is_some_and(|(started,)| started),
                Err(_) => false,
            };
            let Some(this) = weak.upgrade() else { return };
            if started {
                this.shell.window.minimize();
            } else {
                this.show_me_note.set_visible(true);
            }
        });
    }

    // --- Search -----------------------------------------------------------------------------------------

    fn find(&self, query: &str) -> Vec<Hit> {
        let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
        let mut found: Vec<(usize, &Page, &Section)> = Vec::new();
        for page in &self.pages {
            let page_title = page.title.to_lowercase();
            for section in &page.sections {
                let title = section.title.to_lowercase();
                let body = section.text.to_lowercase();
                if !words.iter().all(|w| title.contains(w.as_str()) || body.contains(w.as_str()) || page_title.contains(w.as_str())) {
                    continue;
                }
                let score = words.iter().filter(|w| title.contains(w.as_str())).count() * 3 + usize::from(section.id.is_empty());
                found.push((score, page, section));
            }
        }
        // A stable sort, as Array.prototype.sort is
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found
            .into_iter()
            .take(40)
            .map(|(_, page, section)| Hit {
                route: if section.id.is_empty() { page.id.clone() } else { format!("{}/{}", page.id, section.id) },
                title: if section.id.is_empty() { page.title.clone() } else { format!("{} › {}", page.title, section.title) },
                text: snippet(&section.text.split_whitespace().collect::<Vec<_>>().join(" "), words.first().map(String::as_str).unwrap_or("")),
            })
            .collect()
    }

    /// A key that acts on the results acts on what has been typed
    fn flush_search(&self) {
        if self.search_pause.cancel() {
            self.show_results();
        }
    }

    fn show_results(&self) {
        let query = self.search.text().trim().to_string();
        self.toc_scroller.set_visible(query.is_empty());
        self.results_scroller.set_visible(!query.is_empty());
        if query.is_empty() {
            return;
        }
        ui::clear(&self.results);
        let found = self.find(&query);
        let mut hits = Vec::new();
        if found.is_empty() {
            self.results.append(&ui::label(&t!("Nothing in the manual matches that."), &["none"]));
        }
        for (i, hit) in found.into_iter().enumerate() {
            let content = ui::vbox(0, &[]);
            content.append(&ui::label(&hit.title, &["hit-title"]));
            content.append(&ui::label(&hit.text, &["hit-text"]));
            let button = gtk::Button::builder().child(&content).css_classes(["plain", "hit"]).build();
            if i == 0 {
                button.add_css_class("selected");
            }
            let (weak, route) = (self.me.clone(), hit.route.clone());
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.navigate(&route);
                    let this = this.clone();
                    glib::idle_add_local_once(move || this.clear_search());
                }
            });
            self.results.append(&button);
            hits.push((hit.route, button));
        }
        self.selected.set(0);
        self.hits.replace(hits);
    }

    fn move_selection(&self, step: i32) {
        let hits = self.hits.borrow();
        if hits.is_empty() {
            return;
        }
        let at = self.selected.get();
        hits[at].1.remove_css_class("selected");
        let next = (at as i32 + step).rem_euclid(hits.len() as i32) as usize;
        hits[next].1.add_css_class("selected");
        self.selected.set(next);
        let (scroller, button) = (self.results_scroller.clone(), hits[next].1.clone());
        scroll_into_view(&scroller, &button);
    }

    fn clear_search(&self) {
        self.search.set_text("");
        self.show_results();
    }
}

/// English reads left to right on its page, in a session in Arabic too: GTK gives each widget its direction itself
fn left_to_right(widget: &gtk::Widget) {
    widget.set_direction(gtk::TextDirection::Ltr);
    let mut child = widget.first_child();
    while let Some(next) = child {
        left_to_right(&next);
        child = next.next_sibling();
    }
}

/// A colour as Pango markup takes it
fn hex(color: &gdk::RGBA) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", byte(color.red()), byte(color.green()), byte(color.blue()), byte(color.alpha()))
}

/// The words around the first one searched for, as many as the page showed
fn snippet(text: &str, word: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let needle: Vec<char> = word.chars().collect();
    let at = if needle.is_empty() || lower.len() != chars.len() {
        None
    } else {
        (0..=lower.len().saturating_sub(needle.len())).find(|&i| lower[i..].starts_with(&needle))
    };
    match at {
        None => chars.iter().take(120).collect(),
        Some(at) => {
            let start = at.saturating_sub(50);
            let end = (at + 90).min(chars.len());
            let middle: String = chars[start..end].iter().collect();
            format!("{}{}…", if start > 0 { "…" } else { "" }, middle.trim())
        }
    }
}

/// What a scroller scrolls, which a place in it is measured against: not its child, the viewport, which stays put
/// while what is in it moves, so that a place measured against the viewport already has the scroll taken off
fn content_of(scroller: &gtk::ScrolledWindow) -> Option<gtk::Widget> {
    scroller.child().and_downcast::<gtk::Viewport>().and_then(|viewport| viewport.child())
}

/// Scrolls a list as little as it takes to show one of its rows (scrollIntoView with block: nearest)
fn scroll_into_view(scroller: &gtk::ScrolledWindow, row: &impl IsA<gtk::Widget>) {
    let Some(content) = content_of(scroller) else { return };
    let Some(point) = row.compute_point(&content, &amethystora_ui::graphene::Point::new(0.0, 0.0)) else { return };
    let adjustment = scroller.vadjustment();
    let (top, bottom) = (point.y() as f64, point.y() as f64 + row.height() as f64);
    if top < adjustment.value() {
        adjustment.set_value(top);
    } else if bottom > adjustment.value() + adjustment.page_size() {
        adjustment.set_value(bottom - adjustment.page_size());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_under_share() {
        assert_eq!(inside_share(Path::new("/usr/share/amethystora/manual/../keybindings.md")), Some(PathBuf::from("/usr/share/amethystora/keybindings.md")));
        assert_eq!(inside_share(Path::new("/usr/share/amethystora/manual/../../x.md")), None);
        assert_eq!(inside_share(Path::new("/usr/share/amethystora/manual/./faq.md")), Some(PathBuf::from("/usr/share/amethystora/manual/faq.md")));
    }

    #[test]
    fn show_me_takes_only_a_guides_id() {
        assert_eq!(route("guide:turn-off-bluetooth", "hardware"), "guide:turn-off-bluetooth");
        assert!(valid_guide("turn-off-bluetooth"));
        for id in ["", "-x", "a b", "A", "a/b", "../x", &"a".repeat(65)] {
            assert!(!valid_guide(id), "{id}");
        }
    }

    #[test]
    fn routes_links_as_manual_js_did() {
        assert_eq!(route("updates.md#rolling-back", "faq"), "#updates/rolling-back");
        assert_eq!(route("updates.md", "faq"), "#updates");
        assert_eq!(route("../manual/updates.md", "faq"), "#updates");
        assert_eq!(route("#local", "faq"), "#faq/local");
        assert_eq!(route("https://example.org/x.md", "faq"), "https://example.org/x.md");
        assert_eq!(route("weird", "faq"), "weird");
    }

    #[test]
    fn only_a_section_the_translation_lacks_opens_in_english() {
        let ids = |list: &[&str]| list.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        let page = Page {
            id: "updates".into(),
            section: String::new(),
            markdown: String::new(),
            title: String::new(),
            ids: ids(&["revenir-en-arriere"]),
            sections: Vec::new(),
            english: Some(English { markdown: String::new(), ids: ids(&["rolling-back", "stable"]) }),
            in_english: false,
        };
        assert!(page.english_for("rolling-back"));
        assert!(!page.english_for("revenir-en-arriere"));
        assert!(!page.english_for("nowhere"));
        assert!(!Page { english: None, ..page }.english_for("rolling-back"));
    }

    #[test]
    fn snippets_around_the_word() {
        assert_eq!(snippet("short text", "zzz"), "short text");
        let long = "a".repeat(60) + " needle " + &"b".repeat(100);
        let cut = snippet(&long, "needle");
        assert!(cut.starts_with('…') && cut.ends_with('…') && cut.contains("needle"));
    }
}
