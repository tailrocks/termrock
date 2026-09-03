// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Vendored face chain for deterministic rasterization.
//!
//! JetBrains Mono carries the terminal text and the box/block/geometric
//! vocabulary. It maps no braille cells, so the canonical braille spinner and
//! density ramp used to fall through to `.notdef` tofu. Three OFL-licensed Noto
//! faces back it up, each covering a block JetBrains Mono lacks:
//!
//! | face | covers |
//! |------|--------|
//! | Noto Sans Symbols 2 | braille (U+2800..=U+28FF), OCR/misc symbols |
//! | Noto Sans Math | `⧉`, `↶` and the rest of misc math symbols |
//! | Noto Emoji (monochrome) | emoji-presentation cells the baselines paint |
//!
//! [`resolve`] walks the chain and returns `None` when no vendored face maps
//! the character, so a `.notdef` glyph is never rasterized — an unmapped
//! character paints nothing instead of tofu.

use std::sync::OnceLock;

use ratatui::style::Modifier;
use swash::{FontRef, GlyphId};

const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");
const FONT_ITALIC: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf");
const FONT_SYMBOLS: &[u8] = include_bytes!("../assets/fonts/NotoSansSymbols2-Regular.ttf");
const FONT_MATH: &[u8] = include_bytes!("../assets/fonts/NotoSansMath-Regular.ttf");
const FONT_EMOJI: &[u8] = include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf");

/// Number of faces in the chain: three text styles plus three fallbacks.
const FACE_COUNT: usize = 6;

/// Indices into the parsed face chain.
const REGULAR: usize = 0;
const BOLD: usize = 1;
const ITALIC: usize = 2;
const SYMBOLS: usize = 3;
const MATH: usize = 4;
const EMOJI: usize = 5;

fn faces() -> &'static [FontRef<'static>; FACE_COUNT] {
    static FACES: OnceLock<[FontRef<'static>; FACE_COUNT]> = OnceLock::new();
    FACES.get_or_init(|| {
        [
            parse(FONT_REGULAR, "JetBrainsMono-Regular.ttf"),
            parse(FONT_BOLD, "JetBrainsMono-Bold.ttf"),
            parse(FONT_ITALIC, "JetBrainsMono-Italic.ttf"),
            parse(FONT_SYMBOLS, "NotoSansSymbols2-Regular.ttf"),
            parse(FONT_MATH, "NotoSansMath-Regular.ttf"),
            parse(FONT_EMOJI, "NotoEmoji-Regular.ttf"),
        ]
    })
}

fn parse(bytes: &'static [u8], name: &str) -> FontRef<'static> {
    match FontRef::from_index(bytes, 0) {
        Some(face) => face,
        None => panic!("vendored font must parse: {name}"),
    }
}

/// Selects Bold before Italic when both modifiers are set; fallbacks are
/// regular-only because no bold/italic companions are vendored.
fn chain(modifier: Modifier) -> [usize; 4] {
    let primary = if modifier.contains(Modifier::BOLD) {
        BOLD
    } else if modifier.contains(Modifier::ITALIC) {
        ITALIC
    } else {
        REGULAR
    };
    [primary, SYMBOLS, MATH, EMOJI]
}

/// A character mapped to a real glyph in one of the vendored faces.
///
/// `glyph` is never the `.notdef` id; unmapped characters resolve to [`None`]
/// and paint nothing.
#[derive(Clone, Copy)]
pub(crate) struct ResolvedGlyph {
    /// Face that maps the character.
    pub face: FontRef<'static>,
    /// Mapped glyph id.
    pub glyph: GlyphId,
}

/// Maps `character` through the face chain for `modifier`.
pub(crate) fn resolve(modifier: Modifier, character: char) -> Option<ResolvedGlyph> {
    let faces = faces();
    chain(modifier).into_iter().find_map(|index| {
        let face = faces[index];
        let glyph = face.charmap().map(character);
        (glyph != 0).then_some(ResolvedGlyph { face, glyph })
    })
}

/// Whether any face in the chain for `modifier` maps `character`.
pub(crate) fn is_mapped(modifier: Modifier, character: char) -> bool {
    resolve(modifier, character).is_some()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use sha2::{Digest, Sha256};
    use termrock::style::{
        BLOCK_RAMP, BRAILLE_RAMP, Glyph, LEFT_BLOCK_RAMP, SHADE_RAMP, SPINNER_BRAILLE_FRAMES,
    };

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
            (
                "NotoSansSymbols2-Regular.ttf",
                FONT_SYMBOLS,
                "c4a0a80f0041ce4be81e2478faad22776d23edb98ae3f0d19bd37044820ecf9d",
            ),
            (
                "NotoSansMath-Regular.ttf",
                FONT_MATH,
                "d51afd5739c7ba6c44fcab35a88160e25dfb69a2d4ad0bd99533f8d894af1f96",
            ),
            (
                "NotoEmoji-Regular.ttf",
                FONT_EMOJI,
                "de6c18832938afc99caf132b39d6a30a19bac7f2e812e28db2535b4608d27551",
            ),
        ];
        for (name, bytes, pinned) in cases {
            let actual = hex::encode(Sha256::digest(bytes));
            assert_eq!(actual, pinned, "vendored font hash mismatch: {name}");
        }
    }

    /// The canonical braille spinner frames and density ramp are the cells the
    /// reference paints every 80 ms; a gap here resurfaces as blank cells.
    #[test]
    fn braille_block_is_fully_mapped() {
        for codepoint in 0x2800u32..=0x28FF {
            let character = char::from_u32(codepoint).expect("braille range");
            assert!(
                is_mapped(Modifier::empty(), character),
                "braille U+{codepoint:04X} {character} unmapped in every vendored face",
            );
        }
    }

    /// Box drawing, block elements, and geometric shapes come from JetBrains
    /// Mono itself; the fallback chain must not be needed for them.
    #[test]
    fn terminal_symbol_blocks_are_fully_mapped() {
        for (label, range) in [
            ("box drawing", 0x2500u32..=0x257F),
            ("block elements", 0x2580..=0x259F),
            ("geometric shapes", 0x25A0..=0x25FF),
        ] {
            for codepoint in range {
                let character = char::from_u32(codepoint).expect("contiguous range");
                assert!(
                    is_mapped(Modifier::empty(), character),
                    "{label} U+{codepoint:04X} {character} unmapped in every vendored face",
                );
            }
        }
    }

    /// Every non-ASCII cell the catalog baselines paint must resolve — the
    /// set that rendered tofu before the fallback chain existed.
    #[test]
    fn baseline_symbol_and_emoji_cells_are_mapped() {
        let cells = "↯↶⏳⑂◐☁★⚙⛔✅✨⠋⧉⬡🇯🇵🌐🌑📋📜📣🚀🚫🧪";
        for character in cells.chars() {
            assert!(
                is_mapped(Modifier::empty(), character),
                "{} U+{:04X} unmapped in every vendored face",
                character,
                character as u32,
            );
        }
    }

    /// Nothing the canonical junie catalog spells may rasterize to nothing:
    /// every encoding, ramp cell, and spinner frame must resolve through the
    /// chain, read live from the widget crate rather than a copied literal.
    #[test]
    fn junie_vocabulary_is_mapped() {
        let mut cells: BTreeSet<char> = BTreeSet::new();
        for glyph in Glyph::ALL {
            cells.extend(glyph.resolve().text.chars());
        }
        let ramps: [&[char]; 4] = [BLOCK_RAMP, LEFT_BLOCK_RAMP, SHADE_RAMP, BRAILLE_RAMP];
        for ramp in ramps {
            cells.extend(ramp.iter().copied());
        }
        for frame in SPINNER_BRAILLE_FRAMES {
            cells.extend(frame.chars());
        }
        cells.extend("╭╮╰╯".chars());
        assert!(!cells.is_empty(), "vocabulary read came back empty");
        for character in cells {
            assert!(
                is_mapped(Modifier::empty(), character),
                "{} U+{:04X} is painted by the catalog yet unmapped in every vendored face",
                character,
                character as u32,
            );
        }
    }

    /// Text resolves through the styled JetBrains Mono face; fallback faces
    /// are regular-only and shared across modifiers.
    #[test]
    fn modifiers_use_their_primary_face_first() {
        for character in ['a', '─', '✓'] {
            let plain = resolve(Modifier::empty(), character)
                .expect("regular face")
                .face;
            let bold = resolve(Modifier::BOLD, character).expect("bold face").face;
            let italic = resolve(Modifier::ITALIC, character)
                .expect("italic face")
                .face;
            assert_eq!(plain.key, faces()[REGULAR].key);
            assert_eq!(bold.key, faces()[BOLD].key);
            assert_eq!(italic.key, faces()[ITALIC].key);
        }
        for character in ['⠋', '⣿', '⧉', '🚀'] {
            let plain = resolve(Modifier::empty(), character)
                .expect("fallback face")
                .face;
            let bold = resolve(Modifier::BOLD, character)
                .expect("fallback face")
                .face;
            assert_ne!(plain.key, faces()[REGULAR].key);
            assert_eq!(plain.key, bold.key, "fallbacks are regular-only");
        }
    }
}
