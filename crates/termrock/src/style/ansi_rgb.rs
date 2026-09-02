// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Named ANSI and xterm-256 RGB resolution for exporters.
use ratatui_core::style::Color;

/// RGB for an xterm-256 palette index.
///
/// System slots 0–15 use the junie-tuned named ANSI mapping.
#[must_use]
pub fn xterm256_to_rgb(index: u8) -> [u8; 3] {
    const SYSTEM: [[u8; 3]; 16] = [
        [0x00, 0x00, 0x00],
        [0xff, 0x00, 0x00],
        [0x2b, 0x86, 0x32],
        [0xf5, 0x9e, 0x09],
        [0x00, 0x50, 0xb4],
        [0xff, 0x00, 0xff],
        [0x00, 0xff, 0xff],
        [0xc0, 0xc0, 0xc0],
        [0x80, 0x80, 0x80],
        [0xe4, 0x45, 0x45],
        [0x48, 0xe0, 0x54],
        [0xf5, 0x9e, 0x09],
        [0x7a, 0xa2, 0xff],
        [0xff, 0x7a, 0xff],
        [0x7a, 0xff, 0xff],
        [0xff, 0xff, 0xff],
    ];
    match index {
        0..=15 => SYSTEM[usize::from(index)],
        16..=231 => {
            const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let i = index - 16;
            [
                STEPS[usize::from(i / 36)],
                STEPS[usize::from((i % 36) / 6)],
                STEPS[usize::from(i % 6)],
            ]
        }
        232..=255 => {
            let level = 8 + (index - 232) * 10;
            [level, level, level]
        }
    }
}

/// Resolves a ratatui color to sRGB for exporters.
///
/// Named ANSI uses the junie-tuned table. `Reset` is white when `is_fg`, black
/// otherwise.
#[must_use]
pub fn color_to_rgb(color: Color, is_fg: bool) -> [u8; 3] {
    match color {
        Color::Reset => {
            if is_fg {
                [0xff, 0xff, 0xff]
            } else {
                [0, 0, 0]
            }
        }
        Color::Black => [0, 0, 0],
        Color::Red => [0xff, 0, 0],
        Color::Green => [0x2b, 0x86, 0x32],
        Color::Yellow => [0xf5, 0x9e, 0x09],
        Color::Blue => [0, 0x50, 0xb4],
        Color::Magenta => [0xff, 0, 0xff],
        Color::Cyan => [0, 0xff, 0xff],
        Color::Gray | Color::DarkGray => [0x80, 0x80, 0x80],
        Color::LightRed => [0xe4, 0x45, 0x45],
        Color::LightGreen => [0x48, 0xe0, 0x54],
        Color::LightYellow => [0xff, 0xd8, 0x5e],
        Color::LightBlue => [0x7a, 0xa2, 0xff],
        Color::LightMagenta => [0xff, 0x7a, 0xff],
        Color::LightCyan => [0x7a, 0xff, 0xff],
        Color::White => [0xff, 0xff, 0xff],
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(index) => xterm256_to_rgb(index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn green_light_green_and_xterm_cube() {
        assert_eq!(color_to_rgb(Color::Green, true), [0x2b, 0x86, 0x32]);
        assert_eq!(color_to_rgb(Color::LightGreen, true), [0x48, 0xe0, 0x54]);
        assert_eq!(xterm256_to_rgb(2), [0x2b, 0x86, 0x32]);
        assert_eq!(xterm256_to_rgb(10), [0x48, 0xe0, 0x54]);
        assert_eq!(xterm256_to_rgb(16), [0, 0, 0]);
        assert_eq!(xterm256_to_rgb(21), [0, 0, 255]);
        assert_eq!(xterm256_to_rgb(196), [255, 0, 0]);
        assert_eq!(xterm256_to_rgb(231), [255, 255, 255]);
        assert_eq!(xterm256_to_rgb(232), [8, 8, 8]);
        assert_eq!(xterm256_to_rgb(255), [238, 238, 238]);
    }
}
