// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

pub(crate) use termrock::style::color_to_rgb;

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use termrock::style::xterm256_to_rgb;

    #[test]
    fn named_and_reset_colors_match_lookbook() {
        assert_eq!(color_to_rgb(Color::LightGreen, true), [0x48, 0xe0, 0x54]);
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
