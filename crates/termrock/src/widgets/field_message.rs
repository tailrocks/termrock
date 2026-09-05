// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! One way to say what is wrong with a field.
//!
//! Six inputs each spelled their own validation row: some at `y + 2`, one at
//! the bottom edge, all of them colour-only. A red line with no glyph says
//! nothing on a monochrome terminal, and a message that moves between widgets
//! makes the reader hunt for it. The message sits directly under the field, in
//! every widget. An error is the error tone; the bold `!` lives on the field.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Style};

use crate::style::{DesignSystem, Glyph, Role};
use crate::text::truncate_cols;

use super::label::DescriptionKind;

/// Glyph that names a description kind, when the kind is an event.
fn kind_glyph(kind: DescriptionKind, system: &DesignSystem) -> Option<&'static str> {
    let glyph = match kind {
        // `!` is the error mark. `•` is modified/pending — never an error.
        DescriptionKind::Error => Glyph::Error,
        DescriptionKind::Warning => Glyph::Warning,
        // Help and meta are not events; they need no mark.
        DescriptionKind::Help | DescriptionKind::Meta => return None,
    };
    Some(system.glyphs.resolve(glyph).text)
}

/// Tone a description kind speaks in.
fn kind_style(kind: DescriptionKind, system: &DesignSystem) -> Style {
    let theme = system.junie_theme();
    match kind {
        DescriptionKind::Error => theme.error_fg(),
        DescriptionKind::Warning => system.style(Role::Warning),
        DescriptionKind::Help | DescriptionKind::Meta => theme.muted(),
    }
}

/// Paints one field message across `row`.
///
/// Help is muted copy with no mark. An error is the error tone; the trailing
/// bold `!` lives on the field row, not here — this line is the words.
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
    let text = match kind {
        DescriptionKind::Error => message.to_string(),
        DescriptionKind::Warning => match kind_glyph(kind, system) {
            Some(glyph) => format!("{glyph} {message}"),
            None => message.to_string(),
        },
        DescriptionKind::Help | DescriptionKind::Meta => message.to_string(),
    };
    let painted = truncate_cols(&text, usize::from(row.width), system.glyphs.ellipsis());
    buffer.set_stringn(
        row.x,
        row.y,
        painted.as_ref(),
        usize::from(row.width),
        style,
    );
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
    fn overflow_help_uses_ellipsis_not_hard_clip() {
        let system = DesignSystem::junie();
        let row = Rect::new(0, 0, 12, 1);
        let mut buffer = Buffer::empty(row);
        paint_field_message(
            &mut buffer,
            row,
            &system,
            DescriptionKind::Help,
            "Leave empty to work on a detached checkout",
        );
        let line: String = (0..row.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            line.contains(system.glyphs.ellipsis()),
            "overflow help must mark the cut, got {line:?}"
        );
        assert!(
            !line.contains("checkout"),
            "overflow help must not hard-clip the tail, got {line:?}"
        );
    }

    #[test]
    fn each_kind_speaks_in_its_own_tone() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        assert_eq!(
            kind_style(DescriptionKind::Error, &system),
            theme.error_fg()
        );
        assert_eq!(
            kind_style(DescriptionKind::Warning, &system),
            system.style(Role::Warning)
        );
        assert_eq!(kind_style(DescriptionKind::Help, &system), theme.muted());
    }

    #[test]
    fn error_uses_bang_not_bullet() {
        let system = DesignSystem::junie();
        assert_eq!(kind_glyph(DescriptionKind::Error, &system), Some("!"));
        assert_ne!(kind_glyph(DescriptionKind::Error, &system), Some("•"));
        let line = render(DescriptionKind::Error, &system);
        assert!(
            line.starts_with("too short"),
            "help-row error is the words; `!` lives on the field, got {line:?}"
        );
        assert!(
            !line.contains('•'),
            "error copy must not use the modified/pending bullet, got {line:?}"
        );
    }
}
