// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

/// Outcome of a zero-tolerance decoded-pixel comparison.
#[derive(Debug, PartialEq, Eq)]
pub enum PixelDiff {
    /// One input is not a valid PNG.
    Decode {
        /// Which input failed to decode.
        input: &'static str,
        /// Decoder diagnostic.
        message: String,
    },
    /// The two decoded images have different dimensions.
    DimensionMismatch {
        /// (width, height) of the first image.
        a: (u32, u32),
        /// (width, height) of the second image.
        b: (u32, u32),
    },
    /// First differing pixel in row-major scan order.
    FirstDifference {
        /// X coordinate of the first differing pixel.
        x: u32,
        /// Y coordinate of the first differing pixel.
        y: u32,
        /// RGBA of that pixel in the first image.
        a: [u8; 4],
        /// RGBA of that pixel in the second image.
        b: [u8; 4],
    },
}

impl fmt::Display for PixelDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode { input, message } => write!(f, "cannot decode PNG {input}: {message}"),
            Self::DimensionMismatch { a, b } => write!(f, "PNG dimensions differ: {a:?} != {b:?}"),
            Self::FirstDifference { x, y, a, b } => {
                write!(f, "PNG pixels first differ at ({x}, {y}): {a:?} != {b:?}")
            }
        }
    }
}

/// Decode two PNGs and compare every pixel at zero tolerance.
pub fn compare_png_pixels(a: &[u8], b: &[u8]) -> Result<(), PixelDiff> {
    let a = tiny_skia::Pixmap::decode_png(a).map_err(|error| PixelDiff::Decode {
        input: "a",
        message: error.to_string(),
    })?;
    let b = tiny_skia::Pixmap::decode_png(b).map_err(|error| PixelDiff::Decode {
        input: "b",
        message: error.to_string(),
    })?;
    let a_size = (a.width(), a.height());
    let b_size = (b.width(), b.height());
    if a_size != b_size {
        return Err(PixelDiff::DimensionMismatch {
            a: a_size,
            b: b_size,
        });
    }
    for (index, (a_pixel, b_pixel)) in a
        .data()
        .chunks_exact(4)
        .zip(b.data().chunks_exact(4))
        .enumerate()
    {
        if a_pixel != b_pixel {
            let x = index as u32 % a.width();
            let y = index as u32 / a.width();
            return Err(PixelDiff::FirstDifference {
                x,
                y,
                a: a_pixel.try_into().expect("RGBA chunk"),
                b: b_pixel.try_into().expect("RGBA chunk"),
            });
        }
    }
    Ok(())
}
