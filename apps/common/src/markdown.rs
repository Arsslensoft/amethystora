//! Markdown as widgets, the way marked made it HTML for the Manual and for Notes: paragraphs, headings with the
//! ids links go by, lists and task lists, tables, quotes, code blocks, pictures, and inline code drawn as
//! code, or as keycaps when it is a key or a combination of them. Raw HTML is shown as the text it is. Every
//! block gets the CSS class its HTML element had (`p`, `h2`, `ul`, `code`, ...) so that the app's stylesheet
//! styles it; the vertical margins between blocks are the stylesheet's, given here, and collapse the way
//! HTML collapses them, which GTK does not do by itself.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::icon::IconData;
use crate::rich::{Deco, Kind, Rich, PAD};
use crate::{glib, gtk, icon, ui};
use gtk::prelude::*;
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options as Parse, Parser, Tag, TagEnd};

/// The kinds of block, for their margins
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Block {
    H1,
    H2,
    H3,
    H4,
    /// The paragraph straight after the title
    Lead,
    P,
    List,
    Item,
    Code,
    Table,
    Quote,
    Hr,
    Image,
}

/// What the page asks of the renderer
#[derive(Clone)]
pub struct Options {
    /// Where a link written in the page goes, as the label's href: the Manual's routing of `updates.md#x`
    pub route: Rc<dyn Fn(&str) -> String>,
    /// What a click on a link does with that href, or on an attachment with its href
    pub open: Rc<dyn Fn(&str)>,
    /// The ids the page's sections get, h2 and below in order; a missing one is made from its heading
    pub ids: Vec<String>,
    /// Inline code that is a key or a combination of keys is drawn as keycaps
    pub keycaps: bool,
    /// The colours of a key's lettering and of the + between keys (kbd's var(--fg), .keys > span's
    /// var(--soft)), and of an attachment's name, which Pango markup needs as they are
    pub key_color: String,
    pub plus_color: String,
    pub attachment_color: String,
    /// "Copy" and "Copied" for a copy button on every code block, or none
    pub copy: Option<(String, String)>,
    /// The # before each section's heading, there while the pointer is on it
    pub anchors: bool,
    /// A table's header in capitals
    pub head_capitals: bool,
    /// A line break in a paragraph is one, as marked's `breaks: true` had it
    pub breaks: bool,
    /// Thin spaces either side of inline code, and the radius of its background
    pub code_pad: usize,
    pub code_radius: f32,
    /// The widget for a picture, by its address and its text, or None to show the text instead
    pub image: Option<Rc<dyn Fn(&str, &str) -> Option<gtk::Widget>>>,
    /// Pictures that follow one another in a paragraph, with nothing but white space between them, share a row of
    /// their own (a flow box of class `cards`, which wraps), as the Manual's moving cards do; otherwise each is a block
    pub image_rows: bool,
    /// A task list's box was clicked: its index among the page's boxes. Without it the boxes only show.
    pub task: Option<Rc<dyn Fn(usize)>>,
    /// Whether a link (its routed href) is an attachment, drawn as a chip with `clip` at its start
    pub attachment: Option<Rc<dyn Fn(&str) -> bool>>,
    pub clip: Option<&'static IconData>,
    /// The margins above and below each kind of block, in pixels, before the zoom
    pub margins: fn(Block) -> (i32, i32),
}

impl Options {
    /// Plain Markdown: links routed and opened as given, no keycaps, buttons, anchors or pictures
    pub fn new(route: Box<dyn Fn(&str) -> String>, open: Rc<dyn Fn(&str)>, margins: fn(Block) -> (i32, i32)) -> Options {
        Options {
            route: Rc::from(route),
            open,
            ids: Vec::new(),
            keycaps: false,
            key_color: "#000000".into(),
            plus_color: "#000000".into(),
            attachment_color: "#000000".into(),
            copy: None,
            anchors: false,
            head_capitals: false,
            breaks: false,
            code_pad: 1,
            code_radius: 6.0,
            image: None,
            image_rows: false,
            task: None,
            attachment: None,
            clip: None,
            margins,
        }
    }
}

/// What rendering made: the widgets, and each section's heading by its id, to scroll to
pub struct Rendered {
    pub root: gtk::Box,
    pub headings: Vec<(String, gtk::Widget)>,
}

// --- Ids and words ---------------------------------------------------------------------------------------

/// The id a heading gets, as manual.js made it from marked's HTML of the heading: its text with what HTML
/// escapes (& < > " ') taken out, in lower case, every run of what is neither a letter nor a digit a dash
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.chars().filter(|c| !matches!(c, '&' | '<' | '>' | '"' | '\'')).flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    out
}

fn parse_options() -> Parse {
    Parse::ENABLE_TABLES | Parse::ENABLE_STRIKETHROUGH | Parse::ENABLE_TASKLISTS
}

/// The text of every heading below the title, in order, for their ids
pub fn heading_texts(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for event in Parser::new_ext(markdown, parse_options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) if level != HeadingLevel::H1 => current = Some(String::new()),
            Event::End(TagEnd::Heading(level)) if level != HeadingLevel::H1 => {
                if let Some(text) = current.take() {
                    out.push(text);
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(current) = current.as_mut() {
                    current.push_str(&text);
                }
            }
            _ => {}
        }
    }
    out
}

/// A page as search reads it: its sections, each its heading's id and title and the text under it, split
/// at the headings of the second and third level, the first with no id
pub struct Section {
    pub id: String,
    pub title: String,
    pub text: String,
}

pub fn sections(markdown: &str, title: &str, ids: &[String]) -> Vec<Section> {
    let mut out = vec![Section { id: String::new(), title: title.to_string(), text: String::new() }];
    let mut heading: Option<(HeadingLevel, String)> = None;
    let mut next = 0;
    // What a block says is its text as textContent has it, inline pieces joined as they are; blocks are
    // set apart by a space
    let gap = |out: &mut Vec<Section>| {
        if let Some(last) = out.last_mut() {
            last.text.push(' ');
        }
    };
    for event in Parser::new_ext(markdown, parse_options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = Some((level, String::new())),
            Event::End(TagEnd::Heading(_)) => {
                let Some((level, text)) = heading.take() else { continue };
                if level == HeadingLevel::H1 {
                    continue;
                }
                let id = ids.get(next).cloned().unwrap_or_else(|| slug(&text));
                next += 1;
                if level == HeadingLevel::H2 || level == HeadingLevel::H3 {
                    out.push(Section { id, title: text, text: String::new() });
                } else {
                    gap(&mut out);
                    if let Some(last) = out.last_mut() {
                        last.text.push_str(&text);
                    }
                }
            }
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => match heading.as_mut() {
                Some((_, heading)) => heading.push_str(&text),
                None => {
                    if let Some(last) = out.last_mut() {
                        last.text.push_str(&text);
                    }
                }
            },
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, heading)) = heading.as_mut() {
                    heading.push(' ');
                } else {
                    gap(&mut out);
                }
            }
            Event::Start(Tag::Paragraph | Tag::Item | Tag::TableCell | Tag::CodeBlock(_) | Tag::BlockQuote(_)) | Event::Rule => gap(&mut out),
            _ => {}
        }
    }
    out
}

// --- Keys ------------------------------------------------------------------------------------------------

fn n_digits(number: &str) -> bool {
    (1..=2).contains(&number.len()) && number.chars().all(|c| c.is_ascii_digit())
}

/// A key as the Manual names one: Super, Ctrl, F5, a letter or digit, or one of . , / ` = - [ ]
fn is_key(key: &str) -> bool {
    const NAMED: &[&str] = &[
        "Super", "Ctrl", "Alt", "Shift", "Fn", "Tab", "Esc", "Escape", "Return", "Enter", "Space", "Backspace", "Delete", "Print", "Home", "End",
        "PageUp", "PageDown", "Up", "Down", "Left", "Right",
    ];
    if NAMED.contains(&key) {
        return true;
    }
    if let Some(number) = key.strip_prefix('F').filter(|n| !n.is_empty()) {
        return n_digits(number);
    }
    let mut chars = key.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_uppercase() || c.is_ascii_digit() || ".,/`=-[]".contains(c))
}

/// The keys of a code span that is a key or a combination of keys: `Super+Shift+B`. On its own, only a
/// named key or a letter: a lone `1` or `-` is more likely a value than a key.
pub fn keycaps(text: &str) -> Option<Vec<&str>> {
    let keys: Vec<&str> = text.split('+').collect();
    if !keys.iter().all(|key| is_key(key)) {
        return None;
    }
    if keys.len() == 1 {
        let one = keys[0];
        let named = one.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && one.chars().skip(1).all(|c| c.is_ascii_lowercase());
        let function = one.strip_prefix('F').is_some_and(n_digits);
        if !(named || function) {
            return None;
        }
    }
    Some(keys)
}

// --- Inline text -----------------------------------------------------------------------------------------

#[derive(Default)]
struct Inline {
    markup: String,
    /// The text without its markup, for a heading's id
    plain: String,
    /// How long the label's text is so far, in bytes, for the decorations' ranges
    length: usize,
    decos: Vec<Deco>,
    /// In a table's header, whose text is in capitals
    capitals: bool,
    /// In a table's cell, where inline code does not break
    cell: bool,
    /// An attachment's link being written: where its text started, and where it goes
    chip: Option<(usize, String)>,
    /// A picture's text, while its events come
    image: Option<(String, String)>,
}

const MONO: &str = "JetBrains Mono, Adwaita Mono, monospace";
const SANS: &str = "Inter, Adwaita Sans, Cantarell, sans-serif";

impl Inline {
    fn text(&mut self, text: &str) {
        if let Some((_, alt)) = self.image.as_mut() {
            alt.push_str(text);
            return;
        }
        let text = if self.capitals { text.to_uppercase() } else { text.to_string() };
        self.markup.push_str(&glib::markup_escape_text(&text));
        self.length += text.len();
        self.plain.push_str(&text);
    }

    fn raw(&mut self, markup: &str, text: &str) {
        self.markup.push_str(markup);
        self.length += text.len();
    }

    fn code(&mut self, text: &str, options: &Options) {
        if let Some((_, alt)) = self.image.as_mut() {
            alt.push_str(text);
            return;
        }
        self.plain.push_str(text);
        if options.keycaps {
            if let Some(keys) = self::keycaps(text) {
                for (i, key) in keys.iter().enumerate() {
                    if i > 0 {
                        // .keys > span: the + between keys, soft and smaller
                        let plus = "\u{200A}+\u{200A}";
                        self.raw(&format!("<span size=\"80%\" foreground=\"{}\">{plus}</span>", options.plus_color), plus);
                    }
                    // kbd: padding 1px 7px, about three thin spaces of its lettering, which also makes a
                    // one-letter key about as wide as its min-width of 1.9 em
                    let pad = PAD.repeat(3);
                    let shown = format!("{pad}{key}{pad}");
                    let start = self.length;
                    self.raw(
                        &format!(
                            "<span font_family=\"{SANS}\" weight=\"600\" size=\"80%\" foreground=\"{}\">{}</span>",
                            options.key_color,
                            glib::markup_escape_text(&shown)
                        ),
                        &shown,
                    );
                    self.decos.push(Deco { start, end: self.length, kind: Kind::Key, target: None });
                }
                return;
            }
        }
        let mut shown = text.to_string();
        if self.cell {
            // td code { white-space: nowrap }
            shown = shown.replace(' ', "\u{a0}").replace('-', "\u{2011}");
        }
        let pad = PAD.repeat(options.code_pad);
        let shown = format!("{pad}{shown}{pad}");
        let start = self.length;
        self.raw(&format!("<span font_family=\"{MONO}\" size=\"86%\">{}</span>", glib::markup_escape_text(&shown)), &shown);
        self.decos.push(Deco { start, end: self.length, kind: Kind::Code(options.code_radius), target: None });
    }

    fn is_empty(&self) -> bool {
        self.markup.trim().is_empty()
    }
}

// --- Blocks ----------------------------------------------------------------------------------------------

/// A box of blocks, whose margins collapse as HTML's do
struct Stack {
    container: gtk::Box,
    blocks: Vec<(gtk::Widget, i32, i32)>,
    /// A quote's last block has no margin below it (blockquote > :last-child)
    trim_last: bool,
}

impl Stack {
    fn new(container: gtk::Box, trim_last: bool) -> Stack {
        Stack { container, blocks: Vec::new(), trim_last }
    }

    fn push(&mut self, widget: impl IsA<gtk::Widget>, margins: (i32, i32)) {
        let widget = widget.upcast();
        // As finish() gives them, for a page that shows while it fills in
        let above = self.blocks.last().map_or(margins.0, |last| margins.0.max(last.2));
        widget.set_margin_top((above as f64 * icon::zoom()).round() as i32);
        self.container.append(&widget);
        self.blocks.push((widget, margins.0, margins.1));
    }

    fn finish(self) -> gtk::Box {
        let zoom = icon::zoom();
        let scaled = |n: i32| (n as f64 * zoom).round() as i32;
        let count = self.blocks.len();
        for (i, (widget, top, bottom)) in self.blocks.iter().enumerate() {
            let above = if i == 0 { *top } else { (*top).max(self.blocks[i - 1].2) };
            widget.set_margin_top(scaled(above));
            let below = if i + 1 == count { if self.trim_last { 0 } else { *bottom } } else { 0 };
            widget.set_margin_bottom(scaled(below));
        }
        self.container
    }
}

enum Frame {
    Blocks(Stack),
    List { stack: Stack, ordered: Option<u64>, index: u64 },
    Item { stack: Stack, marker: String, task: Option<bool> },
    Table { grid: gtk::Grid, alignments: Vec<Alignment>, row: i32, column: i32, head: bool, cells: Vec<Vec<gtk::Widget>> },
}

/// The page's state while it is rendered
struct Render {
    options: Rc<Options>,
    frames: Vec<Frame>,
    inline: Option<Inline>,
    tasks: Rc<Cell<usize>>,
    /// The row the paragraph's last pictures went into, while nothing but white space has come after them
    cards: Option<gtk::FlowBox>,
}

impl Render {
    fn inline(&mut self) -> &mut Inline {
        self.inline.get_or_insert_with(Inline::default)
    }

    fn push_block(&mut self, widget: gtk::Widget, margins: (i32, i32)) {
        match self.frames.last_mut().expect("the page itself at least") {
            Frame::Blocks(stack) | Frame::List { stack, .. } | Frame::Item { stack, .. } => stack.push(widget, margins),
            Frame::Table { .. } => {}
        }
    }

    /// The label a run of inline text becomes, with its links followed by the page
    fn paragraph(&self, inline: Inline, classes: &[&str]) -> Rich {
        let has_chips = inline.decos.iter().any(|deco| deco.kind == Kind::Chip);
        let rich = Rich::new(&inline.markup, inline.decos, classes);
        let open = self.options.open.clone();
        rich.label().connect_activate_link(move |_, uri| {
            open(uri);
            glib::Propagation::Stop
        });
        if has_chips {
            let open = self.options.open.clone();
            rich.set_chips(self.options.clip, Rc::new(move |target| open(target)));
        }
        rich
    }

    /// The paragraph so far, as a block of its own, when a picture comes in the middle of it
    fn flush_paragraph(&mut self, class: &str) {
        if let Some(text) = self.inline.take().filter(|text| !text.is_empty()) {
            let rich = self.paragraph(text, &[class]);
            self.push_block(rich.upcast(), (self.options.margins)(Block::P));
        }
    }
}

pub fn render(markdown: &str, options: &Options) -> Rendered {
    let mut filling = Filling::new(markdown, options.clone());
    filling.advance(usize::MAX);
    filling.finish()
}

/// A page rendered a few blocks at a time, for a long one that should not stop the window: its root is there from the
/// start and fills in with each `advance`.
pub struct Filling {
    options: Rc<Options>,
    render: Render,
    events: std::vec::IntoIter<Event<'static>>,
    root: gtk::Box,
    code: Option<String>,
    headings: Vec<(String, gtk::Widget)>,
    next_id: usize,
    seen_title: bool,
    lead_done: bool,
}

impl Filling {
    pub fn new(markdown: &str, options: Options) -> Filling {
        let options = Rc::new(options);
        let root = ui::vbox(0, &["blocks"]);
        let render = Render {
            options: options.clone(),
            frames: vec![Frame::Blocks(Stack::new(root.clone(), false))],
            inline: None,
            tasks: Rc::new(Cell::new(0)),
            cards: None,
        };
        let events: Vec<Event<'static>> = Parser::new_ext(markdown, parse_options()).map(Event::into_static).collect();
        Filling { options, render, events: events.into_iter(), root, code: None, headings: Vec::new(), next_id: 0, seen_title: false, lead_done: false }
    }

    pub fn root(&self) -> gtk::Box {
        self.root.clone()
    }

    /// The headings so far, each with its widget
    pub fn headings(&self) -> &[(String, gtk::Widget)] {
        &self.headings
    }

    /// As far as the end of `blocks` more of the page's blocks. Whether there is more.
    pub fn advance(&mut self, blocks: usize) -> bool {
        let options_kept = self.options.clone();
        let options: &Options = &options_kept;
        let render = &mut self.render;
        let mut code = self.code.take();
        let mut headings = std::mem::take(&mut self.headings);
        let (mut next_id, mut seen_title, mut lead_done) = (self.next_id, self.seen_title, self.lead_done);
        let margins = options.margins;
        let mut done = 0;

        for event in self.events.by_ref() {
            match event {
                Event::Start(Tag::Paragraph) => render.inline = Some(Inline::default()),
                Event::End(TagEnd::Paragraph) => {
                    render.cards = None;
                    if let Some(text) = render.inline.take() {
                        // .page h1 + p: the paragraph straight after the title
                        let lead = seen_title && !lead_done && render.frames.len() == 1;
                        lead_done = true;
                        if text.is_empty() {
                            continue;
                        }
                        let (kind, class) = if lead { (Block::Lead, "lead") } else { (Block::P, "p") };
                        let rich = render.paragraph(text, &[class]);
                        render.push_block(rich.upcast(), margins(kind));
                    }
                }
                Event::Start(Tag::Heading { .. }) => render.inline = Some(Inline::default()),
                Event::End(TagEnd::Heading(level)) => {
                    let text = render.inline.take().unwrap_or_default();
                    if level == HeadingLevel::H1 {
                        seen_title = true;
                        let rich = render.paragraph(text, &["h1"]);
                        render.push_block(rich.upcast(), margins(Block::H1));
                        continue;
                    }
                    lead_done = true;
                    let id = options.ids.get(next_id).cloned().unwrap_or_else(|| slug(&text.plain));
                    next_id += 1;
                    let (kind, class) = match level {
                        HeadingLevel::H2 => (Block::H2, "h2"),
                        HeadingLevel::H3 => (Block::H3, "h3"),
                        _ => (Block::H4, "h4"),
                    };
                    let rich = render.paragraph(text, &[class]);
                    let overlay = gtk::Overlay::new();
                    overlay.add_css_class("heading");
                    overlay.add_css_class(class);
                    overlay.set_child(Some(&rich));
                    if options.anchors {
                        // The # before a section's heading, there while the pointer is on it
                        let anchor = ui::line("#", &["anchor"]);
                        anchor.set_halign(gtk::Align::Start);
                        anchor.set_valign(gtk::Align::Start);
                        overlay.add_overlay(&anchor);
                        hover_class(&overlay);
                    }
                    headings.push((id, overlay.clone().upcast()));
                    render.push_block(overlay.upcast(), margins(kind));
                }
                Event::Start(Tag::BlockQuote(_)) => render.frames.push(Frame::Blocks(Stack::new(ui::vbox(0, &["blockquote"]), true))),
                Event::End(TagEnd::BlockQuote(_)) => {
                    if let Some(Frame::Blocks(stack)) = render.frames.pop() {
                        render.push_block(stack.finish().upcast(), margins(Block::Quote));
                    }
                }
                Event::Start(Tag::CodeBlock(_)) => code = Some(String::new()),
                Event::End(TagEnd::CodeBlock) => {
                    let text = code.take().unwrap_or_default();
                    let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
                    render.push_block(code_block(&text, options.copy.as_ref()).upcast(), margins(Block::Code));
                }
                Event::Start(Tag::List(start)) => {
                    let class = if start.is_some() { "ol" } else { "ul" };
                    render.frames.push(Frame::List { stack: Stack::new(ui::vbox(0, &[class]), false), ordered: start, index: start.unwrap_or(1) });
                }
                Event::End(TagEnd::List(_)) => {
                    if let Some(Frame::List { stack, .. }) = render.frames.pop() {
                        render.push_block(stack.finish().upcast(), margins(Block::List));
                    }
                }
                Event::Start(Tag::Item) => {
                    let marker = match render.frames.last_mut() {
                        Some(Frame::List { ordered: Some(_), index, .. }) => {
                            let marker = format!("{index}.");
                            *index += 1;
                            marker
                        }
                        _ => "•".to_string(),
                    };
                    render.frames.push(Frame::Item { stack: Stack::new(ui::vbox(0, &["li-body"]), false), marker, task: None });
                }
                Event::End(TagEnd::Item) => {
                    // A tight list's item has its text without a paragraph around it
                    if let Some(text) = render.inline.take().filter(|text| !text.is_empty()) {
                        let rich = render.paragraph(text, &["li-text"]);
                        render.push_block(rich.upcast(), (0, 0));
                    }
                    if let Some(Frame::Item { stack, marker, task }) = render.frames.pop() {
                        let row = ui::hbox(0, &["li"]);
                        match task {
                            // A task list's item has its box in place of the bullet
                            Some(done) => {
                                row.add_css_class("task");
                                if done {
                                    row.add_css_class("done");
                                }
                                let check = gtk::CheckButton::builder().active(done).valign(gtk::Align::Start).css_classes(["task-check"]).build();
                                let index = render.tasks.get();
                                render.tasks.set(index + 1);
                                match options.task.clone() {
                                    Some(toggle) => {
                                        check.connect_toggled(move |check| {
                                            // The note changes, and is drawn again from it
                                            check.set_sensitive(false);
                                            toggle(index);
                                        });
                                    }
                                    None => check.set_sensitive(false),
                                }
                                row.append(&check);
                            }
                            None => {
                                let mark = ui::line(&marker, &["marker"]);
                                mark.set_valign(gtk::Align::Start);
                                mark.set_xalign(1.0);
                                row.append(&mark);
                            }
                        }
                        let body = stack.finish();
                        body.set_hexpand(true);
                        row.append(&body);
                        render.push_block(row.upcast(), margins(Block::Item));
                    }
                }
                Event::TaskListMarker(done) => {
                    if let Some(Frame::Item { task, .. }) = render.frames.last_mut() {
                        *task = Some(done);
                    }
                }
                Event::Start(Tag::Table(alignments)) => {
                    let grid = gtk::Grid::new();
                    grid.add_css_class("table");
                    render.frames.push(Frame::Table { grid, alignments, row: 0, column: 0, head: false, cells: Vec::new() });
                }
                Event::End(TagEnd::Table) => {
                    if let Some(Frame::Table { grid, cells, .. }) = render.frames.pop() {
                        hover_rows(&cells);
                        render.push_block(grid.upcast(), margins(Block::Table));
                    }
                }
                Event::Start(Tag::TableHead) => {
                    if let Some(Frame::Table { head, column, cells, .. }) = render.frames.last_mut() {
                        *head = true;
                        *column = 0;
                        cells.push(Vec::new());
                    }
                }
                Event::End(TagEnd::TableHead) => {
                    if let Some(Frame::Table { head, row, .. }) = render.frames.last_mut() {
                        *head = false;
                        *row += 1;
                    }
                }
                Event::Start(Tag::TableRow) => {
                    if let Some(Frame::Table { column, cells, .. }) = render.frames.last_mut() {
                        *column = 0;
                        cells.push(Vec::new());
                    }
                }
                Event::End(TagEnd::TableRow) => {
                    if let Some(Frame::Table { row, .. }) = render.frames.last_mut() {
                        *row += 1;
                    }
                }
                Event::Start(Tag::TableCell) => {
                    let head = matches!(render.frames.last(), Some(Frame::Table { head: true, .. }));
                    render.inline = Some(Inline { capitals: head && options.head_capitals, cell: true, ..Default::default() });
                }
                Event::End(TagEnd::TableCell) => {
                    let text = render.inline.take().unwrap_or_default();
                    let head = matches!(render.frames.last(), Some(Frame::Table { head: true, .. }));
                    let rich = render.paragraph(text, &[if head { "th-text" } else { "td-text" }]);
                    if let Some(Frame::Table { grid, alignments, row, column, head, cells }) = render.frames.last_mut() {
                        match alignments.get(*column as usize) {
                            Some(Alignment::Center) => rich.label().set_xalign(0.5),
                            Some(Alignment::Right) => rich.label().set_xalign(1.0),
                            _ => {}
                        }
                        let cell = ui::vbox(0, &[if *head { "th" } else { "td" }]);
                        cell.append(&rich);
                        cell.set_hexpand(true);
                        grid.attach(&cell, *column, *row, 1, 1);
                        if let Some(last) = cells.last_mut() {
                            last.push(cell.upcast());
                        }
                        *column += 1;
                    }
                }
                Event::Rule => {
                    let rule = ui::hbox(0, &["hr"]);
                    render.push_block(rule.upcast(), margins(Block::Hr));
                }
                Event::Text(text) => match code.as_mut() {
                    Some(code) => code.push_str(&text),
                    None => render.inline().text(&text),
                },
                Event::Code(text) => {
                    render.inline().code(&text, options);
                }
                // Raw HTML is shown as the text it is
                Event::Html(text) | Event::InlineHtml(text) => match code.as_mut() {
                    Some(code) => code.push_str(&text),
                    None => render.inline().text(&text),
                },
                Event::SoftBreak => match code.as_mut() {
                    Some(code) => code.push('\n'),
                    None => {
                        let breaks = options.breaks;
                        render.inline().text(if breaks { "\n" } else { " " });
                    }
                },
                Event::HardBreak => render.inline().text("\n"),
                Event::Start(Tag::Emphasis) => render.inline().raw("<i>", ""),
                Event::End(TagEnd::Emphasis) => render.inline().raw("</i>", ""),
                Event::Start(Tag::Strong) => render.inline().raw("<span weight=\"650\">", ""),
                Event::End(TagEnd::Strong) => render.inline().raw("</span>", ""),
                Event::Start(Tag::Strikethrough) => render.inline().raw("<s>", ""),
                Event::End(TagEnd::Strikethrough) => render.inline().raw("</s>", ""),
                Event::Start(Tag::Link { dest_url, title, .. }) => {
                    let href = (options.route)(&dest_url);
                    if options.attachment.as_ref().is_some_and(|is| is(&href)) {
                        // An attachment: a chip with its clip, clicked as a whole, the room for the clip and
                        // its padding made with thin spaces (a.attachment's 9 px, 14 px clip and 5 px gap)
                        let pad = PAD.repeat(9);
                        let inline = render.inline();
                        let start = inline.length;
                        inline.raw(&format!("<span foreground=\"{}\">{pad}", options.attachment_color), &pad);
                        inline.chip = Some((start, href));
                    } else {
                        let title = if title.is_empty() { String::new() } else { format!(" title=\"{}\"", glib::markup_escape_text(&title)) };
                        render.inline().raw(&format!("<a href=\"{}\"{title}>", glib::markup_escape_text(&href)), "");
                    }
                }
                Event::End(TagEnd::Link) => {
                    let inline = render.inline();
                    match inline.chip.take() {
                        Some((start, href)) => {
                            let pad = PAD.repeat(3);
                            inline.raw(&format!("{pad}</span>"), &pad);
                            inline.decos.push(Deco { start, end: inline.length, kind: Kind::Chip, target: Some(href) });
                        }
                        None => inline.raw("</a>", ""),
                    }
                }
                Event::Start(Tag::Image { dest_url, .. }) => {
                    render.inline().image = Some((dest_url.to_string(), String::new()));
                }
                Event::End(TagEnd::Image) => {
                    let Some((url, alt)) = render.inline().image.take() else { continue };
                    let href = (options.route)(&url);
                    match options.image.as_ref().and_then(|image| image(&href, &alt)) {
                        // Beside the pictures just before it, with nothing but white space between them
                        Some(picture) if options.image_rows && render.cards.is_some() && render.inline.as_ref().is_none_or(Inline::is_empty) => {
                            if let Some(cards) = render.cards.as_ref() {
                                cards.append(&picture);
                                // The card itself takes the focus, not the row's slot round it
                                if let Some(slot) = picture.parent() { slot.set_focusable(false); }
                            }
                            render.inline = Some(Inline::default());
                        }
                        Some(picture) if options.image_rows => {
                            render.flush_paragraph("p");
                            let cards = gtk::FlowBox::builder()
                                .css_classes(["cards"])
                                .homogeneous(true)
                                .column_spacing(12)
                                .row_spacing(12)
                                .selection_mode(gtk::SelectionMode::None)
                                .max_children_per_line(16)
                                .halign(gtk::Align::Start)
                                .build();
                            cards.append(&picture);
                            if let Some(slot) = picture.parent() { slot.set_focusable(false); }
                            render.push_block(cards.clone().upcast(), margins(Block::Image));
                            render.cards = Some(cards);
                            render.inline = Some(Inline::default());
                        }
                        // A picture is a block of its own, between what the paragraph said before and after it
                        Some(picture) => {
                            render.flush_paragraph("p");
                            render.push_block(picture, margins(Block::Image));
                            render.inline = Some(Inline::default());
                        }
                        None => render.inline().text(&alt),
                    }
                }
                _ => {}
            }
            if render.frames.len() == 1 && render.inline.is_none() && code.is_none() {
                done += 1;
                if done >= blocks {
                    break;
                }
            }
        }
        self.code = code;
        self.headings = headings;
        (self.next_id, self.seen_title, self.lead_done) = (next_id, seen_title, lead_done);
        self.events.len() > 0
    }

    pub fn finish(mut self) -> Rendered {
        let root = match self.render.frames.drain(..).next() {
            Some(Frame::Blocks(stack)) => stack.finish(),
            _ => ui::vbox(0, &["blocks"]),
        };
        Rendered { root, headings: self.headings }
    }
}

/// The class `hover` on a widget while the pointer is on it, for what CSS's :hover did to its children
fn hover_class(widget: &impl IsA<gtk::Widget>) {
    let hover = gtk::EventControllerMotion::new();
    let shown = widget.clone().upcast::<gtk::Widget>();
    hover.connect_enter(move |_, _, _| shown.add_css_class("hover"));
    let shown = widget.clone().upcast::<gtk::Widget>();
    hover.connect_leave(move |_| shown.remove_css_class("hover"));
    widget.add_controller(hover);
}

/// A code block: the code, scrolled sideways when it is wider than the page, and a copy button over its
/// corner, there while the pointer is on the block
fn code_block(text: &str, copy: Option<&(String, String)>) -> gtk::Overlay {
    let code = gtk::Label::builder().label(text).xalign(0.0).selectable(true).css_classes(["pre-code"]).build();
    code.set_direction(gtk::TextDirection::Ltr);
    let scroller = gtk::ScrolledWindow::builder()
        .vscrollbar_policy(gtk::PolicyType::Never)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_height(true)
        .child(&code)
        .css_classes(["pre"])
        .build();
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("code");
    overlay.set_direction(gtk::TextDirection::Ltr);
    overlay.set_child(Some(&scroller));
    if let Some((copy, copied)) = copy {
        let button = gtk::Button::with_label(copy);
        button.add_css_class("plain");
        button.add_css_class("copy");
        button.set_halign(gtk::Align::End);
        button.set_valign(gtk::Align::Start);
        let (text, copy, copied) = (text.to_string(), copy.clone(), copied.clone());
        button.connect_clicked(move |button| {
            crate::process::copy(&text);
            button.set_label(&copied);
            let (button, copy) = (button.clone(), copy.clone());
            glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || button.set_label(&copy));
        });
        overlay.add_overlay(&button);
        hover_class(&overlay);
    }
    overlay
}

/// tr:hover td: the whole row lit while the pointer is on any of its cells
fn hover_rows(rows: &[Vec<gtk::Widget>]) {
    for row in rows.iter().skip(1) {
        let shared = Rc::new(RefCell::new(row.clone()));
        for cell in row {
            let hover = gtk::EventControllerMotion::new();
            let cells = shared.clone();
            hover.connect_enter(move |_, _, _| cells.borrow().iter().for_each(|c| c.add_css_class("hover")));
            let cells = shared.clone();
            hover.connect_leave(move |_| cells.borrow().iter().for_each(|c| c.remove_css_class("hover")));
            cell.add_controller(hover);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_as_manual_js_did() {
        assert_eq!(slug("Rolling back"), "rolling-back");
        assert_eq!(slug("Don't panic & relax"), "dont-panic-relax");
        assert_eq!(slug("The `ame` command"), "the-ame-command");
        assert_eq!(slug("Été — déjà vu"), "été-déjà-vu");
        assert_eq!(slug("  Spaces, then 1.2!  "), "spaces-then-1-2");
    }

    #[test]
    fn keycaps_only_for_keys() {
        assert_eq!(keycaps("Super+Shift+B"), Some(vec!["Super", "Shift", "B"]));
        assert_eq!(keycaps("Ctrl+K"), Some(vec!["Ctrl", "K"]));
        assert_eq!(keycaps("F5"), Some(vec!["F5"]));
        assert_eq!(keycaps("Super"), Some(vec!["Super"]));
        assert_eq!(keycaps("K"), Some(vec!["K"]));
        assert_eq!(keycaps("Super+Escape"), Some(vec!["Super", "Escape"]));
        assert_eq!(keycaps("Super+["), Some(vec!["Super", "["]));
        assert_eq!(keycaps("["), None);
        assert_eq!(keycaps("1"), None);
        assert_eq!(keycaps("-"), None);
        assert_eq!(keycaps("ame update"), None);
        assert_eq!(keycaps("Super+x"), None);
        assert_eq!(keycaps("Super+F"), Some(vec!["Super", "F"]));
        assert_eq!(keycaps("F"), Some(vec!["F"]));
        // F\d{1,2}, as manual.js had it
        assert_eq!(keycaps("F13"), Some(vec!["F13"]));
        assert_eq!(keycaps("F123"), None);
    }

    #[test]
    fn sections_split_at_headings() {
        let md = "# Title\n\nIntro text.\n\n## First part\n\nBody `code` (x).\n\n### Sub\n\nMore.\n\n#### Deep\n\nDeeper.\n";
        let sections = sections(md, "Title", &[]);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[1].id, "first-part");
        assert!(sections[1].text.contains("Body code (x)."));
        assert_eq!(sections[2].title, "Sub");
        assert!(sections[2].text.contains("Deep"));
        assert_eq!(heading_texts(md), vec!["First part", "Sub", "Deep"]);
    }
    #[test]
    fn pictures_in_one_paragraph_share_a_row() {
        // Widgets need GTK, which a build without a display does not have: there, this is left to the lab
        if gtk::init().is_err() {
            return;
        }
        let route: Box<dyn Fn(&str) -> String> = Box::new(|href| href.to_string());
        let mut options = Options::new(route, Rc::new(|_| {}), |_| (0, 0));
        options.image = Some(Rc::new(|_, alt| Some(gtk::Label::new(Some(alt)).upcast())));
        options.image_rows = true;
        let rows = |options: &Options, markdown: &str| {
            let root = render(markdown, options).root;
            let mut kinds = Vec::new();
            let mut child = root.first_child();
            while let Some(widget) = child {
                kinds.push(widget.downcast_ref::<gtk::FlowBox>().map(|row| {
                    let mut count = 0;
                    let mut slot = row.first_child();
                    while let Some(next) = slot {
                        count += 1;
                        slot = next.next_sibling();
                    }
                    count
                }));
                child = widget.next_sibling();
            }
            kinds
        };
        assert_eq!(rows(&options, "![a](a.webm) ![b](b.webm)\n![c](c.webm)\n"), vec![Some(3)]);
        assert_eq!(rows(&options, "![a](a.webm)\n\n![b](b.webm)\n"), vec![Some(1), Some(1)]);
        assert_eq!(rows(&options, "![a](a.webm) and ![b](b.webm)\n"), vec![Some(1), None, Some(1)]);
        options.image_rows = false;
        assert_eq!(rows(&options, "![a](a.webm) ![b](b.webm)\n"), vec![None, None]);
    }
}
