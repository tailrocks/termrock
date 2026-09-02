// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lookbook binary is the canonical Junie-style catalog. No studio shell.

fn main() -> std::io::Result<()> {
    termrock_catalog::run()
}
