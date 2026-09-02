// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! One way to say what is wrong with a field.
//!
//! Six inputs each spelled their own validation row: some at `y + 2`, one at
//! the bottom edge, all of them colour-only. A red line with no glyph says
//! nothing on a monochrome terminal, and a message that moves between widgets
//! makes the reader hunt for it. The message sits directly under the field, in
//! every widget, and leads with the glyph that names its kind.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Style};

use crate::style::{DesignSystem, Glyph, Role};
use crate::text::take_display_cols;

use super::label::DescriptionKind;

/// Glyph that names a description kind, when the kind is an event.
fn kind_glyph(kind: DescriptionKind, system: &DesignSystem) -> Option<&'static str> {
    let glyph = match kind {
        DescriptionKind::Error => Glyph::Error,
        DescriptionKind::Warning => Glyph::Warning,
        // Help and meta are not events; they need no mark.
        DescriptionKind::Help | DescriptionKind::Meta => return None,
    };
    Some(system.glyphs.resolve(glyph).text)
}

/// Tone a description kind speaks in.
fn kind_style(kind: DescriptionKind, system: &DesignSystem) -> Style {
    system.style(match kind {
        DescriptionKind::Error => Role::Danger,
        DescriptionKind::Warning => Role::Warning,
        DescriptionKind::Help | DescriptionKind::Meta => Role::TextFaint,
    })
}

/// Paints one field message across `row`, glyph first.
///
/// The glyph carries the kind so the row survives `NO_COLOR`; the words stay
/// readable rather than being dyed by severity.
pub(crate) fn paint_field_message(
    buffer: &mut Buffer,
    row: Rect,
    system: &DesignSystem,
    kind: DescriptionKind,
    message: &str,
) {
    if row.width == 0 || row.height == 0 || message.is_empty() {
        return;
    }
    let style = kind_style(kind, system);
    let text = match kind_glyph(kind, system) {
        Some(glyph) => format!("{glyph} {message}"),
        None => message.to_string(),
    };
    let painted = take_display_cols(&text, usize::from(row.width));
    buffer.set_stringn(row.x, row.y, &painted, usize::from(row.width), style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(kind: DescriptionKind, system: &DesignSystem) -> String {
        let row = Rect::new(0, 0, 20, 1);
        let mut buffer = Buffer::empty(row);
        paint_field_message(&mut buffer, row, system, kind, "too short");
        (0..row.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect()
    }

    #[test]
    fn help_is_quiet_and_unmarked() {
        let system = DesignSystem::junie();
        let line = render(DescriptionKind::Help, &system);
        assert!(line.starts_with("too short"), "help carries no event glyph");
    }

    #[test]
    fn each_kind_speaks_in_its_own_tone() {
        let system = DesignSystem::junie();
        assert_eq!(
            kind_style(DescriptionKind::Error, &system),
            system.style(Role::Danger)
        );
        assert_eq!(
            kind_style(DescriptionKind::Warning, &system),
            system.style(Role::Warning)
        );
        assert_eq!(
            kind_style(DescriptionKind::Help, &system),
            system.style(Role::TextFaint)
        );
    }
}
