// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use ratatui::style::Color;

/// Resolves xterm-256 colors like `crates/termrock-lookbook/src/palette256.rs`.
/// Duplicated by decision in plan 001; keep in lockstep until unified.
pub(crate) fn xterm256_to_rgb(index: u8) -> [u8; 3] {
    const SYSTEM: [[u8; 3]; 16] = [
        [0x00, 0x00, 0x00],
        [0xff, 0x00, 0x00],
        [0x00, 0xff, 0x41],
        [0xff, 0xd8, 0x5e],
        [0x00, 0x50, 0xb4],
        [0xff, 0x00, 0xff],
        [0x00, 0xff, 0xff],
        [0xc0, 0xc0, 0xc0],
        [0x80, 0x80, 0x80],
        [0xff, 0x5e, 0x7a],
        [0x00, 0xff, 0x41],
        [0xff, 0xd8, 0x5e],
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

/// Resolves ratatui colors like `crates/termrock-lookbook/src/frame.rs:201-228`.
/// Duplicated by decision in plan 001; keep in lockstep until unified.
pub(crate) fn color_to_rgb(color: Color, is_fg: bool) -> [u8; 3] {
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
        Color::Green => [0, 0xff, 0x41],
        Color::Yellow => [0xff, 0xd8, 0x5e],
        Color::Blue => [0, 0x50, 0xb4],
        Color::Magenta => [0xff, 0, 0xff],
        Color::Cyan => [0, 0xff, 0xff],
        Color::Gray | Color::DarkGray => [0x80, 0x80, 0x80],
        Color::LightRed => [0xff, 0x5e, 0x7a],
        Color::LightGreen => [0, 0xff, 0x41],
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
    fn named_and_reset_colors_match_lookbook() {
        assert_eq!(color_to_rgb(Color::Green, true), [0x00, 0xff, 0x41]);
        assert_eq!(color_to_rgb(Color::Reset, true), [0xff, 0xff, 0xff]);
        assert_eq!(color_to_rgb(Color::Reset, false), [0, 0, 0]);
    }

    #[test]
    fn indexed_spot_values_match_xterm() {
        assert_eq!(xterm256_to_rgb(21), [0, 0, 255]);
        assert_eq!(xterm256_to_rgb(196), [255, 0, 0]);
        assert_eq!(xterm256_to_rgb(232), [8, 8, 8]);
        assert_eq!(xterm256_to_rgb(255), [238, 238, 238]);
    }
}
