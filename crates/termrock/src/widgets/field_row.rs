// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composed label/value rows for forms and detail surfaces.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Line, widgets::Widget};

use crate::{
    style::{ControlState, DesignSystem, ListRowVisualState, Role},
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

    /// Sets the shared label-band width.
    #[must_use]
    pub fn label_cols(mut self, cols: u16) -> Self {
        self.label_cols = cols.max(8);
        self
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

    /// Paints this row into one terminal line.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let recipe = self.system.resolve_list_row(ListRowVisualState {
            selected: self.selected,
            focused: self.selected,
            hovered: self.hovered,
            enabled: self.enabled,
            loading: false,
            checked: false,
            ..ListRowVisualState::default()
        });
        let input_recipe = self.system.input_recipe(
            if !self.enabled {
                ControlState::Disabled
            } else if self.selected {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            self.invalid,
        );
        let row = Rect::new(area.x, area.y, area.width, 1);
        if recipe.use_fill {
            buffer.set_style(row, recipe.label);
        } else if recipe.use_tint {
            buffer.set_style(row, recipe.tint);
        } else if recipe.hover_fill {
            buffer.set_style(row, recipe.hover_wash);
        }

        let mut x = area.x;
        if let Some((glyph, style)) = recipe.gutter {
            buffer.set_stringn(x, area.y, glyph, 1, style);
        }
        x = x.saturating_add(2);
        if let Some(marker) = self.marker {
            let marker_width =
                display_cols(marker).min(usize::from(area.right().saturating_sub(x)));
            buffer.set_stringn(x, area.y, marker, marker_width, recipe.secondary);
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
            recipe.secondary,
        );
        x = x.saturating_add(label_width);
        x = x.saturating_add(self.system.spacing.gap);
        if x >= area.right() {
            return;
        }

        let annotation_width = self.annotation.map_or(0, |text| {
            display_cols(text).saturating_add(usize::from(self.system.spacing.gap))
        });
        let value_width =
            usize::from(area.right().saturating_sub(x)).saturating_sub(annotation_width);
        let value_style = input_recipe.value;
        match &self.value {
            FieldRowValue::Plain(value) => {
                let text = truncate_cols(value, value_width, "…");
                buffer.set_stringn(x, area.y, &text, value_width, value_style);
            }
            FieldRowValue::Masked { len } => {
                let glyph = "●";
                let text = glyph.repeat((*len).min(value_width));
                buffer.set_stringn(x, area.y, &text, value_width, value_style);
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
                    input_recipe
                        .placeholder
                        .patch(self.system.style(Role::Danger))
                } else {
                    input_recipe.placeholder
                };
                let text = truncate_cols(hint, value_width, "…");
                buffer.set_stringn(x, area.y, &text, value_width, style);
            }
        }

        // A one-line row's border slot is the underline under the value:
        // editing underlines in accent, an invalid value in error. The
        // underline is not a second text colour, so the value keeps the tone
        // `input_recipe` gave it.
        let mut underline = Style::new().add_modifier(input_recipe.border.add_modifier);
        if let Some(color) = input_recipe.border.underline_color {
            underline = underline.underline_color(color);
        }
        buffer.set_style(
            Rect::new(
                x,
                area.y,
                u16::try_from(value_width)
                    .unwrap_or(u16::MAX)
                    .min(area.right().saturating_sub(x)),
                1,
            ),
            underline,
        );

        if let Some(annotation) = self.annotation {
            let used = match &self.value {
                FieldRowValue::Plain(value) => display_cols(value),
                FieldRowValue::Masked { len } => *len,
                FieldRowValue::Composed(line) => line.width(),
                FieldRowValue::Unset { hint } => display_cols(hint),
            }
            .min(value_width);
            let annotation_x = x
                .saturating_add(u16::try_from(used).unwrap_or(u16::MAX))
                .saturating_add(self.system.spacing.gap);
            if annotation_x < area.right() {
                // An annotation is supporting text: the placeholder tone is its
                // whole cue, and ITALIC stays the comment tier's (D5).
                let style = input_recipe.placeholder;
                buffer.set_stringn(
                    annotation_x,
                    area.y,
                    annotation,
                    usize::from(area.right().saturating_sub(annotation_x)),
                    style,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::{buffer::Buffer, layout::Rect};

    use super::*;

    #[test]
    fn label_measurement_is_wide_glyph_correct_with_minimum() {
        assert_eq!(FieldRow::label_cols_for(["id", "日本語"].into_iter()), 8);
        assert_eq!(FieldRow::label_cols_for(["日本語日本"].into_iter()), 10);
    }

    #[test]
    fn masked_value_has_requested_display_width() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Token", FieldRowValue::Masked { len: 5 }).paint(area, &mut buffer);
        let row = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(row.contains("●●●●●"));
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
    fn invalid_value_underlines_in_error_and_keeps_its_tone() {
        // M6: an invalid field says so in its underline (`underline_color`
        // error), never by recolouring the value text.
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        FieldRow::new(&system, "Email", FieldRowValue::Plain("bad"))
            .invalid(true)
            .paint(area, &mut buffer);
        assert!(
            buffer
                .content()
                .iter()
                .any(|cell| cell.style().underline_color == Some(theme.error)
                    && cell
                        .style()
                        .add_modifier
                        .contains(ratatui_core::style::Modifier::UNDERLINED)),
            "the invalid field underlines in error"
        );
        let value_cell = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "b")
            .expect("the value is painted");
        assert_ne!(
            Some(value_cell.fg),
            system.style(Role::Danger).fg,
            "the value text is not the error colour"
        );
    }
}
