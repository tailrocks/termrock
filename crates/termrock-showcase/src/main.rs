// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Showcase binary is the canonical Junie-style catalog. No second shell.

fn main() -> std::io::Result<()> {
    termrock_catalog::run()
}
