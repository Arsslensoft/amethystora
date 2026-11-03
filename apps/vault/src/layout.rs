//! A column exactly as wide as the stylesheet's min-width, whatever the words in it would like: the sidebar's. Without it a line that
//! wraps asks for the width it would take unwrapped, and the sidebar grows with the page beside it.

use amethystora_ui::{glib, gtk};
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod exact {
    use super::*;

    #[derive(Default)]
    pub struct Exact;

    #[glib::object_subclass]
    impl ObjectSubclass for Exact {
        const NAME: &'static str = "AmethystoraVaultExact";
        type Type = super::Exact;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Exact {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Exact {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.obj().first_child() else { return (0, 0, -1, -1) };
            let (min, nat, base_min, base_nat) = child.measure(orientation, for_size);
            if orientation == gtk::Orientation::Horizontal { (min, min, -1, -1) } else { (min, nat, base_min, base_nat) }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.obj().first_child() {
                child.size_allocate(&gtk::Allocation::new(0, 0, width, height), baseline);
            }
        }
    }
}

glib::wrapper! {
    pub struct Exact(ObjectSubclass<exact::Exact>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Exact {
    pub fn new(child: &impl IsA<gtk::Widget>) -> Exact {
        let exact: Exact = glib::Object::new();
        child.set_parent(&exact);
        exact
    }
}
