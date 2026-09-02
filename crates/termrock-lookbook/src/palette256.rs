// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The xterm-256 colour table, shared by every exporter.
//!
//! Both exporters used to render `Color::Indexed` as flat white, which made a
//! 256-colour or ANSI-16 preview a page of white text — exactly the profiles a
//! capability ladder exists to show (plans/011 Step 3).
pub use termrock::style::xterm256_to_rgb;

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
