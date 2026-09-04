// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composed label/value rows for forms and detail surfaces.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Line, widgets::Widget};

use crate::{
    style::{DesignSystem, Glyph, MASK_CELLS, Role, VisualState},
    text::{display_cols, truncate_cols},
};

/// Value content painted by a [`FieldRow`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldRowValue<'a> {
    /// Ordinary text.
    Plain(&'a str),
    /// Secret-shaped content rendered without exposing its value.
    Masked {
        /// Number of mask glyphs to paint.
        len: usize,
    },
    /// Caller-styled content such as breadcrumbs.
    Composed(Line<'a>),
    /// Missing content with explanatory text.
    Unset {
        /// Explanatory placeholder for the missing value.
        hint: &'a str,
    },
}

impl FieldRowValue<'_> {
    /// Searchable plain text; masked and composed values never leak content.
    #[must_use]
    pub fn searchable_text(&self) -> &str {
        match self {
            Self::Plain(value) | Self::Unset { hint: value } => value,
            Self::Masked { .. } | Self::Composed(_) => "",
        }
    }

    /// Whether this projection represents a present value.
    #[must_use]
    pub fn is_set(&self) -> bool {
        match self {
            Self::Plain(value) => !value.trim().is_empty(),
            Self::Masked { len } => *len > 0,
            Self::Composed(line) => line.width() > 0,
            Self::Unset { .. } => false,
        }
    }
}

impl Widget for FieldRow<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

/// A density-aware composed field row.
#[derive(Debug, Clone)]
pub struct FieldRow<'a> {
    system: &'a DesignSystem,
    label: &'a str,
    value: FieldRowValue<'a>,
    marker: Option<&'a str>,
    annotation: Option<&'a str>,
    label_cols: u16,
    required: bool,
    selected: bool,
    hovered: bool,
    enabled: bool,
    invalid: bool,
    editing: bool,
}

impl<'a> FieldRow<'a> {
    /// Creates a field row with an eight-column minimum label band.
    #[must_use]
    pub fn new(system: &'a DesignSystem, label: &'a str, value: FieldRowValue<'a>) -> Self {
        Self {
            system,
            label,
            value,
            marker: None,
            annotation: None,
            label_cols: 8,
            required: false,
            selected: false,
            hovered: false,
            enabled: true,
            invalid: false,
            editing: false,
        }
    }

    /// Measures a shared label band, with an eight-column minimum.
    #[must_use]
    pub fn label_cols_for<'b>(labels: impl Iterator<Item = &'b str>) -> u16 {
        labels
            .map(display_cols)
            .max()
            .unwrap_or(0)
            .max(8)
            .try_into()
            .unwrap_or(u16::MAX)
    }

    /// Adds a marker between selection gutter and label.
    #[must_use]
    pub const fn marker(mut self, marker: &'a str) -> Self {
        self.marker = Some(marker);
        self
    }

    /// Adds quiet trailing annotation text.
    #[must_use]
    pub const fn annotation(mut self, annotation: &'a str) -> Self {
        self.annotation = Some(annotation);
        self
    }

    /// Marks an unset value as required/dangerous.
    #[must_use]
    pub const fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Applies selection state.
    #[must_use]
    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Applies pointer-hover state.
    #[must_use]
    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Applies enabled state.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Applies invalid value chrome.
    #[must_use]
    pub const fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Applies in-place editing chrome (accent underline; hover suppressed).
    #[must_use]
    pub const fn editing(mut self, editing: bool) -> Self {
        self.editing = editing;
        self
    }

    /// Paints this row into one terminal line.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let theme = self.system.junie_theme();
        let visual = VisualState {
            focused: self.selected && self.enabled,
            hovered: self.hovered && self.enabled && !self.editing,
            disabled: !self.enabled,
            error: self.invalid,
            editing: self.editing && self.enabled,
            ..VisualState::default()
        };
        let field = theme.field_style(visual);
        let row = Rect::new(area.x, area.y, area.width, 1);
        buffer.set_style(row, field);
        let gutter = theme.gutter(visual, field.bg.unwrap_or(theme.field), false);
        buffer.set_stringn(
            area.x,
            area.y,
            self.system.glyphs.selection_gutter(),
            1,
            gutter,
        );

        let mut x = area.x.saturating_add(2);
        if let Some(marker) = self.marker {
            let marker_width =
                display_cols(marker).min(usize::from(area.right().saturating_sub(x)));
            buffer.set_stringn(x, area.y, marker, marker_width, theme.secondary());
            x = x.saturating_add(u16::try_from(marker_width).unwrap_or(u16::MAX));
            x = x.saturating_add(self.system.spacing.gap);
        }

        let remaining = area.right().saturating_sub(x);
        let label_width = self.label_cols.min(remaining);
        let label = truncate_cols(self.label, usize::from(label_width), "…");
        buffer.set_stringn(
            x,
            area.y,
            &label,
            usize::from(label_width),
            theme.label(visual.focused),
        );
        x = x.saturating_add(label_width);
        x = x.saturating_add(self.system.spacing.gap);
        if x >= area.right() {
            return;
        }

        let bang = u16::from(self.invalid && remaining > 2);
        let annotation_width = self.annotation.map_or(0, |text| {
            display_cols(text).saturating_add(usize::from(self.system.spacing.gap))
        });
        let value_width = usize::from(
            area.right()
                .saturating_sub(x)
                .saturating_sub(bang.saturating_add(u16::from(bang > 0))),
        )
        .saturating_sub(annotation_width);
        let value_style = if !self.enabled {
            theme.faint().bg(field.bg.unwrap_or(theme.field))
        } else {
            Style::new()
                .fg(theme.text_primary)
                .bg(field.bg.unwrap_or(theme.field))
        };
        let placeholder = theme.placeholder(visual);
        match &self.value {
            FieldRowValue::Plain(value) => {
                let text = truncate_cols(value, value_width, "…");
                buffer.set_stringn(x, area.y, &text, value_width, value_style);
            }
            FieldRowValue::Masked { .. } => {
                let glyph = Glyph::Mask.resolve().text;
                let n = MASK_CELLS.min(value_width);
                buffer.set_stringn(x, area.y, &glyph.repeat(n), value_width, value_style);
            }
            FieldRowValue::Composed(line) => {
                buffer.set_line(
                    x,
                    area.y,
                    line,
                    u16::try_from(value_width).unwrap_or(u16::MAX),
                );
            }
            FieldRowValue::Unset { hint } => {
                let style = if self.required {
                    placeholder.patch(self.system.style(Role::Danger))
                } else {
                    placeholder
                };
                let text = truncate_cols(hint, value_width, "…");
                buffer.set_stringn(x, area.y, &text, value_width, style);
            }
        }

        let underline_color = if visual.editing {
            Some(theme.accent)
        } else {
            None
        };
        if let Some(color) = underline_color {
            buffer.set_style(
                Rect::new(
                    x,
                    area.y,
                    u16::try_from(value_width)
                        .unwrap_or(u16::MAX)
                        .min(area.right().saturating_sub(x)),
                    1,
                ),
                Style::new()
                    .add_modifier(ratatui_core::style::Modifier::UNDERLINED)
                    .underline_color(color),
            );
        }

        if self.invalid {
            let bang_x = area.right().saturating_sub(2);
            if bang_x >= x {
                buffer.set_stringn(
                    bang_x,
                    area.y,
                    Glyph::Error.resolve().text,
                    1,
                    Style::new()
                        .fg(theme.error)
                        .bg(field.bg.unwrap_or(theme.field))
                        .add_modifier(ratatui_core::style::Modifier::BOLD),
                );
            }
        }

        if let Some(annotation) = self.annotation {
            let used = match &self.value {
                FieldRowValue::Plain(value) => display_cols(value),
                FieldRowValue::Masked { .. } => MASK_CELLS,
                FieldRowValue::Composed(line) => line.width(),
                FieldRowValue::Unset { hint } => display_cols(hint),
            }
            .min(value_width);
            let annotation_x = x
                .saturating_add(u16::try_from(used).unwrap_or(u16::MAX))
                .saturating_add(self.system.spacing.gap);
            if annotation_x < area.right() {
                buffer.set_stringn(
                    annotation_x,
                    area.y,
                    annotation,
                    usize::from(area.right().saturating_sub(annotation_x)),
                    placeholder,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::{buffer::Buffer, layout::Rect};

    use super::*;
    use crate::style::{DesignSystem, Glyph, MASK_CELLS, Role};

    #[test]
    fn label_measurement_is_wide_glyph_correct_with_minimum() {
        assert_eq!(FieldRow::label_cols_for(["id", "日本語"].into_iter()), 8);
        assert_eq!(FieldRow::label_cols_for(["日本語日本"].into_iter()), 10);
    }

    #[test]
    fn masked_value_is_fixed_mask_cells() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Token", FieldRowValue::Masked { len: 5 }).paint(area, &mut buffer);
        let row = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let mask = Glyph::Mask.resolve().text.repeat(MASK_CELLS);
        assert!(row.contains(&mask), "{row:?}");
        assert!(
            !row.contains(&Glyph::Mask.resolve().text.repeat(MASK_CELLS + 1)),
            "mask must not track secret length: {row:?}"
        );
    }

    #[test]
    fn required_unset_uses_danger_role() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Token", FieldRowValue::Unset { hint: "required" })
            .required(true)
            .paint(area, &mut buffer);
        let expected = system.style(Role::Danger).fg;
        assert!(
            buffer
                .content()
                .iter()
                .any(|cell| Some(cell.fg) == expected)
        );
    }

    #[test]
    fn invalid_value_trails_bang_and_is_not_underlined() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Email", FieldRowValue::Plain("bad"))
            .invalid(true)
            .paint(area, &mut buffer);
        let value_cell = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "b")
            .expect("the value is painted");
        assert_ne!(
            Some(value_cell.fg),
            Some(theme.error),
            "the value text is not the error colour"
        );
        assert!(
            !value_cell
                .style()
                .add_modifier
                .contains(ratatui_core::style::Modifier::UNDERLINED),
            "idle invalid value is not underlined"
        );
        let row: String = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            row.contains('!'),
            "invalid field trails a bold `!`: {row:?}"
        );
    }

    #[test]
    fn idle_field_is_field_plane_with_hidden_gutter() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Name", FieldRowValue::Plain("Ada")).paint(area, &mut buffer);
        let gutter = &buffer[(0, 0)];
        assert_eq!(gutter.symbol(), "▎");
        assert_eq!(gutter.fg, gutter.bg, "idle gutter is hidden (fg=bg)");
        assert_eq!(gutter.bg, theme.field);
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "A")
            .expect("value");
        assert_eq!(value.bg, theme.field);
        assert_eq!(value.fg, theme.text_primary);
    }

    #[test]
    fn hover_not_editing_lifts_to_field_hover() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Name", FieldRowValue::Plain("Ada"))
            .hovered(true)
            .paint(area, &mut buffer);
        assert_eq!(buffer[(2, 0)].bg, theme.field_hover);
    }

    #[test]
    fn focused_editing_keeps_field_plane_and_accent_underline() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Name", FieldRowValue::Plain("Ada"))
            .selected(true)
            .editing(true)
            .hovered(true)
            .paint(area, &mut buffer);
        let gutter = &buffer[(0, 0)];
        assert_eq!(gutter.symbol(), "▎");
        assert_eq!(gutter.fg, theme.focus);
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "A")
            .expect("value");
        assert_eq!(value.bg, theme.field, "editing does not lift the well");
        assert_eq!(value.style().underline_color, Some(theme.accent));
        assert!(
            value
                .style()
                .add_modifier
                .contains(ratatui_core::style::Modifier::UNDERLINED)
        );
    }

    #[test]
    fn disabled_is_faint_and_does_not_hover() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Name", FieldRowValue::Plain("Ada"))
            .enabled(false)
            .hovered(true)
            .paint(area, &mut buffer);
        let gutter = &buffer[(0, 0)];
        assert_eq!(gutter.fg, gutter.bg, "disabled has no focus bar");
        assert_eq!(gutter.bg, theme.field);
        let value = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "A")
            .expect("value");
        assert_eq!(value.fg, theme.disabled);
        assert_eq!(value.bg, theme.field);
    }
}
