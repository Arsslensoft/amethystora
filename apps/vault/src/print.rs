//! Printing what has to go on paper: the recovery key, backup codes and transfer codes, through GTK's own dialog, asynchronously. Nothing
//! is written to a file of ours; what the printing system keeps is the person's to choose.

use amethystora_ui::gtk;
use gtk::cairo;
use gtk::prelude::*;
use zeroize::Zeroizing;

use crate::qr::Modules;

fn lines(cairo: &cairo::Context, top: f64, size: f64, face: &str, weight: cairo::FontWeight, text: &str) -> f64 {
    cairo.select_font_face(face, cairo::FontSlant::Normal, weight);
    cairo.set_font_size(size);
    let mut y = top;
    for line in text.lines() {
        y += size * 1.4;
        cairo.move_to(0.0, y);
        let _ = cairo.show_text(line);
    }
    y
}

fn start(operation: &gtk::PrintOperation) {
    operation.set_allow_async(true);
    let _ = operation.run(gtk::PrintOperationAction::PrintDialog, None::<&gtk::Window>);
}

/// A heading, then text in a fixed face
pub fn text(heading: &str, body: &str) {
    let operation = gtk::PrintOperation::new();
    operation.set_n_pages(1);
    let (heading, body) = (heading.to_string(), Zeroizing::new(body.to_string()));
    operation.connect_draw_page(move |_, context, _| {
        let cairo = context.cairo_context();
        cairo.set_source_rgb(0.0, 0.0, 0.0);
        let below = lines(&cairo, 0.0, 14.0, "Sans", cairo::FontWeight::Bold, &heading);
        lines(&cairo, below + 12.0, 13.0, "Monospace", cairo::FontWeight::Normal, &body);
    });
    start(&operation);
}

/// One QR code to a page, each under a heading
pub fn codes(pages: Vec<(String, Modules)>) {
    let operation = gtk::PrintOperation::new();
    operation.set_n_pages(pages.len().max(1) as i32);
    operation.connect_draw_page(move |_, context, page| {
        let Some((heading, modules)) = pages.get(page as usize) else { return };
        let cairo = context.cairo_context();
        cairo.set_source_rgb(0.0, 0.0, 0.0);
        let top = lines(&cairo, 0.0, 12.0, "Sans", cairo::FontWeight::Bold, heading) + 24.0;
        let side = modules.side().max(1) as f64;
        let size = context.width().min(context.height() - top).max(1.0);
        let cell = size / side;
        for row in 0..modules.side() {
            for column in 0..modules.side() {
                if modules.dark(column, row) {
                    cairo.rectangle(column as f64 * cell, top + row as f64 * cell, cell + 0.2, cell + 0.2);
                }
            }
        }
        let _ = cairo.fill();
    });
    start(&operation);
}
