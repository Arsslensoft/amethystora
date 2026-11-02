//! The wordmark of the boot splash and the login screen, as logoHtml in branding/generate.mjs sets it: the
//! stone as the "a" of "methystora" in Quicksand Bold, glowing, then the window's own name. Every size is
//! the generator's, in em, so it is the same mark at any size.

use gtk::prelude::*;

use crate::gem::Gem;

/// "Amethystora <product>" as one image to a screen reader, `em` pixels being its lettering's size
pub fn brand(product: &str, em: f32) -> gtk::Box {
    let brand = in_a_line(product, em);
    // A sidebar has room beside the wordmark for about eight letters at the desktop's text size: a longer name, as
    // German's and Arabic's are, goes under it, or the sidebar would clip it
    if product.chars().count() as f64 * crate::app::text_factor() > 8.0 {
        brand.set_orientation(gtk::Orientation::Vertical);
        brand.add_css_class("stacked");
        for part in [brand.first_child(), brand.last_child()].into_iter().flatten() {
            part.set_halign(gtk::Align::Start);
        }
    }
    brand
}

/// The same on one line, where the room is not a sidebar's
pub fn in_a_line(product: &str, em: f32) -> gtk::Box {
    let title = format!("Amethystora {product}");
    let brand = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .accessible_role(gtk::AccessibleRole::Img)
        .css_classes(["brand"])
        .valign(gtk::Align::Center)
        .build();
    brand.update_property(&[gtk::accessible::Property::Label(&title)]);
    // The mark itself reads left to right in every language
    let wordmark = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["wordmark"])
        .valign(gtk::Align::Baseline)
        .build();
    wordmark.set_direction(gtk::TextDirection::Ltr);
    // Its gap to the letters, and theirs to the window's name, are em in the stylesheet (style.css)
    wordmark.append(&Gem::mark(em));
    let letters = gtk::Label::builder().label("methystora").valign(gtk::Align::Baseline).build();
    wordmark.append(&letters);
    brand.append(&wordmark);
    let name = gtk::Label::builder().label(product).css_classes(["product"]).valign(gtk::Align::Baseline).build();
    brand.append(&name);
    brand.set_baseline_position(gtk::BaselinePosition::Center);
    brand
}
