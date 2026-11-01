//! Amethystora Trim (round 6, R6-19; plans/ui-ideas/spec-finish.md): the small window a screen recording's notice
//! opens with Trim, while the desktop's recording-trim key is on. It shows the recording as a strip of ten frames with
//! its two ends to drag (or the Start and End sliders under it, for the keyboard), the frame at the end being moved, and
//! saves the part between them beside the recording, as WebM or as a short GIF for a chat or an issue: "NAME
//! (trimmed).webm", with a number when that is taken. The recording itself is never changed, and nothing is ever
//! written over.
//!
//! Its frames and the cut are ffmpeg's (Fedora's ffmpeg-free), and its length ffprobe's, each run as an argument vector
//! with absolute paths (cut.rs). Frames are kept in a folder of its own under the runtime folder, 0700, taken away when
//! the window closes. It runs only when opened, on the file it was given.

mod cut;

use std::cell::{Cell, RefCell};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::process::{self, Options};
use amethystora_ui::{adw, gio, glib, gtk, t, ui};

use cut::Format;

const FFMPEG: &str = "/usr/bin/ffmpeg";
const FFPROBE: &str = "/usr/bin/ffprobe";
/// An end at rest this long before its frame is asked for
const SETTLE: Duration = Duration::from_millis(150);
/// The longest a cut may take
const CUT_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const FRAME_TIMEOUT: Duration = Duration::from_secs(30);

pub fn main() -> glib::ExitCode {
    app::run(
        Spec { name: "trim", id: "org.amethystora.Trim", css: include_str!("trim.css"), width: 760, height: 580, min_width: 480, min_height: 440 },
        Trim::new,
    )
}

pub struct Trim {
    me: Weak<Trim>,
    shell: Rc<Shell>,
    input: RefCell<Option<PathBuf>>,
    duration: Cell<f64>,
    start: gtk::Adjustment,
    end: gtk::Adjustment,
    title: gtk::Label,
    preview: gtk::Picture,
    strip: gtk::Picture,
    marks: gtk::DrawingArea,
    from: gtk::Label,
    kept: gtk::Label,
    to: gtk::Label,
    status: gtk::Label,
    show: gtk::Button,
    gif: gtk::Button,
    webm: gtk::Button,
    work: PathBuf,
    frame_timer: RefCell<Option<glib::SourceId>>,
    busy: Cell<bool>,
    saved: RefCell<Option<PathBuf>>,
    moving_start: Cell<bool>,
    adjusting: Cell<bool>,
    last_frame: RefCell<Option<PathBuf>>,
}

impl Trim {
    fn new(shell: Rc<Shell>) -> Rc<Self> {
        let window = &shell.window;
        window.set_title(Some(&t!("Trim")));

        let body = ui::vbox(14, &["trim"]);
        let title = ui::line("", &["trim-title"]);
        title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let preview = gtk::Picture::builder().content_fit(gtk::ContentFit::Contain).can_shrink(true).vexpand(true).build();
        preview.add_css_class("trim-preview");
        preview.set_alternative_text(Some(&t!("The frame at the end being moved")));

        let strip = gtk::Picture::builder().content_fit(gtk::ContentFit::Fill).can_shrink(true).build();
        strip.add_css_class("trim-strip");
        let marks = gtk::DrawingArea::new();
        marks.set_content_height(cut::STRIP_HEIGHT as i32);
        marks.set_hexpand(true);
        let film = gtk::Overlay::new();
        film.add_css_class("trim-film");
        film.set_child(Some(&strip));
        film.add_overlay(&marks);
        film.set_overflow(gtk::Overflow::Hidden);

        let start = gtk::Adjustment::new(0.0, 0.0, 1.0, 0.1, 1.0, 0.0);
        let end = gtk::Adjustment::new(1.0, 0.0, 1.0, 0.1, 1.0, 0.0);
        let slider = |adjustment: &gtk::Adjustment, name: &str| {
            let scale = gtk::Scale::builder().orientation(gtk::Orientation::Horizontal).adjustment(adjustment).draw_value(false).hexpand(true).build();
            scale.update_property(&[gtk::accessible::Property::Label(name)]);
            scale
        };
        let start_scale = slider(&start, &t!("Start"));
        let end_scale = slider(&end, &t!("End"));

        let times = ui::hbox(12, &["trim-times"]);
        times.set_direction(gtk::TextDirection::Ltr);
        let from = ui::line("0:00", &["trim-time"]);
        let kept = ui::line("", &["trim-kept"]);
        kept.set_hexpand(true);
        kept.set_xalign(0.5);
        let to = ui::line("0:00", &["trim-time"]);
        times.append(&from);
        times.append(&kept);
        times.append(&to);

        let status = ui::label("", &["trim-status"]);
        status.set_visible(false);
        let actions = ui::hbox(8, &["trim-actions"]);
        actions.set_halign(gtk::Align::End);
        let show = gtk::Button::with_label(&t!("Show in Files"));
        show.set_visible(false);
        let gif = gtk::Button::with_label(&t!("Save as GIF"));
        let webm = gtk::Button::with_label(&t!("Save as WebM"));
        webm.add_css_class("suggested-action");
        actions.append(&show);
        actions.append(&gif);
        actions.append(&webm);

        body.append(&title);
        body.append(&preview);
        body.append(&film);
        body.append(&start_scale);
        body.append(&end_scale);
        body.append(&times);
        body.append(&status);
        body.append(&actions);
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&body));
        window.set_content(Some(&view));

        let work = glib::user_runtime_dir().join(format!("amethystora-trim-{}", std::process::id()));
        let trim = Rc::new_cyclic(|me| Trim {
            me: me.clone(),
            shell: shell.clone(),
            input: RefCell::new(None),
            duration: Cell::new(0.0),
            start,
            end,
            title,
            preview,
            strip,
            marks,
            from,
            kept,
            to,
            status,
            show,
            gif,
            webm,
            work,
            frame_timer: RefCell::new(None),
            busy: Cell::new(true),
            saved: RefCell::new(None),
            moving_start: Cell::new(false),
            adjusting: Cell::new(false),
            last_frame: RefCell::new(None),
        });
        trim.wire();
        trim
    }

    fn wire(self: &Rc<Self>) {
        for (adjustment, is_start) in [(&self.start, true), (&self.end, false)] {
            let weak = self.me.clone();
            adjustment.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.moved(is_start);
                }
            });
        }
        {
            let weak = self.me.clone();
            self.marks.set_draw_func(move |_, cr, width, height| {
                if let Some(this) = weak.upgrade() {
                    this.draw_marks(cr, f64::from(width), f64::from(height));
                }
            });
        }
        // On the strip, the nearer end follows the pointer
        let drag = gtk::GestureDrag::new();
        {
            let weak = self.me.clone();
            drag.connect_drag_begin(move |_, x, _| {
                if let Some(this) = weak.upgrade() {
                    let at = this.time_at(x);
                    let nearer_start = (at - this.start.value()).abs() <= (at - this.end.value()).abs();
                    this.moving_start.set(nearer_start);
                    this.put_end(nearer_start, at);
                }
            });
        }
        {
            let weak = self.me.clone();
            drag.connect_drag_update(move |gesture, dx, _| {
                if let (Some(this), Some((x, _))) = (weak.upgrade(), gesture.start_point()) {
                    let at = this.time_at(x + dx);
                    this.put_end(this.moving_start.get(), at);
                }
            });
        }
        self.marks.add_controller(drag);

        for (button, format) in [(&self.gif, Format::Gif), (&self.webm, Format::Webm)] {
            let weak = self.me.clone();
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.save(format);
                }
            });
        }
        {
            let weak = self.me.clone();
            self.show.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.show_saved();
                }
            });
        }
        // The frames go with the window
        let work = self.work.clone();
        self.shell.app.connect_shutdown(move |_| {
            let _ = std::fs::remove_dir_all(&work);
        });
        self.set_busy(true);
    }

    fn time_at(&self, x: f64) -> f64 {
        let width = f64::from(self.marks.width()).max(1.0);
        (x / width).clamp(0.0, 1.0) * self.duration.get()
    }

    fn put_end(&self, is_start: bool, at: f64) {
        if is_start { self.start.set_value(at) } else { self.end.set_value(at) }
    }

    // An end moved: held apart from the other and inside the recording, the times said, the frame there asked for
    fn moved(&self, is_start: bool) {
        if self.adjusting.get() {
            return;
        }
        let (start, end) = cut::clamp_ends(self.start.value(), self.end.value(), self.duration.get(), is_start);
        self.adjusting.set(true);
        self.start.set_value(start);
        self.end.set_value(end);
        self.adjusting.set(false);
        self.say_times();
        self.marks.queue_draw();
        self.moving_start.set(is_start);
        self.frame_soon(if is_start { start } else { end });
    }

    fn say_times(&self) {
        let (start, end) = (self.start.value(), self.end.value());
        self.from.set_text(&cut::clock(start));
        self.to.set_text(&cut::clock(end));
        self.kept.set_text(&t!("kept {time}", time = cut::clock(end - start)));
    }

    // The part kept clear, the rest of the strip shaded, a bar at each end
    fn draw_marks(&self, cr: &gtk::cairo::Context, width: f64, height: f64) {
        let duration = self.duration.get();
        if duration <= 0.0 {
            return;
        }
        let x1 = self.start.value() / duration * width;
        let x2 = self.end.value() / duration * width;
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.55);
        cr.rectangle(0.0, 0.0, x1, height);
        cr.rectangle(x2, 0.0, width - x2, height);
        let _ = cr.fill();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        for x in [x1, x2] {
            cr.rectangle((x - 3.0).clamp(0.0, width - 6.0), 0.0, 6.0, height);
        }
        cr.rectangle(x1, 0.0, (x2 - x1).max(0.0), 2.5);
        cr.rectangle(x1, height - 2.5, (x2 - x1).max(0.0), 2.5);
        let _ = cr.fill();
    }

    fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        let ready = !busy && self.duration.get() > 0.0;
        self.gif.set_sensitive(ready);
        self.webm.set_sensitive(ready);
    }

    fn say(&self, text: &str) {
        self.status.set_text(text);
        self.status.set_visible(!text.is_empty());
    }

    // ---- The recording ----

    fn load(&self, path: PathBuf) {
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        self.title.set_text(&t!("Trim · {name}", name = name));
        self.input.replace(Some(path.clone()));
        if let Err(e) = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&self.work) {
            self.say(&t!("This recording could not be opened: {why}", why = e.to_string()));
            return;
        }
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let probed = process::run_owned(FFPROBE, &cut::probe_args(&path)).await;
            let Some(this) = weak.upgrade() else { return };
            let Some(duration) = cut::parse_duration(&probed.stdout).filter(|_| probed.ok()) else {
                this.say(&t!("This recording could not be read."));
                return;
            };
            this.duration.set(duration);
            for adjustment in [&this.start, &this.end] {
                adjustment.set_upper(duration);
                adjustment.set_step_increment((duration / 100.0).max(0.1));
                adjustment.set_page_increment((duration / 10.0).max(1.0));
            }
            this.adjusting.set(true);
            this.start.set_value(0.0);
            this.end.set_value(duration);
            this.adjusting.set(false);
            this.say_times();
            this.set_busy(false);
            this.marks.queue_draw();
            this.make_strip(&path, duration).await;
            this.frame_soon(0.0);
        });
    }

    async fn make_strip(&self, input: &Path, duration: f64) {
        let out = self.work.join("strip.png");
        let made = process::run_with(FFMPEG, &as_str(&cut::strip_args(input, duration, &out)), Options { timeout: FRAME_TIMEOUT, ..Default::default() }).await;
        if made.ok() {
            self.strip.set_filename(Some(&out));
        }
    }

    // The frame at an end, once it has rested a moment
    fn frame_soon(&self, at: f64) {
        if let Some(source) = self.frame_timer.borrow_mut().take() {
            source.remove();
        }
        let weak = self.me.clone();
        let source = glib::timeout_add_local_once(SETTLE, move || {
            let Some(this) = weak.upgrade() else { return };
            this.frame_timer.borrow_mut().take();
            let Some(input) = this.input.borrow().clone() else { return };
            let weak = this.me.clone();
            glib::spawn_future_local(async move {
                let Some(this) = weak.upgrade() else { return };
                // A file of its own each time, so that the picture is read again
                let out = this.work.join(format!("frame-{}.png", glib::monotonic_time()));
                let made = process::run_with(FFMPEG, &as_str(&cut::frame_args(&input, at, &out)), Options { timeout: FRAME_TIMEOUT, ..Default::default() }).await;
                if made.ok() {
                    this.preview.set_filename(Some(&out));
                    // The frame before is shown no more
                    if let Some(before) = this.last_frame.replace(Some(out)) {
                        let _ = std::fs::remove_file(before);
                    }
                }
            });
        });
        self.frame_timer.replace(Some(source));
    }

    // ---- Saving ----

    fn save(&self, format: Format) {
        let Some(input) = self.input.borrow().clone() else { return };
        if self.busy.get() {
            return;
        }
        let out = cut::output_path(&input, format, |path| path.exists());
        let (start, end) = (self.start.value(), self.end.value());
        self.set_busy(true);
        self.show.set_visible(false);
        self.say(&t!("Saving…"));
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let made = process::run_with(FFMPEG, &as_str(&cut::cut_args(&input, start, end, format, &out)), Options { timeout: CUT_TIMEOUT, ..Default::default() }).await;
            let Some(this) = weak.upgrade() else { return };
            this.set_busy(false);
            if made.ok() && out.exists() {
                let name = out.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                this.say(&t!("Saved as {name}", name = name));
                this.saved.replace(Some(out));
                this.show.set_visible(true);
            } else {
                // A part written before it failed is not kept as if it were whole
                let _ = std::fs::remove_file(&out);
                this.say(&t!("It could not be saved: {why}", why = made.last_error_line()));
            }
        });
    }

    fn show_saved(&self) {
        let Some(saved) = self.saved.borrow().clone() else { return };
        let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(&saved)));
        launcher.open_containing_folder(Some(&self.shell.window), None::<&gio::Cancellable>, |_| {});
    }
}

fn as_str(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

impl App for Trim {
    // amethystora-trim [--] FILE: the recording, as an absolute path to a file
    fn open(&self, args: &[String]) {
        if self.input.borrow().is_some() {
            return;
        }
        let given = args.iter().find(|arg| arg.as_str() != "--").cloned();
        let path = given.as_deref().map(Path::new).and_then(|path| std::fs::canonicalize(path).ok()).filter(|path| path.is_file());
        match path {
            Some(path) => self.load(path),
            None => self.say(&t!("Open a recording to trim it: amethystora-trim FILE")),
        }
    }
}
