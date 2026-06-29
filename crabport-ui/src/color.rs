// Base
pub const BG_BASE: u32 = 0x1e1e2e;
pub const BG_SIDEBAR: u32 = 0x181825;
pub const BG_TAB_BAR: u32 = 0x181825;

// Border
pub const BORDER: u32 = 0x313244;

// Surface
pub const SURFACE_HOVER: u32 = 0x24273a;
pub const SURFACE_ACTIVE: u32 = 0x313244;

// Text
pub const TEXT_PRIMARY: u32 = 0xcdd6f4;
pub const TEXT_MUTED: u32 = 0x585b70;

// Tab Button (subtle, blends with sidebar/tabbar background)
pub const TAB_BTN_BG: u32 = 0x181825;
pub const TAB_BTN_BG_HOVER: u32 = 0x24273a;
pub const TAB_BTN_BG_SELECTED: u32 = 0x313244;
pub const TAB_BTN_BG_PRESSED: u32 = 0x45475a;
pub const TAB_BTN_BG_DISABLED: u32 = 0x11111b;
pub const TAB_BTN_BORDER: u32 = 0x313244;
pub const TAB_BTN_TEXT_DISABLED: u32 = 0x45475a;

// Button (prominent, stands out from background)
pub const BTN_BG: u32 = 0x313244;
pub const BTN_BG_HOVER: u32 = 0x45475a;
pub const BTN_BG_SELECTED: u32 = 0x585b70;
pub const BTN_BG_PRESSED: u32 = 0x6c7086;
pub const BTN_BG_DISABLED: u32 = 0x1e1e2e;
pub const BTN_BORDER: u32 = 0x45475a;
pub const BTN_TEXT_DISABLED: u32 = 0x585b70;

// Button — Primary (blue accent)
pub const BTN_PRIMARY_BG: u32 = 0x3b82f6;
pub const BTN_PRIMARY_BG_HOVER: u32 = 0x2563eb;
pub const BTN_PRIMARY_BG_SELECTED: u32 = 0x1d4ed8;
pub const BTN_PRIMARY_BG_DISABLED: u32 = 0x1e3a5f;
pub const BTN_PRIMARY_BORDER: u32 = 0x3b82f6;
pub const BTN_PRIMARY_TEXT_DISABLED: u32 = 0x93c5fd;

// Button — Ghost (transparent, icon-only friendly)
// Default bg is fully transparent (alpha = 0x00) so the button blends with
// its parent row. Hover uses a brighter shade than row hover (SURFACE_ACTIVE
// = 0x313244) so the icon button stands out when hovered. Colors use the
// 0xRRGGBBAA format so the alpha channel is respected via rgba().
pub const BTN_GHOST_BG: u32 = 0x00000000;
pub const BTN_GHOST_BG_HOVER: u32 = 0x45475aff;
pub const BTN_GHOST_BG_SELECTED: u32 = 0x313244ff;
pub const BTN_GHOST_BG_DISABLED: u32 = 0x00000000;
pub const BTN_GHOST_BORDER: u32 = 0x00000000;
pub const BTN_GHOST_TEXT_DISABLED: u32 = 0x585b70ff;

// Input
pub const INPUT_BG: u32 = 0x181825;
pub const INPUT_BG_FOCUSED: u32 = 0x1e1e2e;
pub const INPUT_BG_DISABLED: u32 = 0x11111b;
pub const INPUT_TEXT_DISABLED: u32 = 0x45475a;
pub const INPUT_BORDER: u32 = 0x313244;
pub const INPUT_BORDER_HOVER: u32 = 0x45475a;
pub const INPUT_BORDER_FOCUSED: u32 = 0x89b4fa;
pub const INPUT_BORDER_ERROR: u32 = 0xef4444;
pub const INPUT_PLACEHOLDER: u32 = 0x585b70;
pub const INPUT_SELECTION: u32 = 0x89b4fa33;

// Command
pub const COMMAND_OVERLAY: u32 = 0x00000050;
pub const COMMAND_BG: u32 = 0x1e1e2e;
pub const COMMAND_BORDER: u32 = 0x313244;
pub const COMMAND_ITEM_HOVER: u32 = 0x24273a;
pub const COMMAND_ITEM_ACTIVE: u32 = 0x313244;
pub const COMMAND_GROUP_LABEL: u32 = 0x585b70;
