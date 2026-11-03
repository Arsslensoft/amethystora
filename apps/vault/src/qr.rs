//! Drawing a QR code: `qrencode` makes it, the text on its standard input and never on its command line, and the window draws the
//! modules itself, black on white in every theme, so that nothing is ever written to a file. Until it is asked for, and again after,
//! a blurred stand-in that means nothing takes its place.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use amethystora_ui::{gdk, gio, glib, graphene, gtk};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use zeroize::{Zeroize, Zeroizing};

/// The modules of a QR code, dark or light, wiped when they go
#[derive(Clone, Default)]
pub struct Modules {
    side: usize,
    cells: Vec<bool>,
}

impl Drop for Modules {
    fn drop(&mut self) {
        self.cells.zeroize();
    }
}

impl Modules {
    pub fn side(&self) -> usize {
        self.side
    }

    pub fn dark(&self, column: usize, row: usize) -> bool {
        self.cells.get(row * self.side + column).copied().unwrap_or(false)
    }

    /// `qrencode -t ASCII`'s drawing: two characters a module, `##` for a dark one
    fn from_ascii(text: &str) -> Option<Modules> {
        let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
        let side = lines.first()?.len() / 2;
        if side == 0 || lines.len() != side {
            return None;
        }
        let mut cells = Vec::with_capacity(side * side);
        for line in lines {
            if line.len() != side * 2 {
                return None;
            }
            cells.extend(line.as_bytes().chunks(2).map(|pair| pair[0] == b'#'));
        }
        Some(Modules { side, cells })
    }

    /// The same every time, and no code: what is blurred until the real one is asked for
    fn stand_in() -> Modules {
        const SIDE: usize = 29;
        let mut seed = 7u32;
        let mut cells = Vec::with_capacity(SIDE * SIDE);
        let finder = |x: usize, y: usize, a: usize, b: usize| x >= a && x < a + 7 && y >= b && y < b + 7;
        for y in 0..SIDE {
            for x in 0..SIDE {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fff_ffff;
                let mut on = (seed >> 16) & 1 == 1;
                for (a, b) in [(0, 0), (22, 0), (0, 22)] {
                    if finder(x, y, a, b) {
                        let (i, j) = (x - a, y - b);
                        on = i == 0 || i == 6 || j == 0 || j == 6 || ((2..=4).contains(&i) && (2..=4).contains(&j));
                    }
                }
                cells.push(on);
            }
        }
        Modules { side: SIDE, cells }
    }
}

/// Blocks: `text` as a QR code
fn encode_now(text: &[u8]) -> Option<Modules> {
    let mut child = Command::new("qrencode").args(["-t", "ASCII", "-m", "0", "-l", "M", "-o", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    child.stdin.take()?.write_all(text).ok()?;
    let mut drawing = Zeroizing::new(String::new());
    child.stdout.take()?.read_to_string(&mut drawing).ok()?;
    if !child.wait().ok()?.success() {
        return None;
    }
    Modules::from_ascii(&drawing)
}

/// `text` as a QR code, off the window's thread
pub async fn encode(text: Zeroizing<Vec<u8>>) -> Option<Modules> {
    gio::spawn_blocking(move || encode_now(&text)).await.ok().flatten()
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Code {
        pub modules: RefCell<Option<Modules>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Code {
        const NAME: &'static str = "AmethystoraVaultQr";
        type Type = super::Code;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("qrcode");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for Code {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_overflow(gtk::Overflow::Hidden);
            self.obj().set_halign(gtk::Align::Center);
        }
    }

    impl WidgetImpl for Code {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let side = (232.0 * amethystora_ui::icon::zoom()) as i32;
            (side, side, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (width, height) = (widget.width() as f32, widget.height() as f32);
            let side = width.min(height);
            snapshot.append_color(&gdk::RGBA::WHITE, &graphene::Rect::new(0.0, 0.0, width, height));
            let borrowed = self.modules.borrow();
            let shown = borrowed.as_ref();
            let stand_in;
            let (modules, blurred) = match shown {
                Some(modules) => (modules, false),
                None => {
                    stand_in = Modules::stand_in();
                    (&stand_in, true)
                }
            };
            if modules.side() == 0 {
                return;
            }
            // Whole pixels a module, which any camera reads best, in the middle of what is left
            let cell = (side / modules.side() as f32).floor().max(1.0);
            let drawn = cell * modules.side() as f32;
            let (x, y) = ((width - drawn) / 2.0, (height - drawn) / 2.0);
            if blurred {
                snapshot.push_blur(9.0);
            }
            let ink = gdk::RGBA::new(0.067, 0.047, 0.094, 1.0);
            for row in 0..modules.side() {
                let mut column = 0;
                while column < modules.side() {
                    if !modules.dark(column, row) {
                        column += 1;
                        continue;
                    }
                    let start = column;
                    while column < modules.side() && modules.dark(column, row) {
                        column += 1;
                    }
                    snapshot.append_color(&ink, &graphene::Rect::new(x + start as f32 * cell, y + row as f32 * cell, (column - start) as f32 * cell, cell));
                }
            }
            if blurred {
                snapshot.pop();
            }
        }
    }
}

glib::wrapper! {
    /// A QR code on white, or its stand-in
    pub struct Code(ObjectSubclass<imp::Code>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Code {
    pub fn new() -> Code {
        glib::Object::new()
    }

    /// `None` takes it off the screen
    pub fn set(&self, modules: Option<Modules>) {
        self.imp().modules.replace(modules);
        self.queue_draw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_qrencodes_drawing() {
        let modules = Modules::from_ascii("##  \n  ##\n").expect("a square");
        assert_eq!(modules.side(), 2);
        assert!(modules.dark(0, 0) && !modules.dark(1, 0) && modules.dark(1, 1));
        assert!(Modules::from_ascii("##\n##\n").is_none());
        assert!(Modules::from_ascii("").is_none());
    }
}
