// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use ratatui::style::Modifier;
use swash::FontRef;

pub(crate) const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
pub(crate) const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");
pub(crate) const FONT_ITALIC: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf");

/// Selects Bold before Italic when both modifiers are set; only three faces are vendored.
pub(crate) fn face(modifier: Modifier) -> FontRef<'static> {
    let bytes = if modifier.contains(Modifier::BOLD) {
        FONT_BOLD
    } else if modifier.contains(Modifier::ITALIC) {
        FONT_ITALIC
    } else {
        FONT_REGULAR
    };
    FontRef::from_index(bytes, 0).expect("vendored JetBrains Mono face must parse")
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn vendored_font_hashes_are_pinned() {
        let cases = [
            (
                "JetBrainsMono-Regular.ttf",
                FONT_REGULAR,
                "a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f",
            ),
            (
                "JetBrainsMono-Bold.ttf",
                FONT_BOLD,
                "5590990c82e097397517f275f430af4546e1c45cff408bde4255dad142479dcb",
            ),
            (
                "JetBrainsMono-Italic.ttf",
                FONT_ITALIC,
                "9d0a1f7a708e6af183f1193b7e81d40da294f5c67682c085d8401c60aac8ded4",
            ),
        ];
        for (name, bytes, pinned) in cases {
            let actual = hex::encode(Sha256::digest(bytes));
            assert_eq!(actual, pinned, "vendored font hash mismatch: {name}");
        }
    }
}
