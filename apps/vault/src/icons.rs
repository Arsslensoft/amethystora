//! The window's line icons, 24 units square, as the boards draw them: strokes in the widget's colour.

use amethystora_ui::icon::{Icon, IconData, Shape, EMPTY};

pub const STROKE: f32 = 1.8;

pub fn icon(name: &str, size: i32) -> Icon {
    Icon::new(data(name), size, STROKE)
}

macro_rules! icons {
    ($($name:literal $constant:ident [$($d:literal),+ $(,)?];)*) => {
        $(static $constant: IconData = IconData { view: 24.0, shapes: &[$(Shape { d: $d, fill: false, stroke: true, width: None }),+] };)*

        pub fn data(name: &str) -> &'static IconData {
            match name {
                $($name => &$constant,)*
                _ => &EMPTY,
            }
        }
    };
}

icons! {
    "key" KEY ["M4 15a4 4 0 1 0 8 0a4 4 0 1 0 -8 0z", "M11 12l9-9M16 7l3 3M13 10l2 2"];
    "lock" LOCK ["M7 11h10a2 2 0 0 1 2 2v6a2 2 0 0 1 -2 2h-10a2 2 0 0 1 -2 -2v-6a2 2 0 0 1 2 -2z", "M8 11V7a4 4 0 0 1 8 0v4"];
    "codes" CODES ["M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01"];
    "cert" CERT ["M7 9a5 5 0 1 0 10 0a5 5 0 1 0 -10 0z", "M9 13.5 7.5 21 12 18.5 16.5 21 15 13.5"];
    "pass" PASS ["M6 7h12a3 3 0 0 1 3 3v4a3 3 0 0 1 -3 3h-12a3 3 0 0 1 -3 -3v-4a3 3 0 0 1 3 -3z", "M8 12h.01M12 12h.01M16 12h.01"];
    "search" SEARCH ["M4 11a7 7 0 1 0 14 0a7 7 0 1 0 -14 0z", "m20 20-3.5-3.5"];
    "copy" COPY ["M11 9h7a2 2 0 0 1 2 2v7a2 2 0 0 1 -2 2h-7a2 2 0 0 1 -2 -2v-7a2 2 0 0 1 2 -2z", "M5 15V6a2 2 0 0 1 2-2h8"];
    "eye" EYE ["M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z", "M9 12a3 3 0 1 0 6 0a3 3 0 1 0 -6 0z"];
    "plus" PLUS ["M12 5v14M5 12h14"];
    "finger" FINGER ["M12 11v4M8.5 7.5A6 6 0 0 1 18 12v2M6 12a6 6 0 0 1 .6-2.6M6 15v1a6 6 0 0 0 1.5 4M9 13v3a4 4 0 0 0 1 2.6M15 13v3"];
    "usb" USB ["M9 2h6a1 1 0 0 1 1 1v5a1 1 0 0 1 -1 1h-6a1 1 0 0 1 -1 -1v-5a1 1 0 0 1 1 -1z", "M9 9h6a3 3 0 0 1 3 3v7a3 3 0 0 1 -3 3h-6a3 3 0 0 1 -3 -3v-7a3 3 0 0 1 3 -3z", "M10 15.500a2 2 0 1 0 4 0a2 2 0 1 0 -4 0z"];
    "warn" WARN ["M12 4 2 20h20zM12 10v4M12 17h.01"];
    "refresh" REFRESH ["M4 12a8 8 0 0 1 14-5l2 2M20 5v4h-4M20 12a8 8 0 0 1-14 5l-2-2M4 19v-4h4"];
    "down" DOWN ["M12 4v12M7 11l5 5 5-5M5 20h14"];
    "up" UP ["M12 16V4M7 9l5-5 5 5M5 20h14"];
    "check" CHECK ["m5 12 5 5 9-10"];
    "bolt" BOLT ["M13 3 5 14h6l-1 7 8-11h-6z"];
    "back" BACK ["M15 5l-7 7 7 7"];
    "trash" TRASH ["M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6"];
    "edit" EDIT ["M4 20h4L19 9l-4-4L4 16zM13.500 6.500l4 4"];
    "book" BOOK ["M5 4h11a3 3 0 0 1 3 3v13H8a3 3 0 0 1-3-3zM5 17a3 3 0 0 1 3-3h11"];
    "print" PRINT ["M7 8V3h10v5M7 17H4v-7a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v7h-3M7 14h10v7H7z"];
    "camera" CAMERA ["M4 8h3l2-3h6l2 3h3a1 1 0 0 1 1 1v9a1 1 0 0 1 -1 1h-16a1 1 0 0 1 -1 -1v-9a1 1 0 0 1 1 -1z", "M8.500 13a3.500 3.500 0 1 0 7 0a3.500 3.500 0 1 0 -7 0z"];
    "screen" SCREEN ["M4 4h16a1 1 0 0 1 1 1v10a1 1 0 0 1 -1 1h-16a1 1 0 0 1 -1 -1v-10a1 1 0 0 1 1 -1zM9 20h6M12 16v4"];
    "picture" PICTURE ["M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-12a2 2 0 0 1 2 -2zM3 16l5-5 4 4 3-3 6 6M15 8h.01"];
    "qr" QR ["M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h2M20 14v2M14 18v2h2M18 18h2v2"];
    "vaults" VAULTS ["M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-12a2 2 0 0 1 2 -2z", "M8 12a4 4 0 1 0 8 0a4 4 0 1 0 -8 0zM12 8v8M8 12h8"];
    "globe" GLOBE ["M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z", "M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"];
    "shield" SHIELD ["M12 3l8 3v6c0 5-3.500 8-8 9-4.500-1-8-4-8-9V6z"];
    "folder" FOLDER ["M3 6a1 1 0 0 1 1-1h5l2 3h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z"];
}
