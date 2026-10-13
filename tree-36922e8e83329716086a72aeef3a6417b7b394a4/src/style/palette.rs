// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

/// Three-byte RGB value used to construct terminal colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub(crate) const PHOSPHOR_GREEN: Rgb = Rgb::new(0, 255, 65);
pub(crate) const PHOSPHOR_DIM: Rgb = Rgb::new(0, 140, 30);
pub(crate) const PHOSPHOR_DARK: Rgb = Rgb::new(0, 80, 18);
pub(crate) const DIALOG_SCROLL_THUMB: Rgb = PHOSPHOR_GREEN;
pub(crate) const DIALOG_SCROLL_TRACK: Rgb = PHOSPHOR_DARK;
pub(crate) const WHITE: Rgb = Rgb::new(255, 255, 255);
pub(crate) const INPUT_BG_DIM: Rgb = Rgb::new(20, 24, 22);
pub(crate) const TAB_BG_INACTIVE: Rgb = Rgb::new(30, 30, 30);
pub(crate) const TAB_BG_INACTIVE_HOVER: Rgb = Rgb::new(48, 48, 48);
pub(crate) const TAB_BG_ACTIVE: Rgb = Rgb::new(42, 42, 42);
pub(crate) const TAB_BG_ACTIVE_HOVER: Rgb = Rgb::new(58, 58, 58);
pub(crate) const LINK_FG: Rgb = Rgb::new(0, 200, 200);
pub(crate) const LINK_FG_HOVER: Rgb = Rgb::new(130, 240, 240);
pub(crate) const BORDER_GRAY: Rgb = Rgb::new(80, 80, 80);
pub(crate) const DANGER_RED: Rgb = Rgb::new(255, 94, 122);
pub(crate) const CYAN: Rgb = Rgb::new(0, 180, 180);
pub(crate) const WARNING_YELLOW: Rgb = Rgb::new(255, 216, 94);
pub(crate) const PREVIEW_CARD: Rgb = Rgb::new(28, 28, 28);
