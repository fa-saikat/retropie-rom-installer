//! Flat color palette lifted directly from the approved draft mockup.
//! Kept as plain u32 (0xRRGGBB) so both GPUI's `rgb()` and any future
//! theming can consume them without conversion.

pub const WINDOW_BG: u32 = 0x0f1419;
pub const SIDEBAR_TEXT: u32 = 0x8a8fa3;
pub const SIDEBAR_ITEM_SELECTED_BG: u32 = 0x1c2033;

pub const CARD_BG: u32 = 0x171c2b;
pub const CARD_BORDER: u32 = 0x262c42;
pub const CARD_ICON_BG: u32 = 0x26215c;

pub const TEXT_PRIMARY: u32 = 0xfffdfa;
pub const TEXT_SECONDARY: u32 = 0x8a8fa3;
pub const TEXT_MUTED: u32 = 0x5f6478;

pub const ACCENT: u32 = 0x7f77dd;
pub const ACCENT_LIGHT: u32 = 0xafa9ec;

pub const DROPZONE_BORDER: u32 = 0x3a4060;

pub const DANGER: u32 = 0xe0554f;
pub const DANGER_BG: u32 = 0x2a1518;
