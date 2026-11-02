//! The window every app opens: one instance per account, the session's language, the stylesheets with
//! the current theme's colours followed as it changes, and the zoom.
//!
//! Each app gives its name, its application id and its stylesheet, and builds its page in `build`; its
//! [`App`] hears when it is opened again (a second launch, with its arguments), focused, re-coloured or
//! asked to read everything again (Ctrl+R, F5).

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::i18n::{self, I18n};
use crate::palette::{self, Palette};
use crate::shape::shape_css;

/// What an app does with the window
pub trait App: 'static {
    /// The window was opened with these arguments, the program's own name left out: at start, and again
    /// each time it is launched while it is open
    fn open(&self, _args: &[String]) {}
    /// The same, with the folder the launch was made from, for an argument that is a path relative to it
    fn open_in(&self, args: &[String], _cwd: Option<std::path::PathBuf>) {
        self.open(args);
    }
    /// The launch itself, before `open_in`, for an app that reads what the launch was given on its standard input
    /// (the Shelf's drop): GLib passes a second launch's standard input to the window that is open
    fn command_line(&self, _line: &gio::ApplicationCommandLine) {}
    /// Whether a launch with these arguments shows the window; an app that answers no shows it itself when it
    /// needs the person (the Phone app's `send --now`)
    fn presents(&self, _args: &[String]) -> bool {
        true
    }
    /// The window has come to the front, from elsewhere
    fn focus(&self) {}
    /// The theme changed, or the system's colour scheme while no theme is applied
    fn palette(&self) {}
    /// Ctrl+R or F5: everything read again
    fn reload(&self) {}
    /// Ctrl+= and Ctrl+- changed the zoom: what the app sized in code is sized again
    fn zoomed(&self) {}
}

pub struct Spec {
    /// The app's name, as in /usr/lib/amethystora-<name>: security, update, ...
    pub name: &'static str,
    /// Its application id, which the .desktop file's StartupWMClass names
    pub id: &'static str,
    /// Its stylesheet, ported from the Electron page's
    pub css: &'static str,
    pub width: i32,
    pub height: i32,
    pub min_width: i32,
    pub min_height: i32,
}

/// The app's window, its application and the colours it is drawn in
pub struct Shell {
    pub app: adw::Application,
    pub window: adw::ApplicationWindow,
    spec: Spec,
    palette: RefCell<Option<Palette>>,
    zoom_level: Cell<f64>,
    tokens: gtk::CssProvider,
    base: gtk::CssProvider,
    own: gtk::CssProvider,
    monitors: RefCell<Vec<gio::FileMonitor>>,
}

impl Shell {
    /// The current theme's colours, or None before one has been applied
    pub fn palette(&self) -> Option<Palette> {
        self.palette.borrow().clone()
    }

    /// Whether the window is drawn light
    pub fn light(&self) -> bool {
        match &*self.palette.borrow() {
            Some(palette) => palette.light,
            None => !adw::StyleManager::default().is_dark(),
        }
    }

    pub fn gtk_window(&self) -> gtk::Window {
        self.window.clone().upcast()
    }

    fn restyle(&self) {
        let palette = self.palette.borrow().clone();
        let manager = adw::StyleManager::default();
        manager.set_color_scheme(match &palette {
            Some(palette) if palette.light => adw::ColorScheme::ForceLight,
            Some(_) => adw::ColorScheme::ForceDark,
            None => adw::ColorScheme::Default,
        });
        let light = self.light();
        self.tokens.load_from_string(&palette::css(palette.as_ref(), light));
        if light {
            self.window.add_css_class("light");
        } else {
            self.window.remove_css_class("light");
        }
        // The desktop style's corners are in the stylesheets as much as the colours are
        self.rescale();
    }

    fn rescale(&self) {
        let factor = 1.2f64.powf(self.zoom_level.get());
        crate::icon::set_zoom(factor);
        let shape = self.palette.borrow().as_ref().and_then(|palette| palette.shape);
        let text = text_factor();
        self.base.load_from_string(&scale_fonts(&scale_css(&shape_css(BASE_CSS, shape), factor), text));
        self.own.load_from_string(&scale_fonts(&scale_css(&shape_css(self.spec.css, shape), factor), text));
        self.window.queue_resize();
    }

    /// A CSS class on the window while it is at most `width` pixels wide, as a max-width media query was
    pub fn narrow(&self, width: f64, class: &'static str) {
        let condition = adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth, width, adw::LengthUnit::Px);
        let breakpoint = adw::Breakpoint::new(condition);
        let window = self.window.clone();
        breakpoint.connect_apply(move |_| window.add_css_class(class));
        let window = self.window.clone();
        breakpoint.connect_unapply(move |_| window.remove_css_class(class));
        self.window.add_breakpoint(breakpoint);
    }

    /// Ctrl+= and Ctrl+- step the zoom by half a level, Ctrl+0 resets it, as Chromium's did
    pub fn zoom(&self, step: f64) {
        let level = if step == 0.0 { 0.0 } else { (self.zoom_level.get() + step).clamp(-3.0, 4.5) };
        self.zoom_level.set(level);
        self.rescale();
    }
}

const BASE_CSS: &str = include_str!("style.css");

/// GNOME's text size (ame desktop text-size, Settings' Large Text), as GTK has it: the fonts' resolution over 96 dpi.
/// GTK grows text given in points by it, but not text given in px, which is what the apps' stylesheets use.
pub(crate) fn text_factor() -> f64 {
    gtk::Settings::default()
        .map(|settings| settings.gtk_xft_dpi())
        .filter(|dpi| *dpi > 0)
        .map_or(1.0, |dpi| (f64::from(dpi) / 1024.0 / 96.0).clamp(0.5, 3.0))
}

/// The stylesheet with every px of a font-size multiplied by the text size, and nothing else
fn scale_fonts(css: &str, factor: f64) -> String {
    if (factor - 1.0).abs() < 1e-9 {
        return css.to_string();
    }
    const NAME: &str = "font-size";
    let mut out = String::with_capacity(css.len());
    let mut at = 0;
    while let Some(found) = css[at..].find(NAME) {
        let start = at + found;
        let after = start + NAME.len();
        // The property itself, not a longer name that ends so (a custom property's name keeps its own value)
        let before = css[..start].chars().next_back();
        let rest = css[after..].trim_start_matches([' ', '\t']);
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || !rest.starts_with(':') {
            out.push_str(&css[at..after]);
            at = after;
            continue;
        }
        let value_start = css.len() - rest.len() + 1;
        let value_end = css[value_start..].find([';', '}', '\n']).map_or(css.len(), |i| value_start + i);
        out.push_str(&css[at..value_start]);
        out.push_str(&scale_css(&css[value_start..value_end], factor));
        at = value_end;
    }
    out.push_str(&css[at..]);
    out
}

/// The stylesheet with every length in px multiplied by the zoom
fn scale_css(css: &str, factor: f64) -> String {
    if (factor - 1.0).abs() < 1e-9 {
        return css.to_string();
    }
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let c = bytes[at];
        let starts_number = (c.is_ascii_digit() || c == b'.')
            && (at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'-' || bytes[at - 1] == b'#' || bytes[at - 1] == b'_'));
        if starts_number {
            let start = at;
            while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                at += 1;
            }
            if css[at..].starts_with("px") {
                let value: f64 = css[start..at].parse().unwrap_or(0.0);
                let scaled = (value * factor * 100.0).round() / 100.0;
                out.push_str(&format!("{scaled}px"));
                at += 2;
            } else {
                out.push_str(&css[start..at]);
            }
            continue;
        }
        let ch = css[at..].chars().next().unwrap_or(' ');
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

/// The desktop's own stylesheet, ~/.config/gtk-4.0/gtk.css, is where 20-gtk-apps.sh imports a whole GTK
/// theme for the libadwaita apps; it would repaint these windows away from their design, which the
/// Electron windows never read. GTK finds it through g_get_user_config_dir(), which GLib works out once
/// and keeps, so that is worked out here with a folder that has none, and XDG_CONFIG_HOME is put back for
/// everything else straight away, the programs the window starts among them. Nothing in these apps
/// asks GLib for that folder: what they read under ~/.config they find themselves (palette::current).
fn keep_desktop_stylesheet_out() {
    let real = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: nothing else runs yet, so no other thread reads the environment
    unsafe { std::env::set_var("XDG_CONFIG_HOME", "/usr/lib/amethystora-apps/no-config") };
    let _ = glib::user_config_dir();
    // SAFETY: as above
    unsafe {
        match real {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
}

#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigAppFontAddFile(config: *mut std::ffi::c_void, file: *const std::ffi::c_char) -> i32;
}

/// Quicksand, the face of the wordmark, which the image has no system copy of: the windows' own
pub const FONT: &str = "/usr/lib/amethystora-apps/fonts/QuicksandVariable.ttf";

fn add_font(file: &str) {
    if !Path::new(file).exists() {
        return;
    }
    if let Ok(file) = std::ffi::CString::new(file) {
        // SAFETY: a NUL-terminated path, and the current configuration, which fontconfig creates if need be
        unsafe {
            FcConfigAppFontAddFile(std::ptr::null_mut(), file.as_ptr());
        }
    }
}

/// Runs the app until its window is closed
pub fn run<A: App>(spec: Spec, build: impl Fn(Rc<Shell>) -> Rc<A> + 'static) -> glib::ExitCode {
    keep_desktop_stylesheet_out();
    add_font(FONT);
    let language = I18n::load(&i18n::catalogs(spec.name), &|name| std::env::var(name).ok());
    let rtl = language.rtl;
    language.install();

    let app = adw::Application::builder()
        .application_id(spec.id)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    // --version is answered here, before GTK starts, so that the build can see the program run with no
    // display (20-tests.sh). It is read by hand: an app that registers an option of its own makes GLib
    // refuse every option it does not know, and the apps take theirs (Backups' --at=POINT) as arguments.
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("amethystora-{} {}", spec.name, env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    let spec = RefCell::new(Some(spec));
    let state: Rc<RefCell<Option<(Rc<Shell>, Rc<A>)>>> = Rc::new(RefCell::new(None));
    let build = Rc::new(build);

    app.connect_startup(move |_| {
        // A selectable label keeps its text unselected when it gets the focus, as text on a page does
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_label_select_on_focus(false);
        }
        if rtl {
            gtk::Widget::set_default_direction(gtk::TextDirection::Rtl);
        }
    });

    app.connect_command_line(move |app, command_line| {
        let args: Vec<String> = command_line
            .arguments()
            .iter()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        if state.borrow().is_none() {
            let Some(spec) = spec.borrow_mut().take() else { return glib::ExitCode::FAILURE };
            let (shell, own) = open_window(app, spec, &build);
            state.replace(Some((shell, own)));
        }
        let (shell, own) = state.borrow().clone().expect("the window was just made");
        own.command_line(command_line);
        own.open_in(&args, command_line.cwd());
        if own.presents(&args) {
            shell.window.present();
        }
        glib::ExitCode::SUCCESS
    });

    let args: Vec<String> = std::env::args().collect();
    app.run_with_args(&args)
}

fn open_window<A: App>(app: &adw::Application, spec: Spec, build: &Rc<impl Fn(Rc<Shell>) -> Rc<A> + 'static>) -> (Rc<Shell>, Rc<A>) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .default_width(spec.width)
        .default_height(spec.height)
        .build();
    window.set_size_request(spec.min_width, spec.min_height);
    window.add_css_class("amethystora");
    let display = gtk::prelude::WidgetExt::display(&window);
    let tokens = gtk::CssProvider::new();
    let base = gtk::CssProvider::new();
    let own = gtk::CssProvider::new();
    // Above the desktop's own stylesheet (USER), which libadwaita apps take their theme from
    for (offset, provider) in [(1, &tokens), (2, &base), (3, &own)] {
        gtk::style_context_add_provider_for_display(&display, provider, gtk::STYLE_PROVIDER_PRIORITY_USER + offset);
    }
    let shell = Rc::new(Shell {
        app: app.clone(),
        window: window.clone(),
        spec,
        palette: RefCell::new(palette::read()),
        zoom_level: Cell::new(0.0),
        tokens,
        base,
        own,
        monitors: RefCell::new(Vec::new()),
    });
    shell.restyle();
    // GNOME's text size, changed while the window is open
    if let Some(settings) = gtk::Settings::default() {
        let weak = Rc::downgrade(&shell);
        settings.connect_gtk_xft_dpi_notify(move |_| {
            if let Some(shell) = weak.upgrade() {
                shell.rescale();
            }
        });
    }
    let own_app = build(shell.clone());
    crate::where_is::offer(app, shell.window.upcast_ref(), i18n::catalogs(shell.spec.name));
    crate::frame::start_controls(&window);

    // With no theme applied, the system's colour scheme decides
    let weak = Rc::downgrade(&shell);
    let app_weak = Rc::downgrade(&own_app);
    adw::StyleManager::default().connect_dark_notify(move |_| {
        if let (Some(shell), Some(own)) = (weak.upgrade(), app_weak.upgrade()) {
            if shell.palette.borrow().is_none() {
                shell.restyle();
                own.palette();
            }
        }
    });

    // A theme switch replaces current/theme and rewrites current/theme.name, and a workspace with an accent of its own
    // names it in a file of Amethystora UI's: the window follows both
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let weak = Rc::downgrade(&shell);
    let app_weak = Rc::downgrade(&own_app);
    let changed = Rc::new(move || {
        if let Some(source) = pending.borrow_mut().take() {
            source.remove();
        }
        let weak = weak.clone();
        let app_weak = app_weak.clone();
        let done = pending.clone();
        let source = glib::timeout_add_local_once(Duration::from_millis(300), move || {
            done.borrow_mut().take();
            if let (Some(shell), Some(own)) = (weak.upgrade(), app_weak.upgrade()) {
                let read = palette::read();
                if *shell.palette.borrow() != read {
                    shell.palette.replace(read);
                    shell.restyle();
                    own.palette();
                }
            }
        });
        pending.replace(Some(source));
    });
    let mut monitors = Vec::new();
    // A file in a folder that is not there yet is watched by polling: the folder is made first, so that nothing polls
    if let Some(folder) = palette::workspace_accent_file().parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let watched = [
        gio::File::for_path(palette::current()).monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE),
        gio::File::for_path(palette::workspace_accent_file()).monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE),
    ];
    for monitor in watched.into_iter().flatten() {
        let changed = changed.clone();
        monitor.connect_changed(move |_, _, _, _| changed());
        monitors.push(monitor);
    }
    shell.monitors.replace(monitors);

    // Back from elsewhere
    let app_weak = Rc::downgrade(&own_app);
    window.connect_is_active_notify(move |window| {
        if window.is_active() {
            if let Some(own) = app_weak.upgrade() {
                own.focus();
            }
        }
    });

    // Ctrl+R and F5 read everything again; Ctrl+=, Ctrl+- and Ctrl+0 zoom
    let keys = gtk::ShortcutController::new();
    keys.set_scope(gtk::ShortcutScope::Global);
    let app_weak = Rc::downgrade(&own_app);
    let reload = gtk::CallbackAction::new(move |_, _| {
        if let Some(own) = app_weak.upgrade() {
            own.reload();
        }
        glib::Propagation::Stop
    });
    for trigger in ["<Control>r", "F5"] {
        keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger), Some(reload.clone())));
    }
    for (triggers, step) in [(&["<Control>equal", "<Control>plus", "<Control>KP_Add"][..], 0.5), (&["<Control>minus", "<Control>KP_Subtract"][..], -0.5), (&["<Control>0", "<Control>KP_0"][..], 0.0)] {
        let weak = Rc::downgrade(&shell);
        let app_weak = Rc::downgrade(&own_app);
        let action = gtk::CallbackAction::new(move |_, _| {
            if let (Some(shell), Some(own)) = (weak.upgrade(), app_weak.upgrade()) {
                shell.zoom(step);
                own.zoomed();
            }
            glib::Propagation::Stop
        });
        for trigger in triggers {
            keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger), Some(action.clone())));
        }
    }
    window.add_controller(keys);
    (shell, own_app)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Plain;
    impl App for Plain {}

    #[test]
    fn a_launch_shows_the_window_unless_the_app_says_otherwise() {
        assert!(Plain.presents(&["capture".to_string()]));
    }

    #[test]
    fn text_size_scales_font_sizes_only() {
        let css = ".a { font-size: 12px; margin: 10px; }\n.b{font-size:1.5em;padding:4px}\n.c { --x-font-size: 8px; font-size: 20px }";
        assert_eq!(
            scale_fonts(css, 1.5),
            ".a { font-size: 18px; margin: 10px; }\n.b{font-size:1.5em;padding:4px}\n.c { --x-font-size: 8px; font-size: 30px }"
        );
        assert_eq!(scale_fonts(css, 1.0), css);
    }

    #[test]
    fn scales_pixels_only() {
        assert_eq!(scale_css("a { margin: 10px 0.5px; color: #112px; width: 2em; }", 2.0), "a { margin: 20px 1px; color: #112px; width: 2em; }");
        assert_eq!(scale_css("--x-2px: 3px", 1.5), "--x-2px: 4.5px");
    }
}
