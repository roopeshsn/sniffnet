#![allow(clippy::unreadable_literal)]

//! Nord theme
//! <https://www.nordtheme.com/docs/colors-and-palettes>
use iced::color;
use once_cell::sync::Lazy;

use crate::gui::styles::types::palette::Palette;
use crate::gui::styles::types::palette_extension::PaletteExtension;

pub static NORD_DARK_PALETTE: Lazy<Palette> = Lazy::new(|| Palette {
    primary: color!(0x2e3440),      // nord0
    secondary: color!(0x88c0d0),    // nord8
    outgoing: color!(0xB48EAD),     // nord15
    starred: color!(0xebcb8b),      // nord13
    text_headers: color!(0x2e3440), // nord0
    text_body: color!(0xd8dee9),    // nord4
});

pub static NORD_DARK_PALETTE_EXTENSION: Lazy<PaletteExtension> =
    Lazy::new(|| NORD_DARK_PALETTE.generate_palette_extension());

pub static NORD_LIGHT_PALETTE: Lazy<Palette> = Lazy::new(|| Palette {
    primary: color!(0xeceff4),      // nord6
    secondary: color!(0x05e81ac),   // nord10
    outgoing: color!(0xb48ead),     // nord15
    starred: color!(0xebcb8b),      // nord13
    text_headers: color!(0xeceff4), // nord6
    text_body: color!(0x2e3440),    // nord0
});

pub static NORD_LIGHT_PALETTE_EXTENSION: Lazy<PaletteExtension> =
    Lazy::new(|| NORD_LIGHT_PALETTE.generate_palette_extension());
