// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The xterm-256 colour table, shared by every exporter.
//!
//! Both exporters used to render `Color::Indexed` as flat white, which made a
//! 256-colour or ANSI-16 preview a page of white text — exactly the profiles a
//! capability ladder exists to show (plans/011 Step 3).
/// RGB for an xterm-256 palette index.
#[must_use]
pub fn xterm256_to_rgb(index: u8) -> [u8; 3] {
    match index {
        // The 16 system colours, in the same slots both exporters already use.
        0 => [0x00, 0x00, 0x00],
        1 => [0xff, 0x00, 0x00],
        2 => [0x00, 0xff, 0x41],
        3 => [0xff, 0xd8, 0x5e],
        4 => [0x00, 0x50, 0xb4],
        5 => [0xff, 0x00, 0xff],
        6 => [0x00, 0xff, 0xff],
        7 => [0xc0, 0xc0, 0xc0],
        8 => [0x80, 0x80, 0x80],
        9 => [0xff, 0x5e, 0x7a],
        10 => [0x00, 0xff, 0x41],
        11 => [0xff, 0xd8, 0x5e],
        12 => [0x7a, 0xa2, 0xff],
        13 => [0xff, 0x7a, 0xff],
        14 => [0x7a, 0xff, 0xff],
        15 => [0xff, 0xff, 0xff],
        // 6×6×6 colour cube.
        16..=231 => {
            let i = index - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            [
                steps[usize::from(i / 36)],
                steps[usize::from((i % 36) / 6)],
                steps[usize::from(i % 6)],
            ]
        }
        // 24-step grey ramp.
        232..=255 => {
            let level = 8 + (index - 232) * 10;
            [level, level, level]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cube_and_the_ramp_are_not_all_white() {
        assert_eq!(xterm256_to_rgb(16), [0, 0, 0]);
        assert_eq!(xterm256_to_rgb(21), [0, 0, 255]);
        assert_eq!(xterm256_to_rgb(196), [255, 0, 0]);
        assert_eq!(xterm256_to_rgb(231), [255, 255, 255]);
        assert_eq!(xterm256_to_rgb(232), [8, 8, 8]);
        assert_eq!(xterm256_to_rgb(255), [238, 238, 238]);
        // Every index resolves to something; none of them is a stub.
        let whites = (0..=255u8)
            .filter(|i| xterm256_to_rgb(*i) == [255, 255, 255])
            .count();
        assert!(whites <= 2, "indexed colours must not collapse to white");
    }
}
